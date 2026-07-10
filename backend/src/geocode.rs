use serde::Deserialize;
use shared::GeocodeResult;

use crate::error::ApiError;

/// One hit from Nominatim's `/search?format=jsonv2` response.
/// Nominatim returns coordinates as strings.
#[derive(Debug, Deserialize)]
struct NominatimHit {
    lat: String,
    lon: String,
    display_name: String,
}

pub fn parse_nominatim_response(body: &str) -> Result<Option<GeocodeResult>, ApiError> {
    let hits: Vec<NominatimHit> = serde_json::from_str(body)
        .map_err(|e| ApiError::BadRequest(format!("unexpected geocoder response: {e}")))?;
    let Some(hit) = hits.into_iter().next() else {
        return Ok(None);
    };
    let lat: f64 = hit
        .lat
        .parse()
        .map_err(|_| ApiError::BadRequest("geocoder returned invalid latitude".to_owned()))?;
    let lng: f64 = hit
        .lon
        .parse()
        .map_err(|_| ApiError::BadRequest("geocoder returned invalid longitude".to_owned()))?;
    Ok(Some(GeocodeResult {
        lat,
        lng,
        display_name: hit.display_name,
    }))
}

/// Geocodes a free-text query via Nominatim (OpenStreetMap).
pub async fn geocode(
    http: &reqwest::Client,
    base_url: &str,
    query: &str,
) -> Result<Option<GeocodeResult>, ApiError> {
    let url = format!("{base_url}/search");
    let body = http
        .get(&url)
        .query(&[("q", query), ("format", "jsonv2"), ("limit", "1")])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_nominatim_response(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_nominatim_hit() {
        let body = r#"[{"place_id":123,"lat":"47.6097","lon":"-122.3422","display_name":"Pike Place Market, Seattle","category":"amenity"}]"#;
        let hit = parse_nominatim_response(body).expect("parse").expect("hit");
        assert!((hit.lat - 47.6097).abs() < 1e-9);
        assert!((hit.lng + 122.3422).abs() < 1e-9);
        assert_eq!(hit.display_name, "Pike Place Market, Seattle");
    }

    #[test]
    fn empty_result_is_none() {
        assert_eq!(parse_nominatim_response("[]").expect("parse"), None);
    }

    #[test]
    fn invalid_json_is_bad_request() {
        assert!(parse_nominatim_response("not json").is_err());
    }
}
