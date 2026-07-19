//! Thin async client for the backend API (proxied by Trunk at /api).

use gloo_net::http::{Request, RequestBuilder, Response};
use gloo_storage::{LocalStorage, SessionStorage, Storage};
use shared::{
    encode_query_component, ActivityEntry, AuthSession, ChangePassword, Credentials,
    ImportSummary, Invitation, InviteNameUpdate, InvitesOverview, MapsLinkResult, NewInvite,
    NewPlace, NewReview, OverpassPoi, OverpassQuery, PlaceDetail, PlaceEdit, PlaceSummary,
    PlacesQuery, PromotePoi, SetPassword, UpdatePlace, UpdateProfile, UserSummary,
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

/// Overpass POIs (raw OpenStreetMap data, not yet in the app) within a map
/// viewport, respecting the "Discover" toggle upstream of this call (the
/// caller decides whether to fetch at all).
pub async fn fetch_overpass_pois(q: &OverpassQuery) -> ApiResult<Vec<OverpassPoi>> {
    let url = format!("/api/overpass/places?{}", q.to_query_string());
    Request::get(&url).send().await.map_err(err)?.json().await.map_err(err)
}

/// Promotes an Overpass POI into a normal app place — the first time a user
/// saves, rates, or edits one. Idempotent on the backend.
pub async fn promote_overpass_poi(poi_id: &str, device_id: &str) -> ApiResult<PlaceDetail> {
    let body = PromotePoi { poi_id: poi_id.to_owned(), device_id: device_id.to_owned() };
    let res = with_auth(Request::post("/api/overpass/places/promote"))
        .json(&body)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not save this place").await);
    }
    res.json().await.map_err(err)
}

/// Resolves a pasted Google Maps link to a place name and coordinates, for
/// prefilling the "Add a place" form.
pub async fn resolve_maps_link(url: &str) -> ApiResult<MapsLinkResult> {
    let qs = encode_query_component(url);
    let res = Request::get(&format!("/api/maps-link?url={qs}")).send().await.map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not read that link").await);
    }
    res.json().await.map_err(err)
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

pub async fn delete_place(place_id: Uuid) -> ApiResult<()> {
    let res = with_auth(Request::delete(&format!("/api/places/{place_id}")))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not delete place").await);
    }
    Ok(())
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

/// Whether this server has a Google OAuth client configured — the
/// "Continue with Google" buttons stay hidden without one.
pub async fn google_sign_in_enabled() -> bool {
    let Ok(res) = Request::get("/api/auth/google/enabled").send().await else {
        return false;
    };
    let Ok(body) = res.json::<serde_json::Value>().await else {
        return false;
    };
    body["enabled"].as_bool().unwrap_or(false)
}

/// Where the browser was when a Google sign-in began. The OAuth round trip
/// reloads the whole app, so `/oauth-complete` reads this to send sign-ins
/// that started from a gated page (e.g. `/poi/.../edit`) back there.
const OAUTH_RETURN_KEY: &str = "sb_oauth_return";

/// Navigates the whole page (not a SPA route — this leaves the app to
/// Google's consent screen and back) to start the Google OAuth flow.
/// `invite_code` is ignored for `mode == "login"`.
pub fn start_google_auth(mode: &str, invite_code: &str) {
    let url = format!(
        "/api/auth/google/start?mode={}&invite_code={}",
        encode_query_component(mode),
        encode_query_component(invite_code),
    );
    if let Some(window) = web_sys::window() {
        if let Ok(path) = window.location().pathname() {
            let _ = SessionStorage::set(OAUTH_RETURN_KEY, path);
        }
        let _ = window.location().set_href(&url);
    }
}

/// Takes (and clears) the path stored by [`start_google_auth`], if any.
pub fn take_oauth_return_path() -> Option<String> {
    let path = SessionStorage::get::<String>(OAUTH_RETURN_KEY).ok();
    SessionStorage::delete(OAUTH_RETURN_KEY);
    path
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

/// Removes an invitation: soft-deletes an expired one (hidden from the
/// list, kept in the database), or soft-revokes a pending one. Only the
/// inviter can act on their own codes.
pub async fn revoke_invite(code: &str) -> ApiResult<()> {
    let url = format!("/api/invites/{}", encode_query_component(code));
    let res = with_auth(Request::delete(&url))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not remove the invitation").await);
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

/// Adds a password to an account that doesn't have one yet (a Google-only
/// account) — the "vice versa" of linking Google onto a password account.
pub async fn set_password(new_password: &str) -> ApiResult<()> {
    let res = with_auth(Request::post("/api/auth/password/set"))
        .json(&SetPassword { new_password: new_password.to_owned() })
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not set a password").await);
    }
    Ok(())
}

/// Starts the "link my Google account" flow: asks the server for a
/// consent-screen URL (over `fetch`, so the bearer token goes in a header
/// rather than a URL) and navigates the whole page there. `mode == "link"`
/// on the backend, distinct from `start_google_auth`'s login/register modes.
pub async fn start_google_link() -> ApiResult<()> {
    let res = with_auth(Request::post("/api/auth/google/link/start"))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not start linking your Google account").await);
    }
    let body: serde_json::Value = res.json().await.map_err(err)?;
    let url = body["url"].as_str().unwrap_or_default().to_owned();
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_href(&url);
    }
    Ok(())
}

/// Detaches the Google account from the signed-in user — the reverse of
/// [`start_google_link`]. The server refuses if the account has no
/// password, since that would leave no way to sign back in.
pub async fn unlink_google() -> ApiResult<()> {
    let res = with_auth(Request::post("/api/auth/google/unlink"))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not unlink your Google account").await);
    }
    Ok(())
}

/// Updates the signed-in user's given and/or family name.
pub async fn update_profile(profile: &UpdateProfile) -> ApiResult<()> {
    let res = with_auth(Request::patch("/api/auth/profile"))
        .json(profile)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not update profile").await);
    }
    Ok(())
}

/// The signed-in user's own activity log — logins, failed logins, profile
/// changes, place edits and ratings, newest first.
pub async fn fetch_my_activity() -> ApiResult<Vec<ActivityEntry>> {
    let res = with_auth(Request::get("/api/auth/activity")).send().await.map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not load activity").await);
    }
    res.json().await.map_err(err)
}

/// One account's activity log — admin only.
pub async fn fetch_user_activity(id: Uuid) -> ApiResult<Vec<ActivityEntry>> {
    let res = with_auth(Request::get(&format!("/api/admin/users/{id}/activity")))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not load activity").await);
    }
    res.json().await.map_err(err)
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

/// Returns the signed-in user's profile (username, admin flag, given/family
/// name) from `GET /api/auth/me`.
pub async fn get_me() -> ApiResult<serde_json::Value> {
    let res = with_auth(Request::get("/api/auth/me")).send().await.map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not load profile").await);
    }
    res.json().await.map_err(err)
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

/// What `GET /api/admin/users/{id}` returns for the edit page.
#[derive(Debug, serde::Deserialize)]
pub struct AdminUserDetail {
    pub username: String,
    pub is_admin: bool,
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
}

/// Editable fields on a user account — admin-only.
#[derive(Debug, serde::Serialize)]
pub struct AdminUserUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_admin: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
}

/// What `PATCH /api/admin/users/{id}` returns — a fresh summary so the
/// caller can update the list in place without refetching.
pub type AdminUserUpdated = serde_json::Value;

/// Updates one user account (admin only). Empty fields are skipped thanks
/// to `skip_serializing_if`.
pub async fn update_user(id: Uuid, update: &AdminUserUpdate) -> ApiResult<AdminUserUpdated> {
    let res = with_auth(Request::patch(&format!("/api/admin/users/{id}")))
        .json(update)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not update user").await);
    }
    res.json().await.map_err(err)
}

/// Deletes a user account (admin only). Places/reviews cascade; invitations
/// stay so the audit trail is intact.
pub async fn delete_user(id: Uuid) -> ApiResult<()> {
    let res = with_auth(Request::delete(&format!("/api/admin/users/{id}")))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not delete user").await);
    }
    Ok(())
}

/// Fetches one user's editable fields by id (admin only).
pub async fn fetch_user_detail(id: Uuid) -> ApiResult<AdminUserDetail> {
    let res = with_auth(Request::get(&format!("/api/admin/users/{id}")))
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err(error_message(res, "could not load user").await);
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
