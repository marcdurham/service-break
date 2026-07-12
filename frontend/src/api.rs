//! Thin async client for the backend API (proxied by Trunk at /api).

use gloo_net::http::{Request, RequestBuilder, Response};
use gloo_storage::{LocalStorage, Storage};
use shared::{
    encode_query_component, AuthSession, ChangePassword, Credentials, ImportSummary, Invitation,
    InviteNameUpdate, InvitesOverview, NewInvite, NewPlace, NewReview, PlaceDetail, PlaceEdit,
    PlaceSummary, PlacesQuery, UpdatePlace, UserSummary,
};
use uuid::Uuid;

pub type ApiResult<T> = Result<T, String>;

const AUTH_KEY: &str = "sb_auth";

/// The session from the last login on this device, if any.
pub fn stored_auth() -> Option<AuthSession> {
    LocalStorage::get(AUTH_KEY).ok()
}

pub fn store_auth(session: &AuthSession) {
    let _ = LocalStorage::set(AUTH_KEY, session);
}

pub fn clear_auth() {
    LocalStorage::delete(AUTH_KEY);
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Attaches the stored session's bearer token, when logged in.
fn with_auth(req: RequestBuilder) -> RequestBuilder {
    match stored_auth() {
        Some(s) => req.header("Authorization", &format!("Bearer {}", s.token)),
        None => req,
    }
}

/// The `error` field of an API error body, or `fallback`.
async fn error_message(res: Response, fallback: &str) -> String {
    let body: serde_json::Value = res.json().await.unwrap_or_default();
    body["error"].as_str().unwrap_or(fallback).to_owned()
}

fn origin_qs(origin: Option<(f64, f64)>) -> String {
    match origin {
        Some((lat, lng)) => format!("?lat={lat}&lng={lng}"),
        None => String::new(),
    }
}

pub async fn fetch_places(q: &PlacesQuery) -> ApiResult<Vec<PlaceSummary>> {
    let qs = q.to_query_string();
    let url = if qs.is_empty() {
        "/api/places".to_owned()
    } else {
        format!("/api/places?{qs}")
    };
    Request::get(&url).send().await.map_err(err)?.json().await.map_err(err)
}

pub async fn fetch_place(id: Uuid, origin: Option<(f64, f64)>) -> ApiResult<PlaceDetail> {
    let url = format!("/api/places/{id}{}", origin_qs(origin));
    Request::get(&url).send().await.map_err(err)?.json().await.map_err(err)
}

pub async fn create_place(new: &NewPlace) -> ApiResult<PlaceDetail> {
    let res = with_auth(Request::post("/api/places"))
        .json(new)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not add place").await);
    }
    res.json().await.map_err(err)
}

pub async fn update_place(place_id: Uuid, update: &UpdatePlace) -> ApiResult<PlaceDetail> {
    let res = with_auth(Request::put(&format!("/api/places/{place_id}")))
        .json(update)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not save changes").await);
    }
    res.json().await.map_err(err)
}

/// The audit log of edits to a place, most recent first.
pub async fn fetch_place_edits(place_id: Uuid) -> ApiResult<Vec<PlaceEdit>> {
    let url = format!("/api/places/{place_id}/edits");
    Request::get(&url).send().await.map_err(err)?.json().await.map_err(err)
}

pub async fn create_review(place_id: Uuid, review: &NewReview) -> ApiResult<PlaceDetail> {
    let res = with_auth(Request::post(&format!("/api/places/{place_id}/reviews")))
        .json(review)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not post review").await);
    }
    res.json().await.map_err(err)
}

pub async fn fetch_saved(
    device_id: &str,
    origin: Option<(f64, f64)>,
) -> ApiResult<Vec<PlaceSummary>> {
    let url = format!("/api/devices/{device_id}/saved{}", origin_qs(origin));
    Request::get(&url).send().await.map_err(err)?.json().await.map_err(err)
}

pub async fn save_place(device_id: &str, place_id: Uuid) -> ApiResult<()> {
    let res = with_auth(Request::put(&format!("/api/devices/{device_id}/saved/{place_id}")))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not save place").await);
    }
    Ok(())
}

pub async fn unsave_place(device_id: &str, place_id: Uuid) -> ApiResult<()> {
    let res = with_auth(Request::delete(&format!("/api/devices/{device_id}/saved/{place_id}")))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not remove place").await);
    }
    Ok(())
}

pub async fn register(creds: &Credentials) -> ApiResult<AuthSession> {
    auth_request("/api/auth/register", creds, "could not create account").await
}

pub async fn login(creds: &Credentials) -> ApiResult<AuthSession> {
    auth_request("/api/auth/login", creds, "could not sign in").await
}

async fn auth_request(url: &str, creds: &Credentials, fallback: &str) -> ApiResult<AuthSession> {
    let res = Request::post(url).json(creds).map_err(err)?.send().await.map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, fallback).await);
    }
    res.json().await.map_err(err)
}

pub async fn create_invite(name: &str) -> ApiResult<Invitation> {
    let body = NewInvite { name: name.to_owned() };
    let res = with_auth(Request::post("/api/invites"))
        .json(&body)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not create an invite code").await);
    }
    res.json().await.map_err(err)
}

pub async fn list_invites() -> ApiResult<InvitesOverview> {
    let res = with_auth(Request::get("/api/invites")).send().await.map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not load invite codes").await);
    }
    res.json().await.map_err(err)
}

/// Renames an invitation: yours-to-send while it's pending, or the one you
/// joined with.
pub async fn rename_invite(code: &str, name: &str) -> ApiResult<()> {
    let url = format!("/api/invites/{}/name", encode_query_component(code));
    let body = InviteNameUpdate { name: name.to_owned() };
    let res = with_auth(Request::put(&url)).json(&body).map_err(err)?.send().await.map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not update the name").await);
    }
    Ok(())
}

/// Swaps the signed-in user's password. The current password is verified
/// against the stored hash before the new one is accepted.
pub async fn change_password(creds: &ChangePassword) -> ApiResult<()> {
    let res = with_auth(Request::put("/api/auth/password"))
        .json(creds)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not change password").await);
    }
    Ok(())
}

/// Best-effort server-side session invalidation.
pub async fn logout() {
    let _ = with_auth(Request::post("/api/auth/logout")).send().await;
}

/// Checks the stored token against the server: `Ok(true)` if it's still a
/// valid session, `Ok(false)` if the server rejected it, `Err` if the
/// check itself failed (e.g. offline) and nothing should be concluded.
pub async fn session_is_valid() -> ApiResult<bool> {
    let res = with_auth(Request::get("/api/auth/me")).send().await.map_err(err)?;
    Ok(res.status() < 400)
}

/// Downloads the full-data backup (admin only) as raw JSON text, kept
/// opaque so it can be saved to a file untouched.
pub async fn export_backup() -> ApiResult<String> {
    let res = with_auth(Request::get("/api/admin/export")).send().await.map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not export data").await);
    }
    res.text().await.map_err(err)
}

/// All accounts — admin only. Ids are returned as strings so the frontend
/// doesn't need to pull in uuid for a read-only listing.
pub async fn fetch_users() -> ApiResult<Vec<UserSummary>> {
    let res = with_auth(Request::get("/api/admin/users")).send().await.map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not load users").await);
    }
    res.json().await.map_err(err)
}

/// Restores a backup (admin only) from the raw JSON text of an export.
pub async fn import_backup(backup_json: &str) -> ApiResult<ImportSummary> {
    let res = with_auth(Request::post("/api/admin/import"))
        .header("Content-Type", "application/json")
        .body(backup_json.to_owned())
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not import data").await);
    }
    res.json().await.map_err(err)
}
