use actix_cors::Cors;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use backend::{admin, db, handlers, http_client, AppState, DEFAULT_NOMINATIM_URL};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set (see .cargo/config.toml)");
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8081".to_owned());
    let nominatim_url =
        std::env::var("NOMINATIM_URL").unwrap_or_else(|_| DEFAULT_NOMINATIM_URL.to_owned());
    let google = backend::google_auth::GoogleConfig::from_env();
    tracing::info!(google_sign_in = google.is_some(), "startup config");

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
        google,
    });

    tracing::info!(%bind_addr, "starting service-break backend");
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(Cors::permissive())
            .configure(handlers::configure)
    })
    .bind(&bind_addr)?
    .run()
    .await?;
    Ok(())
}
