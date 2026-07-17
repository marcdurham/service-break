use chrono::{DateTime, Utc};
use shared::{
    ActivityEntry, Amenity, Invitation, InviteStatus, InvitesOverview, Parking, PlaceDetail,
    PlaceEdit, PlaceSummary, PlaceType, PlacesQuery, Requirement, Review, SortBy, UserSummary,
    INVITES_PER_DAY_NEW, INVITES_PER_DAY_OLD_AGE, INVITE_EXPIRY_DAYS,
};
use sqlx::postgres::PgRow;
use sqlx::{Acquire, PgPool, Postgres, QueryBuilder, Row};
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
    let amenities = q.parsed_amenities();
    if !amenities.is_empty() {
        // Overlap (`&&`): the place offers at least one requested amenity.
        let amenities: Vec<String> = amenities.iter().map(|a| a.to_string()).collect();
        qb.push(" AND p.amenities && ").push_bind(amenities);
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
    qb.push(" WHERE p.deleted_at IS NULL AND TRUE");
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
    qb.push(" AND p.deleted_at IS NULL");
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

/// The resolved editable fields of a place, applied by [`update_place`].
pub struct UpdateFields {
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
    pub hours: Option<String>,
}

/// Sorted, comma-joined amenity list — a stable text form for the audit log
/// that doesn't flag a reordered multi-select as a change.
fn amenities_text<S: AsRef<str>>(amenities: &[S]) -> String {
    let mut v: Vec<&str> = amenities.iter().map(AsRef::as_ref).collect();
    v.sort_unstable();
    v.join(",")
}

/// Applies an edit to a place and records one `place_edits` audit row per
/// changed field — what changed (old and new value), when, and by whom.
/// Returns how many fields actually changed; a no-op edit writes nothing.
pub async fn update_place(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    f: &UpdateFields,
) -> Result<usize, ApiError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT name, place_type, lat, lng, address, door_ft, door_note, parking, \
         purchase_required, code_required, amenities, hours \
         FROM places WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;

    let old_lat: f64 = row.try_get("lat")?;
    let old_lng: f64 = row.try_get("lng")?;
    let old_amenities: Vec<String> = row.try_get("amenities")?;
    let old_hours: Option<String> = row.try_get("hours")?;
    let new_amenities: Vec<&str> = f.amenities.iter().map(|a| a.as_str()).collect();

    let mut changes: Vec<(&'static str, String, String)> = Vec::new();
    let mut diff = |field: &'static str, old: String, new: String| {
        if old != new {
            changes.push((field, old, new));
        }
    };
    diff("name", row.try_get("name")?, f.name.clone());
    diff("place_type", row.try_get("place_type")?, f.place_type.to_string());
    if (old_lat - f.lat).abs() > 1e-9 || (old_lng - f.lng).abs() > 1e-9 {
        diff(
            "location",
            shared::fmt_latlng(old_lat, old_lng),
            shared::fmt_latlng(f.lat, f.lng),
        );
    }
    diff("address", row.try_get("address")?, f.address.clone());
    diff("door_ft", row.try_get::<i32, _>("door_ft")?.to_string(), f.door_ft.to_string());
    diff("door_note", row.try_get("door_note")?, f.door_note.clone());
    diff("parking", row.try_get("parking")?, f.parking.to_string());
    diff(
        "purchase_required",
        row.try_get("purchase_required")?,
        f.purchase_required.to_string(),
    );
    diff("code_required", row.try_get("code_required")?, f.code_required.to_string());
    diff("amenities", amenities_text(&old_amenities), amenities_text(&new_amenities));
    diff("hours", old_hours.unwrap_or_default(), f.hours.clone().unwrap_or_default());

    if changes.is_empty() {
        return Ok(0);
    }

    sqlx::query(
        "UPDATE places SET name = $1, place_type = $2, lat = $3, lng = $4, address = $5, \
         door_ft = $6, door_note = $7, parking = $8, purchase_required = $9, \
         code_required = $10, amenities = $11, hours = $12 WHERE id = $13",
    )
    .bind(&f.name)
    .bind(f.place_type.as_str())
    .bind(f.lat)
    .bind(f.lng)
    .bind(&f.address)
    .bind(f.door_ft)
    .bind(&f.door_note)
    .bind(f.parking.as_str())
    .bind(f.purchase_required.as_str())
    .bind(f.code_required.as_str())
    .bind(&new_amenities)
    .bind(&f.hours)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    for (field, old_value, new_value) in &changes {
        sqlx::query(
            "INSERT INTO place_edits (place_id, user_id, field, old_value, new_value) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(user_id)
        .bind(field)
        .bind(old_value)
        .bind(new_value)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(changes.len())
}

/// A place's edit history, most recent first.
pub async fn list_place_edits(pool: &PgPool, place_id: Uuid) -> Result<Vec<PlaceEdit>, ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM places WHERE id = $1)")
        .bind(place_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    let rows = sqlx::query(
        "SELECT e.field, e.old_value, e.new_value, e.created_at, u.username \
         FROM place_edits e LEFT JOIN users u ON u.id = e.user_id \
         WHERE e.place_id = $1 ORDER BY e.created_at DESC, e.field",
    )
    .bind(place_id)
    .fetch_all(pool)
    .await?;
    let now = Utc::now();
    rows.iter()
        .map(|row| {
            let username: Option<String> = row.try_get("username")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;
            Ok(PlaceEdit {
                field: row.try_get("field")?,
                old_value: row.try_get("old_value")?,
                new_value: row.try_get("new_value")?,
                // Edits always come from an account; NULL only remains
                // where the account was deleted afterwards.
                author: username.unwrap_or_else(|| "(deleted account)".to_owned()),
                created_at: created_at.to_rfc3339(),
                time_ago: time_ago(created_at, now),
            })
        })
        .collect()
}

/// Records one row in the account-activity audit log: a login, a failed
/// login, or a field changed on the account (by the account itself or an
/// admin). `field`/`old_value`/`new_value` stay empty for login events.
pub async fn record_activity(
    pool: &PgPool,
    user_id: Uuid,
    activity_type: &str,
    field: &str,
    old_value: &str,
    new_value: &str,
    actor_id: Option<Uuid>,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO user_activity (user_id, activity_type, field, old_value, new_value, actor_id) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(activity_type)
    .bind(field)
    .bind(old_value)
    .bind(new_value)
    .bind(actor_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// The last `limit` activity events for one account, newest first: logins,
/// failed logins, profile changes (by the account or an admin), invitations
/// removed, place edits, and ratings (reviews) posted. Each source is
/// queried independently (capped at `limit` rows each — enough for a
/// correct n-way merge), combined, and truncated to `limit`.
pub async fn list_user_activity(
    pool: &PgPool,
    user_id: Uuid,
    username: &str,
    limit: i64,
) -> Result<Vec<ActivityEntry>, ApiError> {
    let now = Utc::now();
    let mut combined: Vec<(DateTime<Utc>, ActivityEntry)> = Vec::new();

    let account_rows = sqlx::query(
        "SELECT a.activity_type, a.field, a.old_value, a.new_value, a.created_at, \
         actor.username AS actor_username \
         FROM user_activity a LEFT JOIN users actor ON actor.id = a.actor_id \
         WHERE a.user_id = $1 ORDER BY a.created_at DESC LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    for row in account_rows {
        let activity_type: String = row.try_get("activity_type")?;
        let field: String = row.try_get("field")?;
        let old_value: String = row.try_get("old_value")?;
        let new_value: String = row.try_get("new_value")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;
        let actor_username: Option<String> = row.try_get("actor_username")?;
        let actor = actor_username.unwrap_or_else(|| username.to_owned());
        let (kind, summary) = match activity_type.as_str() {
            "login" => ("login", "Signed in".to_owned()),
            "failed_login" => ("failed_login", "Failed sign-in attempt".to_owned()),
            "invite_deleted" => {
                ("invite_change", format!("Removed invitation for {old_value}"))
            }
            _ if field == "password" => ("profile_change", "Password reset".to_owned()),
            _ if field == "is_admin" => (
                "profile_change",
                format!(
                    "Admin status changed: {} → {}",
                    shared::bool_label(&old_value),
                    shared::bool_label(&new_value),
                ),
            ),
            _ => (
                "profile_change",
                format!(
                    "{}: {} → {}",
                    shared::user_field_label(&field),
                    shared::edit_value_display(&old_value),
                    shared::edit_value_display(&new_value),
                ),
            ),
        };
        combined.push((
            created_at,
            ActivityEntry {
                kind: kind.to_owned(),
                summary,
                actor,
                created_at: created_at.to_rfc3339(),
                time_ago: time_ago(created_at, now),
            },
        ));
    }

    let place_rows = sqlx::query(
        "SELECT e.field, e.old_value, e.new_value, e.created_at, p.name AS place_name \
         FROM place_edits e JOIN places p ON p.id = e.place_id \
         WHERE e.user_id = $1 ORDER BY e.created_at DESC LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    for row in place_rows {
        let field: String = row.try_get("field")?;
        let old_value: String = row.try_get("old_value")?;
        let new_value: String = row.try_get("new_value")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;
        let place_name: String = row.try_get("place_name")?;
        let summary = format!(
            "Edited {}: {} changed from {} to {}",
            place_name,
            shared::edit_field_label(&field),
            shared::edit_value_display(&old_value),
            shared::edit_value_display(&new_value),
        );
        combined.push((
            created_at,
            ActivityEntry {
                kind: "place_change".to_owned(),
                summary,
                actor: username.to_owned(),
                created_at: created_at.to_rfc3339(),
                time_ago: time_ago(created_at, now),
            },
        ));
    }

    let review_rows = sqlx::query(
        "SELECT r.clean, r.coffee, r.food, r.created_at, p.name AS place_name \
         FROM reviews r JOIN places p ON p.id = r.place_id \
         WHERE r.user_id = $1 ORDER BY r.created_at DESC LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    for row in review_rows {
        let clean: i16 = row.try_get("clean")?;
        let coffee: Option<i16> = row.try_get("coffee")?;
        let food: Option<i16> = row.try_get("food")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;
        let place_name: String = row.try_get("place_name")?;
        let mut summary = format!("Rated {place_name} — cleanliness {clean}/5");
        if let Some(c) = coffee {
            summary.push_str(&format!(", coffee {c}/5"));
        }
        if let Some(f) = food {
            summary.push_str(&format!(", food {f}/5"));
        }
        combined.push((
            created_at,
            ActivityEntry {
                kind: "rating".to_owned(),
                summary,
                actor: username.to_owned(),
                created_at: created_at.to_rfc3339(),
                time_ago: time_ago(created_at, now),
            },
        ));
    }

    combined.sort_by(|a, b| b.0.cmp(&a.0));
    combined.truncate(limit.max(0) as usize);
    Ok(combined.into_iter().map(|(_, e)| e).collect())
}

pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    /// `NULL` for accounts that only ever signed up via Google.
    pub password_hash: Option<String>,
    pub is_admin: bool,
    pub given_name: String,
    pub family_name: String,
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
        "SELECT id FROM invitations WHERE code = $1 AND redeemed_at IS NULL \
         AND is_expired = false \
         AND created_at > now() - make_interval(days => $2) FOR UPDATE",
    )
    .bind(invite_code)
    .bind(INVITE_EXPIRY_DAYS)
    .fetch_optional(&mut *tx)
    .await?;
    let invitation_id = invitation_id.ok_or_else(|| {
        ApiError::BadRequest("that invite code is invalid, expired, or already used".to_owned())
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

/// Minutes an OAuth `state` value stays redeemable before it's treated as
/// expired (covers the round trip to Google's consent screen and back).
const OAUTH_STATE_MINUTES: i32 = 10;

/// Stores a fresh, single-use `state` value for the Google OAuth redirect,
/// carrying `mode` ("login" or "register") and the invite code (empty for
/// login) through the round trip. Opportunistically sweeps expired rows so
/// abandoned sign-in attempts don't accumulate.
pub async fn create_oauth_state(
    pool: &PgPool,
    state: &str,
    mode: &str,
    invite_code: &str,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM oauth_states WHERE expires_at < now()").execute(pool).await?;
    sqlx::query(
        "INSERT INTO oauth_states (state, mode, invite_code, expires_at) \
         VALUES ($1, $2, $3, now() + make_interval(mins => $4))",
    )
    .bind(state)
    .bind(mode)
    .bind(invite_code)
    .bind(OAUTH_STATE_MINUTES)
    .execute(pool)
    .await?;
    Ok(())
}

/// Like [`create_oauth_state`], but for the "link an existing account to a
/// Google identity" flow: mode is always `"link"` and `user_id` (the
/// already-signed-in user doing the linking) rides along instead of an
/// invite code, so the callback knows which account to attach the Google
/// identity to.
pub async fn create_oauth_link_state(
    pool: &PgPool,
    state: &str,
    user_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM oauth_states WHERE expires_at < now()").execute(pool).await?;
    sqlx::query(
        "INSERT INTO oauth_states (state, mode, invite_code, user_id, expires_at) \
         VALUES ($1, 'link', '', $2, now() + make_interval(mins => $3))",
    )
    .bind(state)
    .bind(user_id)
    .bind(OAUTH_STATE_MINUTES)
    .execute(pool)
    .await?;
    Ok(())
}

/// Consumes a `state` value: deletes it (so it can never be replayed) and
/// returns its `(mode, invite_code, linking_user_id)` if it existed and
/// hadn't expired. `linking_user_id` is only ever set for `mode == "link"`.
pub async fn take_oauth_state(
    pool: &PgPool,
    state: &str,
) -> Result<Option<(String, String, Option<Uuid>)>, ApiError> {
    let row = sqlx::query(
        "DELETE FROM oauth_states WHERE state = $1 \
         RETURNING mode, invite_code, user_id, expires_at > now() AS still_valid",
    )
    .bind(state)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|r| {
        let still_valid: bool = r.try_get("still_valid").ok()?;
        if !still_valid {
            return None;
        }
        Some((r.try_get("mode").ok()?, r.try_get("invite_code").ok()?, r.try_get("user_id").ok()?))
    }))
}

/// Looks up an account already linked to a Google subject id — the
/// "sign in with Google" path, which never creates an account.
pub async fn find_user_by_google_sub(
    pool: &PgPool,
    google_sub: &str,
) -> Result<Option<UserRow>, ApiError> {
    let row = sqlx::query(
        "SELECT id, username, password_hash, is_admin, given_name, family_name FROM users \
         WHERE google_sub = $1",
    )
    .bind(google_sub)
    .fetch_optional(pool)
    .await?;
    row.map(|r| {
        Ok(UserRow {
            id: r.try_get("id")?,
            username: r.try_get("username")?,
            password_hash: r.try_get("password_hash")?,
            is_admin: r.try_get("is_admin")?,
            given_name: r.try_get("given_name")?,
            family_name: r.try_get("family_name")?,
        })
    })
    .transpose()
}

/// Attaches a Google identity to an already-signed-in user's account (the
/// "link" flow, as opposed to `upsert_google_user`'s register/login-only
/// flow). A no-op if that Google identity is already linked to this same
/// user; an error if it's linked to a different one, or if its email
/// collides with a different account's.
pub async fn link_google_account(
    pool: &PgPool,
    user_id: Uuid,
    google_sub: &str,
    email: &str,
    given_name: &str,
    family_name: &str,
) -> Result<(), ApiError> {
    if let Some(existing_id) =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE google_sub = $1")
            .bind(google_sub)
            .fetch_optional(pool)
            .await?
    {
        return if existing_id == user_id {
            Ok(())
        } else {
            Err(ApiError::Conflict(
                "that Google account is already linked to a different user".to_owned(),
            ))
        };
    }

    let result = sqlx::query(
        "UPDATE users SET google_sub = $2, email = $3, \
         given_name = CASE WHEN given_name = '' THEN $4 ELSE given_name END, \
         family_name = CASE WHEN family_name = '' THEN $5 ELSE family_name END \
         WHERE id = $1",
    )
    .bind(user_id)
    .bind(google_sub)
    .bind(email)
    .bind(given_name)
    .bind(family_name)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e))
            if e.is_unique_violation() && e.constraint() == Some("idx_users_email") =>
        {
            Err(ApiError::Conflict(
                "that Google account's email is already used by a different account".to_owned(),
            ))
        }
        Err(e) => Err(e.into()),
    }
}

/// A username candidate derived from the local part of an email address:
/// lowercased, stripped of anything outside the allowed username charset,
/// and capped well under [`shared::validate_username`]'s max so a numeric
/// disambiguating suffix always fits.
fn username_from_email(email: &str) -> String {
    let local = email.split('@').next().unwrap_or("");
    let cleaned: String = local
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .map(|c| c.to_ascii_lowercase())
        .take(20)
        .collect();
    if cleaned.chars().count() < 3 {
        "scout".to_owned()
    } else {
        cleaned
    }
}

/// The "sign up with Google" path: logs in if `google_sub` is already
/// linked to an account (so clicking "Continue with Google" again just
/// signs you in), otherwise atomically redeems `invite_code` and creates a
/// new account the same way [`register_user`] does. Returns
/// `(user_id, username, is_admin)`.
pub async fn upsert_google_user(
    pool: &PgPool,
    google_sub: &str,
    email: &str,
    given_name: &str,
    family_name: &str,
    invite_code: &str,
) -> Result<(Uuid, String, bool), ApiError> {
    let mut tx = pool.begin().await?;

    if let Some(row) = sqlx::query("SELECT id, username, is_admin FROM users WHERE google_sub = $1")
        .bind(google_sub)
        .fetch_optional(&mut *tx)
        .await?
    {
        let id: Uuid = row.try_get("id")?;
        let username: String = row.try_get("username")?;
        let is_admin: bool = row.try_get("is_admin")?;
        tx.commit().await?;
        return Ok((id, username, is_admin));
    }

    let invitation_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM invitations WHERE code = $1 AND redeemed_at IS NULL \
         AND is_expired = false \
         AND created_at > now() - make_interval(days => $2) FOR UPDATE",
    )
    .bind(invite_code)
    .bind(INVITE_EXPIRY_DAYS)
    .fetch_optional(&mut *tx)
    .await?;
    let invitation_id = invitation_id.ok_or_else(|| {
        ApiError::BadRequest("that invite code is invalid, expired, or already used".to_owned())
    })?;

    // Each attempt runs in its own savepoint: a unique-violation aborts
    // whatever Postgres transaction it happened in, so retrying the INSERT
    // (with a disambiguated username) has to happen in a fresh one rather
    // than reusing `tx`, which Postgres would otherwise refuse to accept
    // any further commands on.
    let base = username_from_email(email);
    let mut username = base.clone();
    let mut suffix = 0u32;
    let user_id: Uuid = loop {
        let mut savepoint = tx.begin().await?;
        let attempt = sqlx::query_scalar(
            "INSERT INTO users (username, google_sub, email, given_name, family_name) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(&username)
        .bind(google_sub)
        .bind(email)
        .bind(given_name)
        .bind(family_name)
        .fetch_one(&mut *savepoint)
        .await;
        match attempt {
            Ok(id) => {
                savepoint.commit().await?;
                break id;
            }
            Err(sqlx::Error::Database(e))
                if e.is_unique_violation()
                    && e.constraint() == Some("idx_users_username")
                    && suffix < 50 =>
            {
                savepoint.rollback().await?;
                suffix += 1;
                username = format!("{base}{suffix}");
            }
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                savepoint.rollback().await?;
                return Err(ApiError::Conflict(
                    "an account for this Google email already exists".to_owned(),
                ));
            }
            Err(e) => {
                let _ = savepoint.rollback().await;
                return Err(e.into());
            }
        }
    };

    sqlx::query("UPDATE invitations SET redeemed_at = now(), redeemed_by = $1 WHERE id = $2")
        .bind(user_id)
        .bind(invitation_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok((user_id, username, false))
}

/// Replaces a user's stored Argon2 hash. Used for password changes; never
/// returns error on an unknown id so callers can't probe account existence.
pub async fn update_user_password(
    pool: &PgPool,
    user_id: Uuid,
    new_hash: &str,
) -> Result<(), ApiError> {
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(user_id)
        .bind(new_hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// Updates a user's given and/or family name. Only fields that are `Some`
/// get written; `None` leaves the column untouched.
pub async fn update_user_names(
    pool: &PgPool,
    user_id: Uuid,
    given_name: Option<&str>,
    family_name: Option<&str>,
) -> Result<(), ApiError> {
    match (
        given_name.map(|s| s.to_owned()),
        family_name.map(|s| s.to_owned()),
    ) {
        (Some(g), Some(f)) => {
            sqlx::query("UPDATE users SET given_name = $1, family_name = $2 WHERE id = $3")
                .bind(&g)
                .bind(&f)
                .bind(user_id)
                .execute(pool)
                .await?;
        }
        (Some(g), None) => {
            sqlx::query("UPDATE users SET given_name = $1 WHERE id = $2")
                .bind(&g)
                .bind(user_id)
                .execute(pool)
                .await?;
        }
        (None, Some(f)) => {
            sqlx::query("UPDATE users SET family_name = $1 WHERE id = $2")
                .bind(&f)
                .bind(user_id)
                .execute(pool)
                .await?;
        }
        (None, None) => {}
    }
    Ok(())
}

/// Looks a user up by id. Used when the caller already has an authenticated /// identity but needs to read stored fields (e.g. the password hash).
pub async fn find_user_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<UserRow>, ApiError> {
    let row = sqlx::query(
        "SELECT id, username, password_hash, is_admin, given_name, family_name FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    row.map(|r| {
        Ok(UserRow {
            id: r.try_get("id")?,
            username: r.try_get("username")?,
            password_hash: r.try_get("password_hash")?,
            is_admin: r.try_get("is_admin")?,
            given_name: r.try_get("given_name")?,
            family_name: r.try_get("family_name")?,
        })
    })
    .transpose()
}

/// An 8-character invite code — just the code, no prefix.
fn new_invite_code() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_uppercase()
}

/// Issues a fresh, unredeemed invitation code for `inviter_id`, enforcing
/// the anti-abuse rules: new accounts get 25 invites/day, older accounts
/// (24+ hours) get 100/day. Admins bypass the limit entirely.
pub async fn create_invitation(
    pool: &PgPool,
    inviter_id: Uuid,
    name: &str,
    is_admin: bool,
) -> Result<Invitation, ApiError> {
    let limit = if !is_admin {
        let old_enough: Option<bool> = sqlx::query_scalar(
            "SELECT created_at <= now() - interval '24 hours' FROM users WHERE id = $1",
        )
        .bind(inviter_id)
        .fetch_optional(pool)
        .await?;
        if old_enough.unwrap_or(false) {
            INVITES_PER_DAY_OLD_AGE
        } else {
            INVITES_PER_DAY_NEW
        }
    } else {
        i64::MAX // admins bypass the limit entirely
    };

    let sent_today: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invitations \
         WHERE inviter_id = $1 AND created_at > now() - interval '1 day'",
    )
    .bind(inviter_id)
    .fetch_one(pool)
    .await?;
    if sent_today >= limit {
        return Err(ApiError::BadRequest(format!(
            "invitation limit reached — you can send {limit} per day"
        )));
    }

    for _ in 0..5 {
        let code = new_invite_code();
        let res = sqlx::query("INSERT INTO invitations (code, inviter_id, name) VALUES ($1, $2, $3)")
            .bind(&code)
            .bind(inviter_id)
            .bind(name)
            .execute(pool)
            .await;
        match res {
            Ok(_) => {
                return Ok(Invitation {
                    code,
                    name: name.to_owned(),
                    status: InviteStatus::Pending,
                    joined_username: None,
                })
            }
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(ApiError::Internal("could not generate a unique invite code".to_owned()))
}

/// Everything the account page shows about invitations: who invited this
/// user, the (renameable) invitation they redeemed, and every invitation
/// they've sent, most recent first.
pub async fn invites_overview(pool: &PgPool, user_id: Uuid) -> Result<InvitesOverview, ApiError> {
    let rows = sqlx::query(
        "SELECT i.code, i.name, i.redeemed_at IS NOT NULL AS redeemed, \
         (i.created_at <= now() - make_interval(days => $2) OR i.is_expired) AS expired, \
         u.username AS joined_username \
         FROM invitations i LEFT JOIN users u ON u.id = i.redeemed_by \
         WHERE i.inviter_id = $1 AND i.deleted_at IS NULL ORDER BY i.created_at DESC",
    )
    .bind(user_id)
    .bind(INVITE_EXPIRY_DAYS)
    .fetch_all(pool)
    .await?;
    let invites = rows
        .into_iter()
        .map(|r| {
            let redeemed: bool = r.try_get("redeemed")?;
            let expired: bool = r.try_get("expired")?;
            Ok(Invitation {
                code: r.try_get("code")?,
                name: r.try_get("name")?,
                status: if redeemed {
                    InviteStatus::Joined
                } else if expired {
                    InviteStatus::Expired
                } else {
                    InviteStatus::Pending
                },
                joined_username: r.try_get("joined_username")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

    let mine = sqlx::query(
        "SELECT i.code, i.name, u.username AS inviter \
         FROM invitations i LEFT JOIN users u ON u.id = i.inviter_id \
         WHERE i.redeemed_by = $1 ORDER BY i.redeemed_at LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let (invited_by, my_invite_code, my_invite_name) = match mine {
        Some(r) => (
            r.try_get("inviter")?,
            Some(r.try_get::<String, _>("code")?),
            Some(r.try_get::<String, _>("name")?),
        ),
        None => (None, None, None),
    };

    Ok(InvitesOverview { invited_by, my_invite_code, my_invite_name, invites })
}

/// Renames an invitation. Allowed for the inviter while the code is still
/// unredeemed, and for the user who redeemed it (so friends can fix the
/// name they were invited under once they've registered).
pub async fn rename_invitation(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
    name: &str,
) -> Result<(), ApiError> {
    let res = sqlx::query(
        "UPDATE invitations SET name = $1 WHERE code = $2 \
         AND (redeemed_by = $3 OR (inviter_id = $3 AND redeemed_at IS NULL))",
    )
    .bind(name)
    .bind(code)
    .bind(user_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

/// What [`revoke_invitation`] actually did, so the caller can decide
/// whether an activity-log entry is warranted.
pub enum InvitationRemoval {
    /// An already-expired invitation was soft-deleted (hidden from the
    /// list). Carries its display name for the activity log.
    Deleted { name: String },
    /// A still-pending invitation was marked expired so it can no longer
    /// be redeemed.
    Revoked,
}

/// Removes an invitation. Only the inviter can act on their own codes, and
/// redeemed invitations can't be touched. An already-expired invitation is
/// soft-deleted (kept in the database, just hidden from the list) since
/// there's nothing left to preserve it for; a still-pending one is
/// soft-revoked by marking it expired instead, so an in-flight signup
/// can't redeem it out from under the inviter.
pub async fn revoke_invitation(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
) -> Result<InvitationRemoval, ApiError> {
    let row = sqlx::query(
        "UPDATE invitations SET deleted_at = now(), deleted_by = $2 \
         WHERE code = $1 AND inviter_id = $2 AND redeemed_at IS NULL AND deleted_at IS NULL \
         AND (is_expired OR created_at <= now() - make_interval(days => $3)) \
         RETURNING name",
    )
    .bind(code)
    .bind(user_id)
    .bind(INVITE_EXPIRY_DAYS)
    .fetch_optional(pool)
    .await?;
    if let Some(row) = row {
        let name: String = row.try_get("name")?;
        let name = if name.trim().is_empty() { code.to_owned() } else { name };
        return Ok(InvitationRemoval::Deleted { name });
    }
    let res = sqlx::query(
        "UPDATE invitations SET is_expired = true \
         WHERE code = $1 AND inviter_id = $2 AND redeemed_at IS NULL AND deleted_at IS NULL",
    )
    .bind(code)
    .bind(user_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(InvitationRemoval::Revoked)
}

/// Looks a user up by username, case-insensitively.
pub async fn find_user(pool: &PgPool, username: &str) -> Result<Option<UserRow>, ApiError> {
    let row = sqlx::query(
        "SELECT id, username, password_hash, is_admin, given_name, family_name FROM users \
         WHERE lower(username) = lower($1)",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    row.map(|r| {
        Ok(UserRow {
            id: r.try_get("id")?,
            username: r.try_get("username")?,
            password_hash: r.try_get("password_hash")?,
            is_admin: r.try_get("is_admin")?,
            given_name: r.try_get("given_name")?,
            family_name: r.try_get("family_name")?,
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
        "SELECT u.id, u.username, u.is_admin, u.given_name, u.family_name FROM sessions s JOIN users u ON u.id = s.user_id \
         WHERE s.token = $1 AND s.expires_at > now()",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    row.map(|r| {
        Ok(crate::auth::AuthUser {
            id: r.try_get("id")?,
            username: r.try_get("username")?,
            is_admin: r.try_get("is_admin")?,
            given_name: r.try_get("given_name")?,
            family_name: r.try_get("family_name")?,
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

/// Soft-deletes a place: marks `deleted_at` and `deleted_by`. The place
/// remains queryable by id (so the detail page can still render after the
/// delete), but drops out of list/saved results.
pub async fn delete_place(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM places WHERE id = $1 AND deleted_at IS NULL)")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    sqlx::query(
        "UPDATE places SET deleted_at = now(), deleted_by = $2 WHERE id = $1",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// All accounts — admin-only. Ids are returned as strings so the frontend
/// doesn't have to pull in uuid for a read-only listing.
pub async fn list_users(pool: &PgPool) -> Result<Vec<UserSummary>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, username, is_admin, created_at FROM users ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let id: Uuid = r.try_get("id")?;
            Ok(UserSummary {
                id: id.to_string(),
                username: r.try_get("username")?,
                is_admin: r.try_get("is_admin")?,
                created_at: r.try_get::<DateTime<Utc>, _>("created_at")?.to_rfc3339(),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?)
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
    qb.push(" WHERE p.deleted_at IS NULL");
    qb.push(" GROUP BY p.id, s.created_at ORDER BY s.created_at DESC");
    let rows = qb.build().fetch_all(pool).await?;
    rows.iter().map(summary_from_row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amenities_text_is_order_insensitive() {
        assert_eq!(amenities_text(&["coffee", "restrooms"]), "coffee,restrooms");
        assert_eq!(amenities_text(&["restrooms", "coffee"]), "coffee,restrooms");
        assert_eq!(amenities_text::<&str>(&[]), "");
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("camber"), "%camber%");
        assert_eq!(like_pattern("50% off_deal"), "%50\\% off\\_deal%");
        assert_eq!(like_pattern("back\\slash"), "%back\\\\slash%");
    }
}
