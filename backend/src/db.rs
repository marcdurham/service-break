use chrono::{DateTime, Utc};
use shared::{
    Amenity, Parking, PlaceDetail, PlaceSummary, PlaceType, PlacesQuery, Requirement, Review,
    SortBy,
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
     avg(r.clean)::float8 AS clean_avg, count(r.id) AS review_count, ";

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
        "SELECT id, device_id, clean, text, created_at FROM reviews \
         WHERE place_id = $1 ORDER BY created_at DESC",
    )
    .bind(place_id)
    .fetch_all(pool)
    .await?;
    let now = Utc::now();
    rows.iter()
        .map(|row| {
            let device_id: String = row.try_get("device_id")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;
            Ok(Review {
                id: row.try_get("id")?,
                author: shared::scout_name(&device_id),
                clean: row.try_get("clean")?,
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
}

pub async fn insert_place(pool: &PgPool, p: &InsertPlace) -> Result<Uuid, ApiError> {
    let amenities: Vec<&str> = p.amenities.iter().map(|a| a.as_str()).collect();
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO places (name, place_type, lat, lng, address, door_ft, door_note, \
         parking, purchase_required, code_required, amenities, device_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) RETURNING id",
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
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn insert_review(
    pool: &PgPool,
    place_id: Uuid,
    device_id: &str,
    clean: i16,
    text: &str,
) -> Result<Uuid, ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM places WHERE id = $1)")
        .bind(place_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO reviews (place_id, device_id, clean, text) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(place_id)
    .bind(device_id)
    .bind(clean)
    .bind(text)
    .fetch_one(pool)
    .await?;
    Ok(id)
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
