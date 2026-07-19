use actix_cors::Cors;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use backend::{
    admin, db, handlers, http_client, telemetry, AppState, DEFAULT_NOMINATIM_URL,
    DEFAULT_OVERPASS_URL, DEFAULT_TILE_URL,
};
use sqlx::postgres::PgPoolOptions;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Every log line goes to stdout as before. When `OPENOBSERVE_URL` is
    // set, the same lines (plus one per finished HTTP request, via
    // `TracingLogger`'s root span below) are also serialized as JSON and
    // shipped to OpenObserve in the background — see `telemetry.rs`.
    let openobserve_config = telemetry::OpenObserveConfig::from_env();
    let shipping_to_openobserve = openobserve_config.is_some();
    let openobserve_layer = openobserve_config.map(|cfg| {
        let writer = telemetry::spawn_shipper(cfg, http_client());
        tracing_subscriber::fmt::layer()
            .json()
            .flatten_event(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_writer(writer)
    });
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .with(openobserve_layer)
        .init();
    tracing::info!(shipping_to_openobserve, "logging initialized");

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set (see .cargo/config.toml)");
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8020".to_owned());
    let nominatim_url =
        std::env::var("NOMINATIM_URL").unwrap_or_else(|_| DEFAULT_NOMINATIM_URL.to_owned());
    let overpass_url =
        std::env::var("OVERPASS_URL").unwrap_or_else(|_| DEFAULT_OVERPASS_URL.to_owned());
    let tile_url = std::env::var("TILE_URL").unwrap_or_else(|_| DEFAULT_TILE_URL.to_owned());
    let tile_cache_dir =
        std::env::var("TILE_CACHE_DIR").unwrap_or_else(|_| "./tile-cache".to_owned());
    let tile_cache_max_mb: u64 = std::env::var("TILE_CACHE_MAX_MB")
        .map(|v| v.parse().expect("TILE_CACHE_MAX_MB must be a whole number of megabytes"))
        .unwrap_or(512);
    let tile_cache =
        backend::tile_cache::TileCache::open(&tile_cache_dir, tile_cache_max_mb * 1024 * 1024)?;
    let google = backend::google_auth::GoogleConfig::from_env();
    tracing::info!(
        google_sign_in = google.is_some(),
        tile_cache_dir,
        tile_cache_max_mb,
        "startup config"
    );

    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    db::ensure_seeded(&pool).await?;
    admin::ensure_admin(&pool).await?;

    let state = Data::new(AppState {
        pool,
        http: http_client(),
        nominatim_url,
        overpass_url,
        tile_url,
        tile_cache,
        google,
    });

    tracing::info!(%bind_addr, "starting service-break backend");
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(Cors::permissive())
            .wrap(TracingLogger::default())
            .configure(handlers::configure)
    })
    .bind(&bind_addr)?
    .run()
    .await?;
    Ok(())
}
