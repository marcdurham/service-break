//! Fetches and caches points of interest from the Overpass API (a query
//! service over OpenStreetMap's raw data — distinct from the Leaflet map
//! tile imagery, which comes from a separate OSM tile server). Follows the
//! same pure-parse / async-fetch split as `geocode.rs`, so the parsing logic
//! is unit-testable without a live network call.

use std::collections::HashMap;

use serde::Deserialize;
use shared::{BBox, PlaceType};
use sqlx::PgPool;

use crate::error::ApiError;

/// A point of interest parsed from an Overpass response, ready to cache.
#[derive(Debug, Clone, PartialEq)]
pub struct RawPoi {
    pub id: String,
    pub name: String,
    pub place_type: PlaceType,
    pub lat: f64,
    pub lng: f64,
    pub address: String,
    pub tags: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OverpassCenter {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(rename = "type")]
    kind: String,
    id: u64,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    center: Option<OverpassCenter>,
    #[serde(default)]
    tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct OverpassResponseRaw {
    elements: Vec<OverpassElement>,
}

/// Builds the Overpass QL query for one bbox, covering every target
/// category (fast food, cafés, stores, malls, parks). `out center 200;` is
/// a generous Overpass-side cap — kept above the app's own final cap of 100
/// so the "100 nearest to center" pass isn't starved by Overpass's
/// non-distance element ordering, while still bounding worst-case payload
/// size in dense urban cores.
pub fn build_query(bbox: &BBox) -> String {
    let bb = format!("{},{},{},{}", bbox.min_lat, bbox.min_lng, bbox.max_lat, bbox.max_lng);
    let filters = [
        format!("node[\"amenity\"=\"fast_food\"]({bb});"),
        format!("way[\"amenity\"=\"fast_food\"]({bb});"),
        format!("node[\"amenity\"=\"cafe\"]({bb});"),
        format!("way[\"amenity\"=\"cafe\"]({bb});"),
        format!("node[\"shop\"][\"shop\"!=\"mall\"]({bb});"),
        format!("way[\"shop\"][\"shop\"!=\"mall\"]({bb});"),
        format!("node[\"shop\"=\"mall\"]({bb});"),
        format!("way[\"shop\"=\"mall\"]({bb});"),
        format!("node[\"leisure\"=\"park\"]({bb});"),
        format!("way[\"leisure\"=\"park\"]({bb});"),
    ]
    .join("");
    format!("[out:json][timeout:25];({filters});out center 200;")
}

/// Maps OSM tags to one of the app's [`PlaceType`]s; `None` for anything
/// outside the target categories (shouldn't happen given `build_query`
/// already filters server-side, but kept defensive against tag drift).
fn map_tags_to_place_type(tags: &HashMap<String, String>) -> Option<PlaceType> {
    if tags.get("amenity").map(String::as_str) == Some("fast_food") {
        Some(PlaceType::FastFood)
    } else if tags.get("amenity").map(String::as_str) == Some("cafe") {
        Some(PlaceType::Shop)
    } else if tags.get("shop").map(String::as_str) == Some("mall") {
        Some(PlaceType::Mall)
    } else if tags.contains_key("shop") {
        Some(PlaceType::Store)
    } else if tags.get("leisure").map(String::as_str) == Some("park") {
        Some(PlaceType::Park)
    } else {
        None
    }
}

/// Builds a one-line address from `addr:*` tags, when present.
fn build_address(tags: &HashMap<String, String>) -> String {
    let mut parts = Vec::new();
    match (tags.get("addr:housenumber"), tags.get("addr:street")) {
        (Some(num), Some(street)) => parts.push(format!("{num} {street}")),
        (None, Some(street)) => parts.push(street.clone()),
        _ => {}
    }
    if let Some(city) = tags.get("addr:city") {
        parts.push(city.clone());
    }
    parts.join(", ")
}

/// Parses an Overpass `[out:json]` response body into cache-ready POIs.
/// Pure and network-free, so it's directly unit-testable. Elements with no
/// resolvable coordinates, no `name` tag, or tags outside the target
/// categories are skipped.
pub fn parse_overpass_response(body: &str) -> Result<Vec<RawPoi>, ApiError> {
    let raw: OverpassResponseRaw = serde_json::from_str(body)
        .map_err(|e| ApiError::BadRequest(format!("unexpected Overpass response: {e}")))?;
    let mut pois = Vec::new();
    for el in raw.elements {
        let Some(tags) = el.tags else { continue };
        let Some(name) = tags.get("name").filter(|n| !n.trim().is_empty()) else {
            continue;
        };
        let Some(place_type) = map_tags_to_place_type(&tags) else {
            continue;
        };
        let (lat, lng) = match (el.lat, el.lon, el.center) {
            (Some(lat), Some(lon), _) => (lat, lon),
            (_, _, Some(center)) => (center.lat, center.lon),
            _ => continue,
        };
        pois.push(RawPoi {
            id: format!("{}/{}", el.kind, el.id),
            name: name.clone(),
            place_type,
            lat,
            lng,
            address: build_address(&tags),
            tags: serde_json::to_value(&tags).unwrap_or_default(),
        });
    }
    Ok(pois)
}

/// Fetches every POI Overpass reports within `bbox`.
pub async fn fetch_bbox(
    http: &reqwest::Client,
    base_url: &str,
    bbox: &BBox,
) -> Result<Vec<RawPoi>, ApiError> {
    let query = build_query(bbox);
    let body = http
        .post(base_url)
        .form(&[("data", query)])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_overpass_response(&body)
}

/// Ensures every geohash tile covering `bbox` has been fetched within the
/// last 7 days, fetching and caching the whole bbox in one Overpass call if
/// any covered tile is missing or stale. A no-op (no network call) when
/// everything is already fresh.
pub async fn ensure_bbox_cached(
    pool: &PgPool,
    http: &reqwest::Client,
    overpass_url: &str,
    bbox: &BBox,
) -> Result<(), ApiError> {
    let tiles = shared::tiles::tiles_covering_bbox(bbox, shared::tiles::OVERPASS_TILE_PRECISION);
    let stale = crate::db::stale_overpass_tiles(pool, &tiles).await?;
    if stale.is_empty() {
        return Ok(());
    }

    let pois = fetch_bbox(http, overpass_url, bbox).await?;

    let mut tx = pool.begin().await?;
    for poi in &pois {
        let tile_id =
            shared::tiles::geohash_encode(poi.lat, poi.lng, shared::tiles::OVERPASS_TILE_PRECISION);
        // The POI's own tile might fall just outside the requested bbox's
        // covering set (near an edge) -- ensure it exists before the POI
        // row references it via a foreign key.
        crate::db::upsert_overpass_tile(&mut *tx, &tile_id).await?;
        crate::db::upsert_overpass_poi(&mut *tx, &tile_id, poi).await?;
    }
    // Every requested tile gets a fresh timestamp, even ones that yielded
    // zero POIs, so empty-but-covered areas aren't re-fetched every pan.
    for tile_id in &tiles {
        crate::db::upsert_overpass_tile(&mut *tx, tile_id).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seattle_bbox() -> BBox {
        BBox { min_lat: 47.60, min_lng: -122.35, max_lat: 47.62, max_lng: -122.32 }
    }

    #[test]
    fn build_query_includes_bbox_and_every_category() {
        let q = build_query(&seattle_bbox());
        assert!(q.contains("47.6,-122.35,47.62,-122.32"));
        assert!(q.contains("\"amenity\"=\"fast_food\""));
        assert!(q.contains("\"amenity\"=\"cafe\""));
        assert!(q.contains("\"shop\"=\"mall\""));
        assert!(q.contains("\"shop\"!=\"mall\""));
        assert!(q.contains("\"leisure\"=\"park\""));
        assert!(q.contains("out center 200;"));
    }

    type TagCase = (&'static [(&'static str, &'static str)], Option<PlaceType>);

    #[test]
    fn map_tags_to_place_type_covers_every_target_category() {
        let cases: [TagCase; 6] = [
            (&[("amenity", "fast_food")], Some(PlaceType::FastFood)),
            (&[("amenity", "cafe")], Some(PlaceType::Shop)),
            (&[("shop", "mall")], Some(PlaceType::Mall)),
            (&[("shop", "convenience")], Some(PlaceType::Store)),
            (&[("leisure", "park")], Some(PlaceType::Park)),
            (&[("amenity", "fuel")], None),
        ];
        for (tags, expected) in cases {
            let map: HashMap<String, String> =
                tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            assert_eq!(map_tags_to_place_type(&map), expected, "tags: {tags:?}");
        }
    }

    #[test]
    fn parse_overpass_response_maps_node_and_way_with_center() {
        let body = r#"{
            "elements": [
                {"type": "node", "id": 1, "lat": 47.61, "lon": -122.34,
                 "tags": {"name": "Camber Coffee", "amenity": "cafe",
                          "addr:housenumber": "214", "addr:street": "Maple Ave"}},
                {"type": "way", "id": 2, "center": {"lat": 47.615, "lon": -122.33},
                 "tags": {"name": "Northgate Mall", "shop": "mall"}},
                {"type": "node", "id": 3, "lat": 47.6, "lon": -122.3,
                 "tags": {"amenity": "cafe"}},
                {"type": "node", "id": 4, "lat": 47.6, "lon": -122.3,
                 "tags": {"name": "Random Bench", "amenity": "bench"}}
            ]
        }"#;
        let pois = parse_overpass_response(body).expect("parse");
        assert_eq!(pois.len(), 2);

        let cafe = &pois[0];
        assert_eq!(cafe.id, "node/1");
        assert_eq!(cafe.name, "Camber Coffee");
        assert_eq!(cafe.place_type, PlaceType::Shop);
        assert_eq!(cafe.address, "214 Maple Ave");
        assert!((cafe.lat - 47.61).abs() < 1e-9);

        let mall = &pois[1];
        assert_eq!(mall.id, "way/2");
        assert_eq!(mall.place_type, PlaceType::Mall);
        assert!((mall.lat - 47.615).abs() < 1e-9);
        assert!((mall.lng - (-122.33)).abs() < 1e-9);
    }

    #[test]
    fn parse_overpass_response_rejects_invalid_json() {
        assert!(parse_overpass_response("not json").is_err());
    }
}
