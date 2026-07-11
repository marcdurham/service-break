//! Service Break API: places with rated bathrooms, reviews, and saved lists.

pub mod auth;
pub mod db;
pub mod error;
pub mod geocode;
pub mod handlers;
pub mod util;

use sqlx::PgPool;

pub struct AppState {
    pub pool: PgPool,
    pub http: reqwest::Client,
    pub nominatim_url: String,
}

pub const DEFAULT_NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org";

/// HTTP client with the User-Agent required by the Nominatim usage policy.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("service-break/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}
