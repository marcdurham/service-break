//! User accounts and bearer-token sessions.
//!
//! Register/login return a token the frontend sends back as
//! `Authorization: Bearer <token>`; handlers that change data take an
//! [`AuthUser`] argument, which rejects requests without a valid session.

use std::future::Future;
use std::pin::Pin;

use actix_web::dev::Payload;
use actix_web::web::{self, Data, Json, ServiceConfig};
use actix_web::{get, post, FromRequest, HttpRequest, HttpResponse};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde_json::json;
use shared::{validate_password, validate_username, AuthSession, Credentials};
use uuid::Uuid;

use crate::db;
use crate::error::ApiError;
use crate::AppState;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(register).service(login).service(logout).service(me);
}

/// The logged-in user behind a request, extracted from the bearer token.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
}

fn bearer_token(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("Authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|t| t.trim().to_owned())
}

impl FromRequest for AuthUser {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<AuthUser, ApiError>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let state = req.app_data::<Data<AppState>>().cloned();
        let token = bearer_token(req);
        Box::pin(async move {
            let state =
                state.ok_or_else(|| ApiError::Internal("AppState not configured".to_owned()))?;
            let token = token
                .ok_or_else(|| ApiError::Unauthorized("sign in to do that".to_owned()))?;
            db::session_user(&state.pool, &token)
                .await?
                .ok_or_else(|| ApiError::Unauthorized("session expired — sign in again".to_owned()))
        })
    }
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ApiError::Internal(format!("password hashing failed: {e}")))
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

/// A fresh 256-bit session token.
fn new_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// Runs a CPU-heavy closure off the async workers.
async fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    web::block(f)
        .await
        .map_err(|e| ApiError::Internal(format!("blocking task failed: {e}")))?
}

async fn start_session(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    username: String,
) -> Result<AuthSession, ApiError> {
    let token = new_token();
    db::create_session(pool, &token, user_id).await?;
    Ok(AuthSession { token, username })
}

#[post("/api/auth/register")]
async fn register(
    state: Data<AppState>,
    body: Json<Credentials>,
) -> Result<HttpResponse, ApiError> {
    let creds = body.into_inner();
    let username = creds.username.trim().to_owned();
    validate_username(&username).map_err(|e| ApiError::BadRequest(e.to_owned()))?;
    validate_password(&creds.password).map_err(|e| ApiError::BadRequest(e.to_owned()))?;

    let password = creds.password;
    let hash = run_blocking(move || hash_password(&password)).await?;
    let user_id = db::create_user(&state.pool, &username, &hash).await?;
    let session = start_session(&state.pool, user_id, username).await?;
    Ok(HttpResponse::Created().json(session))
}

#[post("/api/auth/login")]
async fn login(state: Data<AppState>, body: Json<Credentials>) -> Result<HttpResponse, ApiError> {
    let creds = body.into_inner();
    let bad = || ApiError::Unauthorized("wrong username or password".to_owned());
    let user = db::find_user(&state.pool, creds.username.trim())
        .await?
        .ok_or_else(bad)?;

    let password = creds.password;
    let hash = user.password_hash;
    let ok = run_blocking(move || Ok(verify_password(&password, &hash))).await?;
    if !ok {
        return Err(bad());
    }
    let session = start_session(&state.pool, user.id, user.username).await?;
    Ok(HttpResponse::Ok().json(session))
}

#[post("/api/auth/logout")]
async fn logout(state: Data<AppState>, req: HttpRequest) -> Result<HttpResponse, ApiError> {
    if let Some(token) = bearer_token(&req) {
        db::delete_session(&state.pool, &token).await?;
    }
    Ok(HttpResponse::NoContent().finish())
}

/// Lets the frontend check whether its stored token is still valid.
#[get("/api/auth/me")]
async fn me(user: AuthUser) -> HttpResponse {
    HttpResponse::Ok().json(json!({ "username": user.username }))
}
