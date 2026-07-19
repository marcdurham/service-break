//! Proxies the Leaflet map's raster tiles through the backend, caching each
//! tile in Postgres for 30 days. The frontend requests `/api/tiles/{z}/{x}/
//! {y}.png` instead of hitting tile.openstreetmap.org directly, so a tile is
//! fetched from OSM at most once per TTL no matter how many devices view it
//! (and the OSM tile usage policy's caching requirement is satisfied
//! server-side). A stale cached copy is served when the tile server is
//! unreachable rather than leaving holes in the map.

use actix_web::web::{Data, Path, ServiceConfig};
use actix_web::{get, HttpResponse};
use sqlx::{PgPool, Row};

use crate::error::ApiError;
use crate::AppState;

/// Highest zoom level the map UI offers (Leaflet `maxZoom` in index.html).
const MAX_ZOOM: u32 = 19;

/// `Cache-Control` sent to browsers: one day, so devices refresh well
/// before the server's own 30-day copy can drift far out of date.
const BROWSER_MAX_AGE_SECS: u32 = 86_400;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(get_tile);
}

/// True when z/x/y address a real slippy-map tile: zoom within the UI's
/// range and x/y inside the 2^z × 2^z grid for that zoom.
fn valid_tile(z: u32, x: u32, y: u32) -> bool {
    z <= MAX_ZOOM && x < (1 << z) && y < (1 << z)
}

struct CachedTile {
    body: Vec<u8>,
    content_type: String,
    fresh: bool,
}

/// The cached tile, if any, with `fresh` false once past the 30-day TTL.
async fn cached_tile(
    pool: &PgPool,
    z: i32,
    x: i32,
    y: i32,
) -> Result<Option<CachedTile>, ApiError> {
    let row = sqlx::query(
        "SELECT body, content_type, fetched_at > now() - interval '30 days' AS fresh \
         FROM map_tiles WHERE z = $1 AND x = $2 AND y = $3",
    )
    .bind(z)
    .bind(x)
    .bind(y)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        Ok(CachedTile {
            body: row.try_get("body")?,
            content_type: row.try_get("content_type")?,
            fresh: row.try_get("fresh")?,
        })
    })
    .transpose()
}

/// Stores a freshly fetched tile (replacing any expired copy).
async fn store_tile(
    pool: &PgPool,
    z: i32,
    x: i32,
    y: i32,
    content_type: &str,
    body: &[u8],
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO map_tiles (z, x, y, body, content_type, fetched_at) \
         VALUES ($1, $2, $3, $4, $5, now()) \
         ON CONFLICT (z, x, y) DO UPDATE SET body = excluded.body, \
         content_type = excluded.content_type, fetched_at = now()",
    )
    .bind(z)
    .bind(x)
    .bind(y)
    .bind(body)
    .bind(content_type)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetches one tile from the upstream tile server.
async fn fetch_tile(
    http: &reqwest::Client,
    base_url: &str,
    z: u32,
    x: u32,
    y: u32,
) -> Result<(String, Vec<u8>), ApiError> {
    let resp = http
        .get(format!("{base_url}/{z}/{x}/{y}.png"))
        .send()
        .await?
        .error_for_status()?;
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_owned();
    Ok((content_type, resp.bytes().await?.to_vec()))
}

fn tile_response(content_type: &str, body: Vec<u8>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type(content_type)
        .insert_header(("Cache-Control", format!("public, max-age={BROWSER_MAX_AGE_SECS}")))
        .body(body)
}

#[get("/api/tiles/{z}/{x}/{y}.png")]
async fn get_tile(
    state: Data<AppState>,
    path: Path<(u32, u32, u32)>,
) -> Result<HttpResponse, ApiError> {
    let (z, x, y) = path.into_inner();
    if !valid_tile(z, x, y) {
        return Err(ApiError::BadRequest("tile coordinates out of range".to_owned()));
    }
    let (zi, xi, yi) = (z as i32, x as i32, y as i32);

    let cached = cached_tile(&state.pool, zi, xi, yi).await?;
    if let Some(tile) = &cached {
        if tile.fresh {
            return Ok(tile_response(&tile.content_type, tile.body.clone()));
        }
    }

    match fetch_tile(&state.http, &state.tile_url, z, x, y).await {
        Ok((content_type, body)) => {
            store_tile(&state.pool, zi, xi, yi, &content_type, &body).await?;
            Ok(tile_response(&content_type, body))
        }
        // An expired copy beats a hole in the map when OSM is unreachable.
        Err(err) => match cached {
            Some(tile) => {
                tracing::warn!(error = %err, z, x, y, "tile refresh failed; serving stale copy");
                Ok(tile_response(&tile.content_type, tile.body))
            }
            None => Err(err),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_tile_accepts_grid_bounds_per_zoom() {
        assert!(valid_tile(0, 0, 0));
        assert!(valid_tile(19, (1 << 19) - 1, (1 << 19) - 1));
        assert!(valid_tile(5, 31, 31));
    }

    #[test]
    fn valid_tile_rejects_out_of_range_coordinates() {
        assert!(!valid_tile(20, 0, 0), "zoom past UI max");
        assert!(!valid_tile(0, 1, 0), "x outside 1x1 grid at z0");
        assert!(!valid_tile(0, 0, 1), "y outside 1x1 grid at z0");
        assert!(!valid_tile(5, 32, 0), "x outside 32x32 grid at z5");
    }
}
