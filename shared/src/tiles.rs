//! A small geohash implementation used purely as a cache-bucketing scheme
//! for Overpass POI tile fetches. Unrelated to Leaflet's own map tile
//! imagery — these "tiles" are just fixed-size lat/lng cells used to dedupe
//! and TTL-track our own Overpass API calls.

use crate::BBox;

/// Tile precision in geohash characters: ~1.2km × 0.6km cells near the
/// equator, sized for a typical mobile-map viewport (a few km across) to
/// overlap a small, bounded number of tiles per pan.
pub const OVERPASS_TILE_PRECISION: usize = 6;

const BASE32: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// Encodes a lat/lng pair into a geohash string of `precision` characters.
pub fn geohash_encode(lat: f64, lng: f64, precision: usize) -> String {
    let mut lat_range = (-90.0_f64, 90.0_f64);
    let mut lng_range = (-180.0_f64, 180.0_f64);
    let mut out = String::with_capacity(precision);
    let mut bit = 0u8;
    let mut ch = 0u8;
    let mut even = true;
    while out.len() < precision {
        if even {
            let mid = (lng_range.0 + lng_range.1) / 2.0;
            if lng >= mid {
                ch = (ch << 1) | 1;
                lng_range.0 = mid;
            } else {
                ch <<= 1;
                lng_range.1 = mid;
            }
        } else {
            let mid = (lat_range.0 + lat_range.1) / 2.0;
            if lat >= mid {
                ch = (ch << 1) | 1;
                lat_range.0 = mid;
            } else {
                ch <<= 1;
                lat_range.1 = mid;
            }
        }
        even = !even;
        bit += 1;
        if bit == 5 {
            out.push(BASE32[ch as usize] as char);
            bit = 0;
            ch = 0;
        }
    }
    out
}

/// Decodes a geohash string back to its cell's lat/lng bounds:
/// `(min_lat, min_lng, max_lat, max_lng)`. Unknown characters are treated
/// as `0` rather than panicking, since tile ids only ever come from
/// [`geohash_encode`] or trusted storage.
pub fn tile_bounds(tile_id: &str) -> (f64, f64, f64, f64) {
    let mut lat_range = (-90.0_f64, 90.0_f64);
    let mut lng_range = (-180.0_f64, 180.0_f64);
    let mut even = true;
    for c in tile_id.chars() {
        let idx = BASE32.iter().position(|&b| b as char == c).unwrap_or(0);
        for shift in (0..5).rev() {
            let bit = (idx >> shift) & 1;
            if even {
                let mid = (lng_range.0 + lng_range.1) / 2.0;
                if bit == 1 {
                    lng_range.0 = mid;
                } else {
                    lng_range.1 = mid;
                }
            } else {
                let mid = (lat_range.0 + lat_range.1) / 2.0;
                if bit == 1 {
                    lat_range.0 = mid;
                } else {
                    lat_range.1 = mid;
                }
            }
            even = !even;
        }
    }
    (lat_range.0, lng_range.0, lat_range.1, lng_range.1)
}

/// The deduplicated, sorted set of geohash tile ids at `precision` that
/// cover `bbox` — steps across the box in cell-sized increments so every
/// overlapping cell is visited at least once.
pub fn tiles_covering_bbox(bbox: &BBox, precision: usize) -> Vec<String> {
    let mut tiles = std::collections::BTreeSet::new();

    let corner = geohash_encode(bbox.min_lat, bbox.min_lng, precision);
    let (clat0, clng0, clat1, clng1) = tile_bounds(&corner);
    let lat_step = (clat1 - clat0).max(1e-9);
    let lng_step = (clng1 - clng0).max(1e-9);

    let mut lat = bbox.min_lat;
    loop {
        let mut lng = bbox.min_lng;
        loop {
            tiles.insert(geohash_encode(lat, lng, precision));
            if lng >= bbox.max_lng {
                break;
            }
            lng = (lng + lng_step).min(bbox.max_lng);
        }
        if lat >= bbox.max_lat {
            break;
        }
        lat = (lat + lat_step).min(bbox.max_lat);
    }
    tiles.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geohash_encode_matches_canonical_example() {
        // The standard geohash.org worked example.
        assert_eq!(geohash_encode(42.6, -5.6, 5), "ezs42");
    }

    #[test]
    fn tile_bounds_contains_the_encoded_point() {
        let (lat, lng) = (47.6097, -122.3422);
        let tile = geohash_encode(lat, lng, OVERPASS_TILE_PRECISION);
        let (min_lat, min_lng, max_lat, max_lng) = tile_bounds(&tile);
        assert!(min_lat <= lat && lat <= max_lat, "{min_lat} <= {lat} <= {max_lat}");
        assert!(min_lng <= lng && lng <= max_lng, "{min_lng} <= {lng} <= {max_lng}");
    }

    #[test]
    fn tiles_covering_bbox_is_deduplicated_and_covers_corners() {
        let bbox = BBox {
            min_lat: 47.60,
            min_lng: -122.35,
            max_lat: 47.62,
            max_lng: -122.32,
        };
        let tiles = tiles_covering_bbox(&bbox, OVERPASS_TILE_PRECISION);
        assert!(!tiles.is_empty());
        let mut sorted = tiles.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(tiles, sorted, "tiles must be deduplicated");

        let min_corner = geohash_encode(bbox.min_lat, bbox.min_lng, OVERPASS_TILE_PRECISION);
        let max_corner = geohash_encode(bbox.max_lat, bbox.max_lng, OVERPASS_TILE_PRECISION);
        assert!(tiles.contains(&min_corner));
        assert!(tiles.contains(&max_corner));
    }

    #[test]
    fn tiles_covering_a_point_bbox_returns_one_tile() {
        let bbox = BBox {
            min_lat: 47.6097,
            min_lng: -122.3422,
            max_lat: 47.6097,
            max_lng: -122.3422,
        };
        let tiles = tiles_covering_bbox(&bbox, OVERPASS_TILE_PRECISION);
        assert_eq!(tiles.len(), 1);
    }
}
