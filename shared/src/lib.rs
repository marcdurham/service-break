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
    Shop,
    Store,
    Mall,
    Park,
    Public,
    Hall,
    Other,
}

impl PlaceType {
    pub const ALL: [PlaceType; 7] = [
        PlaceType::Shop,
        PlaceType::Store,
        PlaceType::Mall,
        PlaceType::Park,
        PlaceType::Public,
        PlaceType::Hall,
        PlaceType::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PlaceType::Shop => "shop",
            PlaceType::Store => "store",
            PlaceType::Mall => "mall",
            PlaceType::Park => "park",
            PlaceType::Public => "public",
            PlaceType::Hall => "hall",
            PlaceType::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PlaceType::Shop => "Café",
            PlaceType::Store => "Store",
            PlaceType::Mall => "Mall",
            PlaceType::Park => "Park",
            PlaceType::Public => "Public",
            PlaceType::Hall => "Hall",
            PlaceType::Other => "Other",
        }
    }

    /// Material Symbols icon name used in the UI.
    pub fn icon(self) -> &'static str {
        match self {
            PlaceType::Shop => "local_cafe",
            PlaceType::Store => "store",
            PlaceType::Mall => "local_mall",
            PlaceType::Park => "park",
            PlaceType::Public => "account_balance",
            PlaceType::Hall => "meeting_room",
            PlaceType::Other => "place",
        }
    }

    /// Brand color for pins and badges, from the design mockup.
    pub fn color(self) -> &'static str {
        match self {
            PlaceType::Shop => "#6f4e37",
            PlaceType::Store => "#6f8256",
            PlaceType::Mall => "#9b6a7d",
            PlaceType::Park => "#5c7a4a",
            PlaceType::Public => "#4f7a86",
            PlaceType::Hall => "#b5533f",
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

/// Something a place has on offer, shown as a multi-select on the add form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Amenity {
    Restrooms,
    Coffee,
    Food,
    Groceries,
    Seating,
    Parking,
}

impl Amenity {
    /// The offerings surfaced as top-level filter chips on the map and
    /// list screens, in display order.
    pub const FEATURED: [Amenity; 4] = [
        Amenity::Restrooms,
        Amenity::Coffee,
        Amenity::Food,
        Amenity::Seating,
    ];

    pub const ALL: [Amenity; 6] = [
        Amenity::Restrooms,
        Amenity::Coffee,
        Amenity::Food,
        Amenity::Groceries,
        Amenity::Seating,
        Amenity::Parking,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Amenity::Restrooms => "restrooms",
            Amenity::Coffee => "coffee",
            Amenity::Food => "food",
            Amenity::Groceries => "groceries",
            Amenity::Seating => "seating",
            Amenity::Parking => "parking",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Amenity::Restrooms => "Restrooms",
            Amenity::Coffee => "Coffee",
            Amenity::Food => "Food",
            Amenity::Groceries => "Groceries",
            Amenity::Seating => "Seating",
            Amenity::Parking => "Parking",
        }
    }

    /// Material Symbols icon name used in the UI.
    pub fn icon(self) -> &'static str {
        match self {
            Amenity::Restrooms => "wc",
            Amenity::Coffee => "local_cafe",
            Amenity::Food => "restaurant",
            Amenity::Groceries => "local_grocery_store",
            Amenity::Seating => "chair",
            Amenity::Parking => "local_parking",
        }
    }
}

impl fmt::Display for Amenity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Amenity {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Amenity::ALL.into_iter().find(|a| a.as_str() == s).ok_or(())
    }
}

/// A rateable aspect of a place. Bathroom cleanliness is required on every
/// review; the other aspects are optional 1-5 scores for places that offer
/// them (see [`Amenity`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aspect {
    Cleanliness,
    Coffee,
    Food,
}

impl Aspect {
    pub const ALL: [Aspect; 3] = [Aspect::Cleanliness, Aspect::Coffee, Aspect::Food];

    pub fn as_str(self) -> &'static str {
        match self {
            Aspect::Cleanliness => "cleanliness",
            Aspect::Coffee => "coffee",
            Aspect::Food => "food",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Aspect::Cleanliness => "Cleanliness",
            Aspect::Coffee => "Coffee",
            Aspect::Food => "Food",
        }
    }

    /// Material Symbols icon name used in the UI.
    pub fn icon(self) -> &'static str {
        match self {
            Aspect::Cleanliness => "mop",
            Aspect::Coffee => "local_cafe",
            Aspect::Food => "restaurant",
        }
    }

    /// Accent color for the aspect's row in the ratings breakdown.
    pub fn color(self) -> &'static str {
        match self {
            Aspect::Cleanliness => "#6f8256",
            Aspect::Coffee => "#6f4e37",
            Aspect::Food => "#c08a4a",
        }
    }
}

impl fmt::Display for Aspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Aspect {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Aspect::ALL.into_iter().find(|a| a.as_str() == s).ok_or(())
    }
}

/// A tri-state answer for questions like "purchase required?" where the
/// scout adding a place may simply not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    Yes,
    No,
    #[default]
    Unknown,
}

impl Requirement {
    pub const ALL: [Requirement; 3] = [Requirement::Yes, Requirement::No, Requirement::Unknown];

    pub fn as_str(self) -> &'static str {
        match self {
            Requirement::Yes => "yes",
            Requirement::No => "no",
            Requirement::Unknown => "unknown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Requirement::Yes => "Yes",
            Requirement::No => "No",
            Requirement::Unknown => "Don't know",
        }
    }
}

impl fmt::Display for Requirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Requirement {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "yes" => Ok(Requirement::Yes),
            "no" => Ok(Requirement::No),
            "unknown" => Ok(Requirement::Unknown),
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
    pub purchase_required: Requirement,
    pub code_required: Requirement,
    #[serde(default)]
    pub amenities: Vec<Amenity>,
    pub clean_avg: Option<f64>,
    /// Average of the reviews' optional coffee scores, when any exist.
    #[serde(default)]
    pub coffee_avg: Option<f64>,
    /// Average of the reviews' optional food scores, when any exist.
    #[serde(default)]
    pub food_avg: Option<f64>,
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
    #[serde(default)]
    pub coffee: Option<i16>,
    #[serde(default)]
    pub food: Option<i16>,
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
    #[serde(default)]
    pub coffee: Option<i16>,
    #[serde(default)]
    pub food: Option<i16>,
    pub door_ft: i32,
    #[serde(default)]
    pub door_note: String,
    pub parking: Parking,
    #[serde(default)]
    pub purchase_required: Requirement,
    #[serde(default)]
    pub code_required: Requirement,
    #[serde(default)]
    pub amenities: Vec<Amenity>,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewReview {
    pub device_id: String,
    pub clean: i16,
    #[serde(default)]
    pub coffee: Option<i16>,
    #[serde(default)]
    pub food: Option<i16>,
    #[serde(default)]
    pub text: String,
}

/// Full set of editable place fields, sent to `PUT /api/places/{id}` by a
/// logged-in user. Coordinates resolve like on create — explicit lat/lng
/// win, otherwise a *changed* address is parsed as "lat, lng" or geocoded;
/// an unchanged address keeps the stored coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdatePlace {
    pub name: String,
    pub place_type: PlaceType,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
    #[serde(default)]
    pub address: Option<String>,
    pub door_ft: i32,
    #[serde(default)]
    pub door_note: String,
    pub parking: Parking,
    #[serde(default)]
    pub purchase_required: Requirement,
    #[serde(default)]
    pub code_required: Requirement,
    #[serde(default)]
    pub amenities: Vec<Amenity>,
    #[serde(default)]
    pub hours: Option<String>,
}

/// One audited change to a place, from `GET /api/places/{id}/edits`: which
/// field changed, its old and new value, who changed it and when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceEdit {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub author: String,
    pub created_at: String,
    pub time_ago: String,
}

/// Human-readable label for a field name recorded in the place edit log.
pub fn edit_field_label(field: &str) -> &str {
    match field {
        "name" => "Name",
        "place_type" => "Type",
        "location" => "Location",
        "address" => "Address",
        "door_ft" => "Bathroom distance (ft)",
        "door_note" => "Bathroom directions",
        "parking" => "Parking",
        "purchase_required" => "Purchase required",
        "code_required" => "Code required",
        "amenities" => "Amenities",
        "hours" => "Hours",
        other => other,
    }
}

/// How an audited value reads in the history UI; blanks become "(empty)".
pub fn edit_value_display(value: &str) -> &str {
    if value.trim().is_empty() {
        "(empty)"
    } else {
        value
    }
}

/// One entry in an account's activity log, from `GET /api/auth/activity` or
/// `GET /api/admin/users/{id}/activity`: a login, failed login, profile
/// change, place edit, or rating — merged from several tables and sorted
/// newest first. `summary` and `actor` are pre-formatted server-side since
/// the underlying sources have very different shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// "login" | "failed_login" | "profile_change" | "place_change" | "rating"
    pub kind: String,
    pub summary: String,
    pub actor: String,
    pub created_at: String,
    pub time_ago: String,
}

/// Human-readable label for a field name recorded in the account activity
/// log — as opposed to [`edit_field_label`], which covers place fields.
pub fn user_field_label(field: &str) -> &str {
    match field {
        "username" => "Username",
        "given_name" => "Given name",
        "family_name" => "Family name",
        other => other,
    }
}

/// "Yes"/"No" for a stringified boolean recorded in the activity log.
pub fn bool_label(value: &str) -> &str {
    if value == "true" {
        "Yes"
    } else {
        "No"
    }
}

/// Username + password sent to `POST /api/auth/register` and `/login`.
/// `invite_code` is required for registration (ignored, and safe to omit,
/// on login).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub invite_code: String,
}

/// Body for `PUT /api/auth/password`: the current password (verified against /// the stored hash) plus a new one that must satisfy the registration rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangePassword {
    pub current_password: String,
    pub new_password: String,
}

/// Body for `POST /api/auth/password/set`: adds a password to an account
/// that doesn't have one yet (a Google-only account). No current password
/// to verify, since there isn't one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetPassword {
    pub new_password: String,
}

/// Body for `PATCH /api/auth/profile`: optional given and family name fields.
/// Only fields present in the JSON are updated; omitting a field leaves it
/// unchanged on the server.
pub const NAME_MAX: usize = 40;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateProfile {
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
}

/// Validates a given or family name for the profile update endpoint.
pub fn validate_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("name must not be empty");
    }
    if trimmed.chars().count() > NAME_MAX {
        return Err("name must be 40 characters or fewer");
    }
    Ok(())
}

/// Days before an unredeemed invitation code expires.
pub const INVITE_EXPIRY_DAYS: i32 = 7;
/// Most invitations a new user may send per (rolling) day, before their
/// account is 24 hours old.
pub const INVITES_PER_DAY_NEW: i64 = 25;
/// Most invitations an older user may send per (rolling) day, once the
/// account has existed for at least 24 hours.
pub const INVITES_PER_DAY_OLD_AGE: i64 = 100;
/// Longest allowed invitation name.
pub const INVITE_NAME_MAX: usize = 40;

/// Validates the friendly name attached to an invitation.
pub fn validate_invite_name(name: &str) -> Result<(), &'static str> {
    if name.chars().count() > INVITE_NAME_MAX {
        return Err("name must be 40 characters or fewer");
    }
    Ok(())
}

/// Where an invitation is in its life: waiting to be used, past its
/// 7-day window, or redeemed by a friend who joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteStatus {
    Pending,
    Expired,
    Joined,
}

impl InviteStatus {
    pub fn label(self) -> &'static str {
        match self {
            InviteStatus::Pending => "Pending",
            InviteStatus::Expired => "Expired",
            InviteStatus::Joined => "Joined",
        }
    }
}

/// A code an existing user can hand to a friend so they can register.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Invitation {
    pub code: String,
    /// Friendly name the inviter gave this invitation (may be empty); the
    /// invited user can change it once they've registered.
    #[serde(default)]
    pub name: String,
    pub status: InviteStatus,
    /// Username of the friend who joined with this code, once redeemed.
    #[serde(default)]
    pub joined_username: Option<String>,
}

/// Body for `POST /api/invites`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewInvite {
    #[serde(default)]
    pub name: String,
}

/// Body for `PUT /api/invites/{code}/name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InviteNameUpdate {
    pub name: String,
}

/// Everything the account page needs about invitations: who invited this
/// user (and the editable name on that invitation), plus every invitation
/// this user has sent, most recent first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvitesOverview {
    /// Username of whoever invited this user, if their account still exists.
    pub invited_by: Option<String>,
    /// Code of the invitation this user redeemed to register, if any.
    pub my_invite_code: Option<String>,
    /// Current name on that invitation — the user may change it.
    pub my_invite_name: Option<String>,
    pub invites: Vec<Invitation>,
}

/// A logged-in session: the bearer token plus the display username.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthSession {
    pub token: String,
    pub username: String,
    /// Admins get the admin page (backup export / import). Defaults so
    /// sessions stored before the field existed still deserialize.
    #[serde(default)]
    pub is_admin: bool,
    /// User's given name, if set. Defaults empty for legacy sessions.
    #[serde(default)]
    pub given_name: String,
    /// User's family name, if set. Defaults empty for legacy sessions.
    #[serde(default)]
    pub family_name: String,
}

/// A user summary for the admin users list — id, name, role and join date;
/// never carries a password hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
    pub created_at: String,
}

/// What `POST /api/admin/import` did: how many rows each table now holds,
/// plus the newly generated passwords for accounts that were recreated
/// (password hashes are never exported, so imported accounts get fresh
/// random passwords — shown once, here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSummary {
    pub users: usize,
    pub invitations: usize,
    pub places: usize,
    pub reviews: usize,
    pub saved_places: usize,
    /// username → newly generated password. Accounts that already existed
    /// (matched by username — notably the importing admin) keep their
    /// current password and don't appear here.
    pub new_passwords: std::collections::BTreeMap<String, String>,
}

pub const USERNAME_MIN: usize = 3;
pub const USERNAME_MAX: usize = 24;
pub const PASSWORD_MIN: usize = 8;

/// Validates a username for registration: 3–24 chars, letters, digits,
/// `_` or `-`. Shared so the form and the API reject the same inputs.
pub fn validate_username(username: &str) -> Result<(), &'static str> {
    let n = username.chars().count();
    if !(USERNAME_MIN..=USERNAME_MAX).contains(&n) {
        return Err("username must be 3-24 characters");
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("username may only use letters, digits, - and _");
    }
    Ok(())
}

/// Validates a password for registration.
pub fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.chars().count() < PASSWORD_MIN {
        return Err("password must be at least 8 characters");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeocodeResult {
    pub lat: f64,
    pub lng: f64,
    pub display_name: String,
}

/// A place name and coordinates resolved from a pasted Google Maps link.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapsLinkResult {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
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

/// Decodes a percent-encoded URL query component; `+` also means space.
pub fn decode_query_component(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push(hi * 16 + lo);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
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

/// The decoded value of `key` in a URL query string (a leading `?` is
/// allowed), e.g. for reading the invite code out of a shared link.
pub fn query_param(search: &str, key: &str) -> Option<String> {
    search
        .trim_start_matches('?')
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| decode_query_component(v))
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
    /// Comma-separated list of [`Amenity`] strings; matches places
    /// offering at least one of them.
    #[serde(default)]
    pub amenities: Option<String>,
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

    pub fn parsed_amenities(&self) -> Vec<Amenity> {
        self.amenities
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
        if let Some(a) = self.amenities.as_deref().filter(|a| !a.is_empty()) {
            parts.push(format!("amenities={a}"));
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

/// Miles at the low and high ends of the distance filter's logarithmic scale.
pub const RADIUS_MIN_MI: f64 = 1.0;
pub const RADIUS_MAX_MI: f64 = 100.0;
/// Resolution of the underlying `<input type="range">` — kept far finer than
/// the whole-mile output so the slider thumb still moves smoothly even
/// though small distances round to the same displayed mile for many steps.
pub const RADIUS_SLIDER_STEPS: u32 = 1000;

/// Maps a raw slider position (`0..=RADIUS_SLIDER_STEPS`) to a whole-mile
/// radius on a logarithmic scale, so drag distance near the low end (1 mi)
/// changes the radius by much less than the same drag near the high end
/// (100 mi).
pub fn radius_from_slider(pos: u32) -> u8 {
    let t = f64::from(pos.min(RADIUS_SLIDER_STEPS)) / f64::from(RADIUS_SLIDER_STEPS);
    let mi = RADIUS_MIN_MI * (RADIUS_MAX_MI / RADIUS_MIN_MI).powf(t);
    mi.round().clamp(RADIUS_MIN_MI, RADIUS_MAX_MI) as u8
}

/// Inverse of [`radius_from_slider`] — used to place the slider thumb for a
/// given radius (e.g. when the filter sheet opens or is reset).
pub fn radius_to_slider(mi: u8) -> u32 {
    let mi = f64::from(mi).clamp(RADIUS_MIN_MI, RADIUS_MAX_MI);
    let t = (mi / RADIUS_MIN_MI).ln() / (RADIUS_MAX_MI / RADIUS_MIN_MI).ln();
    (t * f64::from(RADIUS_SLIDER_STEPS)).round() as u32
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
    fn amenity_serde_round_trip() {
        for a in Amenity::ALL {
            let json = serde_json::to_string(&a).expect("serialize");
            assert_eq!(json, format!("\"{}\"", a.as_str()));
            let back: Amenity = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, a);
            assert_eq!(a.as_str().parse::<Amenity>(), Ok(a));
        }
    }

    #[test]
    fn aspect_serde_round_trip() {
        for a in Aspect::ALL {
            let json = serde_json::to_string(&a).expect("serialize");
            assert_eq!(json, format!("\"{}\"", a.as_str()));
            let back: Aspect = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, a);
            assert_eq!(a.as_str().parse::<Aspect>(), Ok(a));
        }
        assert!("vibes".parse::<Aspect>().is_err());
    }

    #[test]
    fn requirement_round_trip_and_defaults_to_unknown() {
        for r in Requirement::ALL {
            assert_eq!(r.as_str().parse::<Requirement>(), Ok(r));
        }
        assert!("maybe".parse::<Requirement>().is_err());
        assert_eq!(Requirement::default(), Requirement::Unknown);
    }

    #[test]
    fn places_query_round_trips_through_query_string() {
        let q = PlacesQuery {
            q: None,
            lat: Some(47.6),
            lng: Some(-122.3),
            radius_mi: Some(5.0),
            types: Some("shop,park".to_owned()),
            amenities: Some("coffee,seating".to_owned()),
            clean_min: Some(4.0),
            no_purchase: Some(true),
            has_parking: None,
            sort: Some(SortBy::Cleanliness),
        };
        let qs = q.to_query_string();
        assert!(qs.contains("types=shop,park"));
        assert!(qs.contains("amenities=coffee,seating"));
        assert!(qs.contains("sort=cleanliness"));
        assert!(!qs.contains("has_parking"));
        assert_eq!(q.parsed_types(), vec![PlaceType::Shop, PlaceType::Park]);
        assert_eq!(q.parsed_amenities(), vec![Amenity::Coffee, Amenity::Seating]);
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
            types: Some("shop,unknown,store".to_owned()),
            ..PlacesQuery::default()
        };
        assert_eq!(q.parsed_types(), vec![PlaceType::Shop, PlaceType::Store]);
    }

    #[test]
    fn parsed_amenities_skips_unknown_entries_and_defaults_empty() {
        let q = PlacesQuery {
            amenities: Some("coffee,sauna,food".to_owned()),
            ..PlacesQuery::default()
        };
        assert_eq!(q.parsed_amenities(), vec![Amenity::Coffee, Amenity::Food]);
        assert!(PlacesQuery::default().parsed_amenities().is_empty());
    }

    #[test]
    fn featured_amenities_are_a_subset_of_all() {
        for a in Amenity::FEATURED {
            assert!(Amenity::ALL.contains(&a));
        }
    }

    #[test]
    fn door_short_labels() {
        assert_eq!(door_short(0), "At entrance");
        assert_eq!(door_short(40), "40 ft");
    }

    #[test]
    fn edit_field_labels_are_humanized() {
        assert_eq!(edit_field_label("name"), "Name");
        assert_eq!(edit_field_label("door_ft"), "Bathroom distance (ft)");
        assert_eq!(edit_field_label("purchase_required"), "Purchase required");
        // Unknown fields pass through so old logs never break the UI.
        assert_eq!(edit_field_label("mystery"), "mystery");
    }

    #[test]
    fn edit_value_display_marks_blanks() {
        assert_eq!(edit_value_display("street"), "street");
        assert_eq!(edit_value_display(""), "(empty)");
        assert_eq!(edit_value_display("   "), "(empty)");
    }

    #[test]
    fn validate_username_accepts_reasonable_names() {
        assert_eq!(validate_username("sam"), Ok(()));
        assert_eq!(validate_username("Trail_Scout-42"), Ok(()));
    }

    #[test]
    fn validate_username_rejects_bad_lengths_and_chars() {
        assert!(validate_username("ab").is_err());
        assert!(validate_username(&"x".repeat(25)).is_err());
        assert!(validate_username("sam smith").is_err());
        assert!(validate_username("sam@home").is_err());
        assert!(validate_username("").is_err());
    }

    #[test]
    fn query_param_finds_and_decodes_values() {
        assert_eq!(query_param("?code=A1B2C3D4", "code"), Some("A1B2C3D4".to_owned()));
        assert_eq!(query_param("a=1&code=X%20Y%26Z", "code"), Some("X Y&Z".to_owned()));
        assert_eq!(query_param("?note=one+two", "note"), Some("one two".to_owned()));
        assert_eq!(query_param("?code=A", "other"), None);
        assert_eq!(query_param("", "code"), None);
        // Malformed escapes pass through rather than panicking.
        assert_eq!(query_param("?x=50%2", "x"), Some("50%2".to_owned()));
        assert_eq!(query_param("?x=%GG", "x"), Some("%GG".to_owned()));
    }

    #[test]
    fn decode_query_component_round_trips_encode() {
        for s in ["camber coffee", "a&b=c?", "plain-text_1.0~", "naïve café"] {
            assert_eq!(decode_query_component(&encode_query_component(s)), s);
        }
    }

    #[test]
    fn invite_status_serde_and_labels() {
        for (status, json, label) in [
            (InviteStatus::Pending, "\"pending\"", "Pending"),
            (InviteStatus::Expired, "\"expired\"", "Expired"),
            (InviteStatus::Joined, "\"joined\"", "Joined"),
        ] {
            assert_eq!(serde_json::to_string(&status).expect("serialize"), json);
            let back: InviteStatus = serde_json::from_str(json).expect("deserialize");
            assert_eq!(back, status);
            assert_eq!(status.label(), label);
        }
    }

    #[test]
    fn validate_invite_name_limits_length() {
        assert_eq!(validate_invite_name(""), Ok(()));
        assert_eq!(validate_invite_name("Bob"), Ok(()));
        assert!(validate_invite_name(&"x".repeat(INVITE_NAME_MAX + 1)).is_err());
    }

    #[test]
    fn validate_password_requires_min_length() {
        assert_eq!(validate_password("longenough"), Ok(()));
        assert!(validate_password("short").is_err());
    }

    #[test]
    fn scout_name_from_device_id() {
        assert_eq!(scout_name("3f9a2b-xyz"), "Scout 3f9a");
        assert_eq!(scout_name(""), "Scout");
    }

    #[test]
    fn radius_slider_covers_full_range_at_the_ends() {
        assert_eq!(radius_from_slider(0), 1);
        assert_eq!(radius_from_slider(RADIUS_SLIDER_STEPS), 100);
        assert_eq!(radius_from_slider(RADIUS_SLIDER_STEPS * 10), 100);
    }

    #[test]
    fn radius_slider_is_monotonically_non_decreasing() {
        let mut prev = radius_from_slider(0);
        for pos in 1..=RADIUS_SLIDER_STEPS {
            let mi = radius_from_slider(pos);
            assert!(mi >= prev, "radius dropped at pos {pos}: {prev} -> {mi}");
            prev = mi;
        }
    }

    #[test]
    fn radius_slider_is_finer_near_the_low_end_than_the_high_end() {
        // More raw slider ticks are spent representing 1 mi than are spent
        // representing 100 mi — a given drag distance changes the radius by
        // less near the low end (fine control) than near the high end
        // (coarse control), which is the point of the log scale.
        let low_end_ticks = (0..=RADIUS_SLIDER_STEPS)
            .take_while(|&pos| radius_from_slider(pos) <= 1)
            .count();
        let high_end_ticks = (0..=RADIUS_SLIDER_STEPS)
            .rev()
            .take_while(|&pos| radius_from_slider(pos) >= 100)
            .count();
        assert!(
            low_end_ticks > high_end_ticks,
            "low_end_ticks={low_end_ticks} high_end_ticks={high_end_ticks}"
        );
    }

    #[test]
    fn radius_to_slider_round_trips_through_radius_from_slider() {
        for mi in [1, 2, 5, 10, 25, 50, 75, 100] {
            let pos = radius_to_slider(mi);
            assert_eq!(radius_from_slider(pos), mi, "mi={mi} pos={pos}");
        }
    }

    #[test]
    fn place_detail_flattens_summary_fields() {
        let detail = PlaceDetail {
            summary: PlaceSummary {
                id: Uuid::nil(),
                name: "Camber Coffee".to_owned(),
                place_type: PlaceType::Shop,
                lat: 47.6,
                lng: -122.3,
                address: "214 Maple Ave".to_owned(),
                door_ft: 15,
                parking: Parking::Street,
                purchase_required: Requirement::Yes,
                code_required: Requirement::Yes,
                amenities: vec![Amenity::Coffee, Amenity::Seating],
                clean_avg: Some(4.8),
                coffee_avg: Some(4.5),
                food_avg: None,
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
