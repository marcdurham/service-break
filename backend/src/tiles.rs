//! Proxies the Leaflet map's raster tiles through the backend, caching each
//! tile in a disk-based LRU cache (see `src/tile_cache.rs`) for 30 days. The
//! frontend requests `/api/tiles/{z}/{x}/{y}.png` instead of hitting
//! tile.openstreetmap.org directly, so a tile is fetched from OSM at most
//! once per TTL no matter how many devices view it (and the OSM tile usage
//! policy's caching requirement is satisfied server-side). A stale cached
//! copy is served when the tile server is unreachable rather than leaving
//! holes in the map.

use actix_web::web::{Data, Path, ServiceConfig};
use actix_web::{get, HttpResponse};

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

/// Fetches one tile from the upstream tile server. The endpoint only serves
/// `.png` tiles, so the body is always PNG bytes.
async fn fetch_tile(
    http: &reqwest::Client,
    base_url: &str,
    z: u32,
    x: u32,
    y: u32,
) -> Result<Vec<u8>, ApiError> {
    let resp = http
        .get(format!("{base_url}/{z}/{x}/{y}.png"))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.bytes().await?.to_vec())
}

fn tile_response(body: Vec<u8>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/png")
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

    let cached = state.tile_cache.get(z, x, y).await;
    if let Some(tile) = &cached {
        if tile.fresh {
            tracing::debug!(z, x, y, "tile cache hit");
            return Ok(tile_response(tile.body.clone()));
        }
    }
    tracing::debug!(z, x, y, stale = cached.is_some(), "tile cache miss; fetching upstream");

    match fetch_tile(&state.http, &state.tile_url, z, x, y).await {
        Ok(body) => {
            // A cache write failure only costs a re-fetch next time; the
            // tile in hand is still good, so serve it either way.
            if let Err(err) = state.tile_cache.insert(z, x, y, &body).await {
                tracing::warn!(error = %err, z, x, y, "failed to cache tile on disk");
            }
            Ok(tile_response(body))
        }
        // An expired copy beats a hole in the map when OSM is unreachable.
        Err(err) => match cached {
            Some(tile) => {
                tracing::warn!(error = %err, z, x, y, "tile refresh failed; serving stale copy");
                Ok(tile_response(tile.body))
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
