//! User accounts and bearer-token sessions.
//!
//! Register/login return a token the frontend sends back as
//! `Authorization: Bearer <token>`; handlers that change data take an
//! [`AuthUser`] argument, which rejects requests without a valid session.

use actix_web::delete;
use std::future::Future;
use std::pin::Pin;

use actix_web::dev::Payload;
use actix_web::web::{self, Data, Json, Path, ServiceConfig};
use actix_web::{get, patch, post, put, FromRequest, HttpRequest, HttpResponse};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde_json::json;
use shared::{
    validate_invite_name, validate_name, validate_password, validate_username, AuthSession,
    ChangePassword, Credentials, InviteNameUpdate, NewInvite, UpdateProfile,
};
use uuid::Uuid;

use crate::db;
use crate::error::ApiError;
use crate::AppState;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(register)
        .service(login)
        .service(logout)
        .service(me)
        .service(change_password)
        .service(update_profile)
        .service(my_activity)
        .service(create_invite)
        .service(list_invites)
        .service(rename_invite)
        .service(delete_invite);
}

/// The logged-in user behind a request, extracted from the bearer token.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
    pub given_name: String,
    pub family_name: String,
}

/// Extractor for admin-only endpoints: like [`AuthUser`], but rejects
/// non-admin accounts with 403.
#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthUser);

impl FromRequest for AdminUser {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<AdminUser, ApiError>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let user = AuthUser::from_request(req, payload);
        Box::pin(async move {
            let user = user.await?;
            if user.is_admin {
                Ok(AdminUser(user))
            } else {
                Err(ApiError::Forbidden("that needs an admin account".to_owned()))
            }
        })
    }
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

/// Argon2-hashes a password into a PHC string, as stored in `users`.
/// Public because the `hash-password` helper binary (used by
/// `scripts/change-password.sh`) and the admin importer reuse it.
pub fn hash_password(password: &str) -> Result<String, ApiError> {
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
pub(crate) async fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    web::block(f)
        .await
        .map_err(|e| ApiError::Internal(format!("blocking task failed: {e}")))?
}

pub(crate) async fn start_session(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    username: String,
    is_admin: bool,
) -> Result<AuthSession, ApiError> {
    let token = new_token();
    db::create_session(pool, &token, user_id).await?;
    Ok(AuthSession {
        token,
        username,
        is_admin,
        given_name: String::new(),
        family_name: String::new(),
    })
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
    let invite_code = creds.invite_code.trim().to_owned();
    if invite_code.is_empty() {
        return Err(ApiError::BadRequest("an invite code is required to register".to_owned()));
    }

    let password = creds.password;
    let hash = run_blocking(move || hash_password(&password)).await?;
    let user_id = db::register_user(&state.pool, &username, &hash, &invite_code).await?;
    let session = start_session(&state.pool, user_id, username, false).await?;
    Ok(HttpResponse::Created().json(session))
}

#[post("/api/auth/login")]
async fn login(state: Data<AppState>, body: Json<Credentials>) -> Result<HttpResponse, ApiError> {
    let creds = body.into_inner();
    let bad = || ApiError::Unauthorized("wrong username or password".to_owned());
    // An unknown username has no account to log the attempt against — only
    // failures against a real account (wrong password) reach the activity
    // log, which is always viewed scoped to one account.
    let user = db::find_user(&state.pool, creds.username.trim())
        .await?
        .ok_or_else(bad)?;
    let user_id = user.id;

    let Some(hash) = user.password_hash else {
        // Google-only account: no password to check against. Same generic
        // error as a wrong password, so this endpoint can't be used to
        // probe which accounts exist or how they sign in.
        db::record_activity(&state.pool, user_id, "failed_login", "", "", "", Some(user_id))
            .await?;
        return Err(bad());
    };
    let password = creds.password;
    let ok = run_blocking(move || Ok(verify_password(&password, &hash))).await?;
    if !ok {
        db::record_activity(&state.pool, user_id, "failed_login", "", "", "", Some(user_id))
            .await?;
        return Err(bad());
    }
    db::record_activity(&state.pool, user_id, "login", "", "", "", Some(user_id)).await?;
    let session = start_session(&state.pool, user.id, user.username, user.is_admin).await?;
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
async fn me(state: Data<AppState>, user: AuthUser) -> HttpResponse {
    let (given, family): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT given_name, family_name FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or_default();
    HttpResponse::Ok().json(json!({
        "username": user.username,
        "is_admin": user.is_admin,
        "given_name": given,
        "family_name": family,
    }))
}

/// Updates the signed-in user's given and/or family name. Only fields
/// present in the request body are changed; empty strings are rejected.
#[patch("/api/auth/profile")]
async fn update_profile(
    state: Data<AppState>,
    user: AuthUser,
    body: Json<UpdateProfile>,
) -> Result<HttpResponse, ApiError> {
    let profile = body.into_inner();

    if let Some(ref name) = profile.given_name {
        validate_name(name).map_err(|e| ApiError::BadRequest(e.to_owned()))?;
    }
    if let Some(ref name) = profile.family_name {
        validate_name(name).map_err(|e| ApiError::BadRequest(e.to_owned()))?;
    }

    let before = db::find_user_by_id(&state.pool, user.id)
        .await?
        .ok_or_else(|| ApiError::Internal("user disappeared from under us".to_owned()))?;

    db::update_user_names(
        &state.pool,
        user.id,
        profile.given_name.as_deref(),
        profile.family_name.as_deref(),
    )
    .await?;

    if let Some(new_given) = &profile.given_name {
        if *new_given != before.given_name {
            db::record_activity(
                &state.pool,
                user.id,
                "profile_change",
                "given_name",
                &before.given_name,
                new_given,
                Some(user.id),
            )
            .await?;
        }
    }
    if let Some(new_family) = &profile.family_name {
        if *new_family != before.family_name {
            db::record_activity(
                &state.pool,
                user.id,
                "profile_change",
                "family_name",
                &before.family_name,
                new_family,
                Some(user.id),
            )
            .await?;
        }
    }

    Ok(HttpResponse::NoContent().finish())
}

/// Swaps the signed-in user's password. The current password is verified
/// against the stored Argon2 hash before the new one (which must satisfy
/// the same rules as registration) is hashed and persisted.
#[put("/api/auth/password")]
async fn change_password(
    state: Data<AppState>,
    user: AuthUser,
    body: Json<ChangePassword>,
) -> Result<HttpResponse, ApiError> {
    let creds = body.into_inner();
    validate_password(&creds.new_password).map_err(|e| ApiError::BadRequest(e.to_owned()))?;

    // Find the current hash so we can verify with `verify_password`.
    let user_row = db::find_user_by_id(&state.pool, user.id)
        .await?
        .ok_or_else(|| ApiError::Internal("user disappeared from under us".to_owned()))?;

    let Some(hash) = user_row.password_hash else {
        return Err(ApiError::BadRequest(
            "this account signs in with Google and has no password to change".to_owned(),
        ));
    };
    let ok = run_blocking(move || Ok(verify_password(&creds.current_password, &hash))).await?;
    if !ok {
        return Err(ApiError::BadRequest(
            "current password is wrong".to_owned(),
        ));
    }

    let new_hash = run_blocking(move || hash_password(&creds.new_password)).await?;
    db::update_user_password(&state.pool, user.id, &new_hash).await?;
    db::record_activity(&state.pool, user.id, "profile_change", "password", "", "", Some(user.id))
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// The signed-in user's own activity log — logins, failed logins, profile
/// changes, place edits, and ratings they've posted. Newest first, capped
/// at 50.
#[get("/api/auth/activity")]
async fn my_activity(state: Data<AppState>, user: AuthUser) -> Result<HttpResponse, ApiError> {
    let entries = db::list_user_activity(&state.pool, user.id, &user.username, 50).await?;
    Ok(HttpResponse::Ok().json(entries))
}

/// Issues a fresh invite code the signed-in user can hand to a friend,
/// optionally labelled with the friend's name.
#[post("/api/invites")]
async fn create_invite(
    state: Data<AppState>,
    user: AuthUser,
    body: Json<NewInvite>,
) -> Result<HttpResponse, ApiError> {
    let name = body.into_inner().name.trim().to_owned();
    validate_invite_name(&name).map_err(|e| ApiError::BadRequest(e.to_owned()))?;
    let invite = db::create_invitation(&state.pool, user.id, &name, user.is_admin).await?;
    Ok(HttpResponse::Created().json(invite))
}

/// The signed-in user's invitations and friends, plus who invited them.
#[get("/api/invites")]
async fn list_invites(state: Data<AppState>, user: AuthUser) -> Result<HttpResponse, ApiError> {
    let overview = db::invites_overview(&state.pool, user.id).await?;
    Ok(HttpResponse::Ok().json(overview))
}

/// Renames an invitation: the inviter may rename a pending code, and the
/// user who redeemed it may change the name they were invited under.
#[put("/api/invites/{code}/name")]
async fn rename_invite(
    state: Data<AppState>,
    user: AuthUser,
    code: Path<String>,
    body: Json<InviteNameUpdate>,
) -> Result<HttpResponse, ApiError> {
    let name = body.into_inner().name.trim().to_owned();
    validate_invite_name(&name).map_err(|e| ApiError::BadRequest(e.to_owned()))?;
    db::rename_invitation(&state.pool, user.id, code.trim(), &name).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Revokes (expires) a pending invitation. Only the inviter can revoke.
#[delete("/api/invites/{code}")]
async fn delete_invite(
    state: Data<AppState>,
    user: AuthUser,
    path: Path<String>,
) -> Result<HttpResponse, ApiError> {
    let code = path.into_inner();
    db::revoke_invitation(&state.pool, user.id, &code).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hash produced here (and by the `hash-password` binary that
    /// `scripts/change-password.sh` calls) must verify with the same code
    /// the login endpoint uses.
    #[test]
    fn hash_password_round_trips_through_verify() {
        let hash = hash_password("I brake for coffee").expect("hash");
        assert!(hash.starts_with("$argon2"));
        assert!(verify_password("I brake for coffee", &hash));
        assert!(!verify_password("i brake for tea", &hash));
    }
}
