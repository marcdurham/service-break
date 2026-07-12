use chrono::{DateTime, Utc};
use shared::{
    Amenity, Invitation, Parking, PlaceDetail, PlaceSummary, PlaceType, PlacesQuery, Requirement,
    Review, SortBy,
};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::error::ApiError;
use crate::util::time_ago;

/// Applies seed.sql once, when the places table is empty.
pub async fn ensure_seeded(pool: &PgPool) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM places")
        .fetch_one(pool)
        .await?;
    if count == 0 {
        sqlx::raw_sql(include_str!("../seed.sql")).execute(pool).await?;
        tracing::info!("seeded demo places");
    }
    Ok(())
}

/// Pushes the haversine distance (miles) from (`lat`, `lng`) to the place row.
fn push_distance_expr(qb: &mut QueryBuilder<'_, Postgres>, lat: f64, lng: f64) {
    qb.push("2.0 * 3958.8 * asin(least(1.0, sqrt(power(sin(radians(p.lat - ")
        .push_bind(lat)
        .push(") / 2), 2) + cos(radians(")
        .push_bind(lat)
        .push(")) * cos(radians(p.lat)) * power(sin(radians(p.lng - ")
        .push_bind(lng)
        .push(") / 2), 2))))");
}

const SUMMARY_COLS: &str = "p.id, p.name, p.place_type, p.lat, p.lng, p.address, p.door_ft, \
     p.parking, p.purchase_required, p.code_required, p.amenities, \
     avg(r.clean)::float8 AS clean_avg, avg(r.coffee)::float8 AS coffee_avg, \
     avg(r.food)::float8 AS food_avg, count(r.id) AS review_count, ";

fn summary_from_row(row: &PgRow) -> Result<PlaceSummary, ApiError> {
    let place_type: String = row.try_get("place_type")?;
    let parking: String = row.try_get("parking")?;
    let purchase_required: String = row.try_get("purchase_required")?;
    let code_required: String = row.try_get("code_required")?;
    let amenities: Vec<String> = row.try_get("amenities")?;
    Ok(PlaceSummary {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        place_type: place_type
            .parse::<PlaceType>()
            .map_err(|()| ApiError::BadRequest(format!("unknown place type {place_type:?}")))?,
        lat: row.try_get("lat")?,
        lng: row.try_get("lng")?,
        address: row.try_get("address")?,
        door_ft: row.try_get("door_ft")?,
        parking: parking
            .parse::<Parking>()
            .map_err(|()| ApiError::BadRequest(format!("unknown parking {parking:?}")))?,
        purchase_required: purchase_required.parse::<Requirement>().map_err(|()| {
            ApiError::BadRequest(format!("unknown requirement {purchase_required:?}"))
        })?,
        code_required: code_required.parse::<Requirement>().map_err(|()| {
            ApiError::BadRequest(format!("unknown requirement {code_required:?}"))
        })?,
        amenities: amenities.iter().filter_map(|a| a.parse().ok()).collect(),
        clean_avg: row.try_get("clean_avg")?,
        coffee_avg: row.try_get("coffee_avg")?,
        food_avg: row.try_get("food_avg")?,
        review_count: row.try_get("review_count")?,
        distance_mi: row.try_get("distance_mi")?,
    })
}

fn push_summary_select(qb: &mut QueryBuilder<'_, Postgres>, q: &PlacesQuery) {
    qb.push("SELECT ").push(SUMMARY_COLS);
    if let (Some(lat), Some(lng)) = (q.lat, q.lng) {
        push_distance_expr(qb, lat, lng);
    } else {
        qb.push("NULL::float8");
    }
    qb.push(" AS distance_mi FROM places p LEFT JOIN reviews r ON r.place_id = p.id");
}

/// Builds a `%…%` ILIKE pattern, escaping the wildcard characters in `needle`.
fn like_pattern(needle: &str) -> String {
    let escaped: String = needle
        .chars()
        .flat_map(|c| match c {
            '\\' | '%' | '_' => vec!['\\', c],
            _ => vec![c],
        })
        .collect();
    format!("%{escaped}%")
}

fn push_filters(qb: &mut QueryBuilder<'_, Postgres>, q: &PlacesQuery) {
    if let Some(search) = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let pattern = like_pattern(search);
        qb.push(" AND (p.name ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR p.address ILIKE ")
            .push_bind(pattern)
            .push(")");
    }
    let types = q.parsed_types();
    if !types.is_empty() {
        qb.push(" AND p.place_type IN (");
        let mut sep = qb.separated(", ");
        for t in types {
            sep.push_bind(t.as_str());
        }
        qb.push(")");
    }
    if q.no_purchase == Some(true) {
        qb.push(" AND p.purchase_required = 'no' AND p.code_required = 'no'");
    }
    if q.has_parking == Some(true) {
        qb.push(" AND p.parking <> 'none'");
    }
}

pub async fn list_places(pool: &PgPool, q: &PlacesQuery) -> Result<Vec<PlaceSummary>, ApiError> {
    let mut qb = QueryBuilder::new("");
    push_summary_select(&mut qb, q);
    qb.push(" WHERE TRUE");
    push_filters(&mut qb, q);
    qb.push(" GROUP BY p.id");

    let mut first = true;
    if let Some(clean_min) = q.clean_min {
        qb.push(" HAVING avg(r.clean) >= ").push_bind(clean_min);
        first = false;
    }
    if let (Some(radius), Some(lat), Some(lng)) = (q.radius_mi, q.lat, q.lng) {
        qb.push(if first { " HAVING " } else { " AND " });
        push_distance_expr(&mut qb, lat, lng);
        qb.push(" <= ").push_bind(radius);
    }

    match q.sort.unwrap_or_default() {
        SortBy::Cleanliness => qb.push(" ORDER BY clean_avg DESC NULLS LAST, p.name"),
        SortBy::Distance if q.lat.is_some() && q.lng.is_some() => {
            qb.push(" ORDER BY distance_mi ASC, p.name")
        }
        SortBy::Distance => qb.push(" ORDER BY p.name"),
    };

    let rows = qb.build().fetch_all(pool).await?;
    rows.iter().map(summary_from_row).collect()
}

pub async fn get_place(
    pool: &PgPool,
    id: Uuid,
    origin: Option<(f64, f64)>,
) -> Result<PlaceDetail, ApiError> {
    let q = PlacesQuery {
        lat: origin.map(|(lat, _)| lat),
        lng: origin.map(|(_, lng)| lng),
        ..PlacesQuery::default()
    };
    let mut qb = QueryBuilder::new("");
    push_summary_select(&mut qb, &q);
    qb.push(" WHERE p.id = ").push_bind(id);
    qb.push(" GROUP BY p.id");
    let row = qb.build().fetch_optional(pool).await?.ok_or(ApiError::NotFound)?;
    let summary = summary_from_row(&row)?;

    let extra = sqlx::query("SELECT door_note, hours FROM places WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;

    let reviews = list_reviews(pool, id).await?;
    Ok(PlaceDetail {
        summary,
        door_note: extra.try_get("door_note")?,
        hours: extra.try_get("hours")?,
        reviews,
    })
}

async fn list_reviews(pool: &PgPool, place_id: Uuid) -> Result<Vec<Review>, ApiError> {
    let rows = sqlx::query(
        "SELECT r.id, r.device_id, r.clean, r.coffee, r.food, r.text, r.created_at, u.username \
         FROM reviews r LEFT JOIN users u ON u.id = r.user_id \
         WHERE r.place_id = $1 ORDER BY r.created_at DESC",
    )
    .bind(place_id)
    .fetch_all(pool)
    .await?;
    let now = Utc::now();
    rows.iter()
        .map(|row| {
            let device_id: String = row.try_get("device_id")?;
            let username: Option<String> = row.try_get("username")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;
            Ok(Review {
                id: row.try_get("id")?,
                // Pre-account rows (and seed data) have no user: fall back
                // to the anonymous scout name derived from the device id.
                author: username.unwrap_or_else(|| shared::scout_name(&device_id)),
                clean: row.try_get("clean")?,
                coffee: row.try_get("coffee")?,
                food: row.try_get("food")?,
                text: row.try_get("text")?,
                created_at: created_at.to_rfc3339(),
                time_ago: time_ago(created_at, now),
            })
        })
        .collect()
}

pub struct InsertPlace {
    pub name: String,
    pub place_type: PlaceType,
    pub lat: f64,
    pub lng: f64,
    pub address: String,
    pub door_ft: i32,
    pub door_note: String,
    pub parking: Parking,
    pub purchase_required: Requirement,
    pub code_required: Requirement,
    pub amenities: Vec<Amenity>,
    pub device_id: String,
    pub user_id: Uuid,
}

pub async fn insert_place(pool: &PgPool, p: &InsertPlace) -> Result<Uuid, ApiError> {
    let amenities: Vec<&str> = p.amenities.iter().map(|a| a.as_str()).collect();
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO places (name, place_type, lat, lng, address, door_ft, door_note, \
         parking, purchase_required, code_required, amenities, device_id, user_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING id",
    )
    .bind(&p.name)
    .bind(p.place_type.as_str())
    .bind(p.lat)
    .bind(p.lng)
    .bind(&p.address)
    .bind(p.door_ft)
    .bind(&p.door_note)
    .bind(p.parking.as_str())
    .bind(p.purchase_required.as_str())
    .bind(p.code_required.as_str())
    .bind(&amenities)
    .bind(&p.device_id)
    .bind(p.user_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub struct InsertReview<'a> {
    pub place_id: Uuid,
    pub device_id: &'a str,
    pub user_id: Uuid,
    pub clean: i16,
    pub coffee: Option<i16>,
    pub food: Option<i16>,
    pub text: &'a str,
}

pub async fn insert_review(pool: &PgPool, r: &InsertReview<'_>) -> Result<Uuid, ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM places WHERE id = $1)")
        .bind(r.place_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO reviews (place_id, device_id, user_id, clean, coffee, food, text) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(r.place_id)
    .bind(r.device_id)
    .bind(r.user_id)
    .bind(r.clean)
    .bind(r.coffee)
    .bind(r.food)
    .bind(r.text)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
}

/// Atomically redeems `invite_code` and creates the account it admits.
/// Rolls back (leaving the invitation unredeemed) if the code is invalid,
/// already used, or the username is taken.
pub async fn register_user(
    pool: &PgPool,
    username: &str,
    password_hash: &str,
    invite_code: &str,
) -> Result<Uuid, ApiError> {
    let mut tx = pool.begin().await?;

    let invitation_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM invitations WHERE code = $1 AND redeemed_at IS NULL FOR UPDATE",
    )
    .bind(invite_code)
    .fetch_optional(&mut *tx)
    .await?;
    let invitation_id = invitation_id.ok_or_else(|| {
        ApiError::BadRequest("that invite code is invalid or already used".to_owned())
    })?;

    let user_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id",
    )
    .bind(username)
    .bind(password_hash)
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(ApiError::Conflict("that username is taken".to_owned()));
        }
        Err(e) => return Err(e.into()),
    };

    sqlx::query("UPDATE invitations SET redeemed_at = now(), redeemed_by = $1 WHERE id = $2")
        .bind(user_id)
        .bind(invitation_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(user_id)
}

/// Issues a fresh, unredeemed invitation code for `inviter_id`.
pub async fn create_invitation(pool: &PgPool, inviter_id: Uuid) -> Result<String, ApiError> {
    for _ in 0..5 {
        let code = format!("BREAK-{}", &Uuid::new_v4().simple().to_string()[..6].to_uppercase());
        let res = sqlx::query("INSERT INTO invitations (code, inviter_id) VALUES ($1, $2)")
            .bind(&code)
            .bind(inviter_id)
            .execute(pool)
            .await;
        match res {
            Ok(_) => return Ok(code),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(ApiError::Internal("could not generate a unique invite code".to_owned()))
}

/// This user's invitations, most recent first.
pub async fn list_invitations(pool: &PgPool, inviter_id: Uuid) -> Result<Vec<Invitation>, ApiError> {
    let rows = sqlx::query(
        "SELECT code, redeemed_at IS NOT NULL AS redeemed FROM invitations \
         WHERE inviter_id = $1 ORDER BY created_at DESC",
    )
    .bind(inviter_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(Invitation {
                code: r.try_get("code")?,
                redeemed: r.try_get("redeemed")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(ApiError::from)
}

/// Looks a user up by username, case-insensitively.
pub async fn find_user(pool: &PgPool, username: &str) -> Result<Option<UserRow>, ApiError> {
    let row = sqlx::query(
        "SELECT id, username, password_hash FROM users WHERE lower(username) = lower($1)",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    row.map(|r| {
        Ok(UserRow {
            id: r.try_get("id")?,
            username: r.try_get("username")?,
            password_hash: r.try_get("password_hash")?,
        })
    })
    .transpose()
}

/// How long a login stays valid.
const SESSION_DAYS: i32 = 30;

pub async fn create_session(pool: &PgPool, token: &str, user_id: Uuid) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO sessions (token, user_id, expires_at) \
         VALUES ($1, $2, now() + make_interval(days => $3))",
    )
    .bind(token)
    .bind(user_id)
    .bind(SESSION_DAYS)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_session(pool: &PgPool, token: &str) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM sessions WHERE token = $1").bind(token).execute(pool).await?;
    Ok(())
}

/// Resolves a session token to its user, if the session is still valid.
pub async fn session_user(
    pool: &PgPool,
    token: &str,
) -> Result<Option<crate::auth::AuthUser>, ApiError> {
    let row = sqlx::query(
        "SELECT u.id, u.username FROM sessions s JOIN users u ON u.id = s.user_id \
         WHERE s.token = $1 AND s.expires_at > now()",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    row.map(|r| {
        Ok(crate::auth::AuthUser {
            id: r.try_get("id")?,
            username: r.try_get("username")?,
        })
    })
    .transpose()
}

pub async fn save_place(pool: &PgPool, device_id: &str, place_id: Uuid) -> Result<(), ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM places WHERE id = $1)")
        .bind(place_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    sqlx::query(
        "INSERT INTO saved_places (device_id, place_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(device_id)
    .bind(place_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn unsave_place(pool: &PgPool, device_id: &str, place_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM saved_places WHERE device_id = $1 AND place_id = $2")
        .bind(device_id)
        .bind(place_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_saved(
    pool: &PgPool,
    device_id: &str,
    origin: Option<(f64, f64)>,
) -> Result<Vec<PlaceSummary>, ApiError> {
    let q = PlacesQuery {
        lat: origin.map(|(lat, _)| lat),
        lng: origin.map(|(_, lng)| lng),
        ..PlacesQuery::default()
    };
    let mut qb = QueryBuilder::new("");
    push_summary_select(&mut qb, &q);
    qb.push(" JOIN saved_places s ON s.place_id = p.id AND s.device_id = ")
        .push_bind(device_id);
    qb.push(" GROUP BY p.id, s.created_at ORDER BY s.created_at DESC");
    let rows = qb.build().fetch_all(pool).await?;
    rows.iter().map(summary_from_row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("camber"), "%camber%");
        assert_eq!(like_pattern("50% off_deal"), "%50\\% off\\_deal%");
        assert_eq!(like_pattern("back\\slash"), "%back\\\\slash%");
    }
}
