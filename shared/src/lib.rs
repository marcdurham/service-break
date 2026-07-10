//! Types and pure logic shared between the Actix backend and the Yew frontend.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const EARTH_RADIUS_MI: f64 = 3958.8;

/// Great-circle distance between two coordinates, in miles.
pub fn haversine_mi(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlng / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_MI * a.sqrt().min(1.0).asin()
}

/// Parses free text as a "lat, lng" coordinate pair, validating ranges.
pub fn parse_latlng(text: &str) -> Option<(f64, f64)> {
    let (lat, lng) = text.split_once(',')?;
    let lat: f64 = lat.trim().parse().ok()?;
    let lng: f64 = lng.trim().parse().ok()?;
    if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lng) {
        Some((lat, lng))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceType {
    Coffee,
    Grocery,
    Bookstore,
    Gas,
    Park,
    Restroom,
    Other,
}

impl PlaceType {
    pub const ALL: [PlaceType; 7] = [
        PlaceType::Coffee,
        PlaceType::Grocery,
        PlaceType::Bookstore,
        PlaceType::Gas,
        PlaceType::Park,
        PlaceType::Restroom,
        PlaceType::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PlaceType::Coffee => "coffee",
            PlaceType::Grocery => "grocery",
            PlaceType::Bookstore => "bookstore",
            PlaceType::Gas => "gas",
            PlaceType::Park => "park",
            PlaceType::Restroom => "restroom",
            PlaceType::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PlaceType::Coffee => "Coffee shop",
            PlaceType::Grocery => "Grocery",
            PlaceType::Bookstore => "Bookstore",
            PlaceType::Gas => "Gas station",
            PlaceType::Park => "Park",
            PlaceType::Restroom => "Public restroom",
            PlaceType::Other => "Other",
        }
    }

    /// Material Symbols icon name used in the UI.
    pub fn icon(self) -> &'static str {
        match self {
            PlaceType::Coffee => "local_cafe",
            PlaceType::Grocery => "local_grocery_store",
            PlaceType::Bookstore => "menu_book",
            PlaceType::Gas => "local_gas_station",
            PlaceType::Park => "park",
            PlaceType::Restroom => "wc",
            PlaceType::Other => "place",
        }
    }

    /// Brand color for pins and badges, from the design mockup.
    pub fn color(self) -> &'static str {
        match self {
            PlaceType::Coffee => "#6f4e37",
            PlaceType::Grocery => "#6f8256",
            PlaceType::Bookstore => "#9b6a7d",
            PlaceType::Gas => "#b5533f",
            PlaceType::Park => "#5c7a4a",
            PlaceType::Restroom => "#4f7a86",
            PlaceType::Other => "#8a7565",
        }
    }
}

impl fmt::Display for PlaceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PlaceType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PlaceType::ALL
            .into_iter()
            .find(|t| t.as_str() == s)
            .ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parking {
    Easy,
    Street,
    None,
}

impl Parking {
    pub fn as_str(self) -> &'static str {
        match self {
            Parking::Easy => "easy",
            Parking::Street => "street",
            Parking::None => "none",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Parking::Easy => "Easy lot",
            Parking::Street => "Street",
            Parking::None => "No parking",
        }
    }
}

impl fmt::Display for Parking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Parking {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "easy" => Ok(Parking::Easy),
            "street" => Ok(Parking::Street),
            "none" => Ok(Parking::None),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    #[default]
    Distance,
    Cleanliness,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceSummary {
    pub id: Uuid,
    pub name: String,
    pub place_type: PlaceType,
    pub lat: f64,
    pub lng: f64,
    pub address: String,
    pub door_ft: i32,
    pub parking: Parking,
    pub purchase_required: bool,
    pub code_required: bool,
    pub clean_avg: Option<f64>,
    pub review_count: i64,
    pub distance_mi: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceDetail {
    #[serde(flatten)]
    pub summary: PlaceSummary,
    pub door_note: String,
    pub hours: Option<String>,
    pub reviews: Vec<Review>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Review {
    pub id: Uuid,
    pub author: String,
    pub clean: i16,
    pub text: String,
    pub created_at: String,
    pub time_ago: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewPlace {
    pub device_id: String,
    pub name: String,
    pub place_type: PlaceType,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
    #[serde(default)]
    pub address: Option<String>,
    pub clean: i16,
    pub door_ft: i32,
    #[serde(default)]
    pub door_note: String,
    pub parking: Parking,
    #[serde(default)]
    pub purchase_required: bool,
    #[serde(default)]
    pub code_required: bool,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewReview {
    pub device_id: String,
    pub clean: i16,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeocodeResult {
    pub lat: f64,
    pub lng: f64,
    pub display_name: String,
}

/// Percent-encodes a string for use as a URL query value.
pub fn encode_query_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Query parameters for `GET /api/places`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PlacesQuery {
    /// Free-text search over place names and addresses.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
    #[serde(default)]
    pub radius_mi: Option<f64>,
    /// Comma-separated list of [`PlaceType`] strings.
    #[serde(default)]
    pub types: Option<String>,
    #[serde(default)]
    pub clean_min: Option<f64>,
    #[serde(default)]
    pub no_purchase: Option<bool>,
    #[serde(default)]
    pub has_parking: Option<bool>,
    #[serde(default)]
    pub sort: Option<SortBy>,
}

impl PlacesQuery {
    pub fn parsed_types(&self) -> Vec<PlaceType> {
        self.types
            .as_deref()
            .unwrap_or("")
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    }

    /// Serializes to a URL query string (no leading `?`).
    pub fn to_query_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(q) = self.q.as_deref().filter(|q| !q.trim().is_empty()) {
            parts.push(format!("q={}", encode_query_component(q.trim())));
        }
        if let Some(v) = self.lat {
            parts.push(format!("lat={v}"));
        }
        if let Some(v) = self.lng {
            parts.push(format!("lng={v}"));
        }
        if let Some(v) = self.radius_mi {
            parts.push(format!("radius_mi={v}"));
        }
        if let Some(t) = self.types.as_deref().filter(|t| !t.is_empty()) {
            parts.push(format!("types={t}"));
        }
        if let Some(v) = self.clean_min {
            parts.push(format!("clean_min={v}"));
        }
        if let Some(v) = self.no_purchase {
            parts.push(format!("no_purchase={v}"));
        }
        if let Some(v) = self.has_parking {
            parts.push(format!("has_parking={v}"));
        }
        if let Some(SortBy::Cleanliness) = self.sort {
            parts.push("sort=cleanliness".to_owned());
        }
        parts.join("&")
    }
}

/// Formats a coordinate pair as `"lat, lng"` text that [`parse_latlng`]
/// accepts back, e.g. for prefilling an address field.
pub fn fmt_latlng(lat: f64, lng: f64) -> String {
    format!("{lat:.6}, {lng:.6}")
}

/// Formats a distance in miles for display, e.g. `"0.2"`.
pub fn fmt_distance_mi(mi: f64) -> String {
    format!("{mi:.1}")
}

/// Short label for how far the bathroom is from the entrance.
pub fn door_short(door_ft: i32) -> String {
    if door_ft <= 0 {
        "At entrance".to_owned()
    } else {
        format!("{door_ft} ft")
    }
}

/// Display name derived from an anonymous device id, e.g. `"Scout 3f9a"`.
pub fn scout_name(device_id: &str) -> String {
    let tag: String = device_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(4)
        .collect();
    if tag.is_empty() {
        "Scout".to_owned()
    } else {
        format!("Scout {tag}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_one_degree_latitude_is_about_69_miles() {
        let d = haversine_mi(47.0, -122.0, 48.0, -122.0);
        assert!((d - 69.1).abs() < 0.5, "got {d}");
    }

    #[test]
    fn haversine_zero_distance() {
        assert!(haversine_mi(47.6, -122.3, 47.6, -122.3).abs() < 1e-9);
    }

    #[test]
    fn parse_latlng_accepts_valid_pairs() {
        assert_eq!(
            parse_latlng("37.7749, -122.4194"),
            Some((37.7749, -122.4194))
        );
        assert_eq!(parse_latlng(" 0 , 0 "), Some((0.0, 0.0)));
    }

    #[test]
    fn parse_latlng_rejects_garbage_and_out_of_range() {
        assert_eq!(parse_latlng("214 Maple Ave"), None);
        assert_eq!(parse_latlng("91.0, 10.0"), None);
        assert_eq!(parse_latlng("45.0, 181.0"), None);
        assert_eq!(parse_latlng(""), None);
    }

    #[test]
    fn fmt_latlng_round_trips_through_parse() {
        let text = fmt_latlng(47.6097, -122.3422);
        assert_eq!(text, "47.609700, -122.342200");
        assert_eq!(parse_latlng(&text), Some((47.6097, -122.3422)));
    }

    #[test]
    fn place_type_serde_round_trip() {
        for t in PlaceType::ALL {
            let json = serde_json::to_string(&t).expect("serialize");
            assert_eq!(json, format!("\"{}\"", t.as_str()));
            let back: PlaceType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, t);
            assert_eq!(t.as_str().parse::<PlaceType>(), Ok(t));
        }
    }

    #[test]
    fn parking_round_trip() {
        for p in [Parking::Easy, Parking::Street, Parking::None] {
            assert_eq!(p.as_str().parse::<Parking>(), Ok(p));
        }
        assert!("valet".parse::<Parking>().is_err());
    }

    #[test]
    fn places_query_round_trips_through_query_string() {
        let q = PlacesQuery {
            q: None,
            lat: Some(47.6),
            lng: Some(-122.3),
            radius_mi: Some(5.0),
            types: Some("coffee,park".to_owned()),
            clean_min: Some(4.0),
            no_purchase: Some(true),
            has_parking: None,
            sort: Some(SortBy::Cleanliness),
        };
        let qs = q.to_query_string();
        assert!(qs.contains("types=coffee,park"));
        assert!(qs.contains("sort=cleanliness"));
        assert!(!qs.contains("has_parking"));
        assert_eq!(q.parsed_types(), vec![PlaceType::Coffee, PlaceType::Park]);
    }

    #[test]
    fn encode_query_component_escapes_reserved_chars() {
        assert_eq!(encode_query_component("camber coffee"), "camber%20coffee");
        assert_eq!(encode_query_component("a&b=c?"), "a%26b%3Dc%3F");
        assert_eq!(encode_query_component("plain-text_1.0~"), "plain-text_1.0~");
    }

    #[test]
    fn query_string_includes_encoded_search_and_skips_blank() {
        let q = PlacesQuery {
            q: Some("elm st park".to_owned()),
            ..PlacesQuery::default()
        };
        assert_eq!(q.to_query_string(), "q=elm%20st%20park");

        let blank = PlacesQuery {
            q: Some("   ".to_owned()),
            ..PlacesQuery::default()
        };
        assert_eq!(blank.to_query_string(), "");
    }

    #[test]
    fn parsed_types_skips_unknown_entries() {
        let q = PlacesQuery {
            types: Some("coffee,unknown,gas".to_owned()),
            ..PlacesQuery::default()
        };
        assert_eq!(q.parsed_types(), vec![PlaceType::Coffee, PlaceType::Gas]);
    }

    #[test]
    fn door_short_labels() {
        assert_eq!(door_short(0), "At entrance");
        assert_eq!(door_short(40), "40 ft");
    }

    #[test]
    fn scout_name_from_device_id() {
        assert_eq!(scout_name("3f9a2b-xyz"), "Scout 3f9a");
        assert_eq!(scout_name(""), "Scout");
    }

    #[test]
    fn place_detail_flattens_summary_fields() {
        let detail = PlaceDetail {
            summary: PlaceSummary {
                id: Uuid::nil(),
                name: "Camber Coffee".to_owned(),
                place_type: PlaceType::Coffee,
                lat: 47.6,
                lng: -122.3,
                address: "214 Maple Ave".to_owned(),
                door_ft: 15,
                parking: Parking::Street,
                purchase_required: true,
                code_required: true,
                clean_avg: Some(4.8),
                review_count: 2,
                distance_mi: Some(0.2),
            },
            door_note: "Right past the counter".to_owned(),
            hours: None,
            reviews: vec![],
        };
        let json = serde_json::to_value(&detail).expect("serialize");
        assert_eq!(json["name"], "Camber Coffee");
        assert_eq!(json["door_note"], "Right past the counter");
        let back: PlaceDetail = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, detail);
    }
}
