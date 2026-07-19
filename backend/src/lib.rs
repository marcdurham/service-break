//! Service Break API: places with rated bathrooms, reviews, and saved lists.

pub mod admin;
pub mod auth;
pub mod db;
pub mod error;
pub mod geocode;
pub mod google_auth;
pub mod handlers;
pub mod maps_link;
pub mod overpass;
pub mod util;

use sqlx::PgPool;

pub struct AppState {
    pub pool: PgPool,
    pub http: reqwest::Client,
    pub nominatim_url: String,
    pub overpass_url: String,
    /// `None` disables "Sign in with Google" (`GOOGLE_CLIENT_ID` etc. not set).
    pub google: Option<google_auth::GoogleConfig>,
}

pub const DEFAULT_NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org";
pub const DEFAULT_OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";

/// HTTP client with the User-Agent required by the Nominatim usage policy.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("service-break/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}
