//! Resolves a pasted Google Maps link (typically a `maps.app.goo.gl` short
//! link) to a place name and coordinates, by following its redirect and
//! parsing the canonical `google.com/maps/place/...` URL it lands on.

use crate::error::ApiError;

/// Hosts we're willing to fetch. Keeps `/api/maps-link` from being usable as
/// an open URL fetcher (SSRF) for arbitrary hosts.
fn is_allowed_host(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some(
                "maps.app.goo.gl"
                    | "goo.gl"
                    | "www.google.com"
                    | "google.com"
                    | "maps.google.com"
            )
        )
}

/// Percent-decodes a URL path segment, also treating `+` as a space the way
/// Google encodes the place-name segment.
fn decode_segment(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(byte) => {
                    out.push(byte);
                    i += 3;
                }
                Err(_) => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The precise marker coordinates embedded as `!3d<lat>!4d<lng>` in the
/// `data=` blob. This is the actual pin location; the `@lat,lng,zoom`
/// segment is only the map camera position and can be noticeably off.
fn find_marker_coords(url: &str) -> Option<(f64, f64)> {
    let after_3d = &url[url.find("!3d")? + 3..];
    let lat_end = after_3d.find(['!', '?']).unwrap_or(after_3d.len());
    let lat: f64 = after_3d[..lat_end].parse().ok()?;
    let after_lat = &after_3d[lat_end..];
    let after_4d = &after_lat[after_lat.find("!4d")? + 3..];
    let lng_end = after_4d.find(['!', '?']).unwrap_or(after_4d.len());
    let lng: f64 = after_4d[..lng_end].parse().ok()?;
    Some((lat, lng))
}

/// The `@lat,lng,zoom` camera-position segment, used when no precise marker
/// coordinates are present.
fn find_camera_coords(segments: &[&str]) -> Option<(f64, f64)> {
    segments.iter().find_map(|seg| {
        let rest = seg.strip_prefix('@')?;
        let mut parts = rest.split(',');
        let lat: f64 = parts.next()?.parse().ok()?;
        let lng: f64 = parts.next()?.parse().ok()?;
        Some((lat, lng))
    })
}

/// Extracts a place name and coordinates from a canonical Google Maps URL
/// of the form `.../maps/place/<name>/@<lat>,<lng>,<zoom>z/data=...!3d<lat>!4d<lng>...`.
pub fn parse_place_url(url: &reqwest::Url) -> Option<(String, f64, f64)> {
    let segments: Vec<&str> = url.path_segments()?.collect();
    let place_idx = segments.iter().position(|s| *s == "place")?;
    let name = decode_segment(segments.get(place_idx + 1)?);
    if name.trim().is_empty() {
        return None;
    }
    let (lat, lng) = find_marker_coords(url.as_str()).or_else(|| find_camera_coords(&segments))?;
    Some((name, lat, lng))
}

/// Fetches `link`, follows its redirect(s), and extracts the place name and
/// coordinates from where it lands.
pub async fn resolve(http: &reqwest::Client, link: &str) -> Result<(String, f64, f64), ApiError> {
    let parsed = reqwest::Url::parse(link)
        .map_err(|_| ApiError::BadRequest("not a valid URL".to_owned()))?;
    if !is_allowed_host(&parsed) {
        return Err(ApiError::BadRequest("only Google Maps links are supported".to_owned()));
    }
    let resp = http
        .get(parsed)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await?
        .error_for_status()?;
    let final_url = resp.url().clone();
    parse_place_url(&final_url)
        .ok_or_else(|| ApiError::BadRequest("couldn't find a place in that link".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_precise_marker_coords_over_camera_position() {
        let url = reqwest::Url::parse(
            "https://www.google.com/maps/place/Shawn+O'Donnell's+American+Grill+and+Irish+Pub/\
             @47.8643712,-122.2551304,14z/data=!4m6!3m5!1s0x54900690c4509883:0x72ce85c35a200b9c\
             !8m2!3d47.8815461!4d-122.2301566!16s%2Fg%2F1whdj12z?entry=tts",
        )
        .expect("parse");
        let (name, lat, lng) = parse_place_url(&url).expect("parsed");
        assert_eq!(name, "Shawn O'Donnell's American Grill and Irish Pub");
        assert!((lat - 47.8815461).abs() < 1e-9);
        assert!((lng + 122.2301566).abs() < 1e-9);
    }

    #[test]
    fn falls_back_to_camera_position_without_marker_coords() {
        let url = reqwest::Url::parse(
            "https://www.google.com/maps/place/Caf%C3%A9+Ladro/@47.6205855,-122.3212972,17z",
        )
        .expect("parse");
        let (name, lat, lng) = parse_place_url(&url).expect("parsed");
        assert_eq!(name, "Café Ladro");
        assert!((lat - 47.6205855).abs() < 1e-9);
        assert!((lng + 122.3212972).abs() < 1e-9);
    }

    #[test]
    fn non_place_url_is_none() {
        let url = reqwest::Url::parse("https://www.google.com/maps/search/coffee").expect("parse");
        assert!(parse_place_url(&url).is_none());
    }

    #[test]
    fn rejects_disallowed_hosts() {
        let url = reqwest::Url::parse("https://evil.example.com/maps/place/x/@1,2,3z").expect("parse");
        assert!(!is_allowed_host(&url));
    }
}
