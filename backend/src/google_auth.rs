//! "Sign in / sign up with Google" — an OAuth 2.0 authorization-code
//! round trip against Google, layered on top of the same bearer-token
//! sessions [`crate::auth`] issues for username/password accounts.
//!
//! Registration still requires an invite code: `/api/auth/google/start`
//! takes `mode=register&invite_code=...` and the callback redeems it
//! exactly like [`db::register_user`] does, just with a Google identity
//! instead of a username/password. `mode=login` never creates an account —
//! it only signs in a Google identity that's already linked to one.
//!
//! There are no cookies or server-side redirect sessions here: the flow's
//! CSRF/replay protection and the `mode`/`invite_code` it carries live in
//! the short-lived `oauth_states` table (see [`db::create_oauth_state`]).
//! On success the callback redirects the browser to the frontend's
//! `/oauth-complete` route with the session in the URL *fragment* (never
//! the query string, so it's never sent to a server or logged) for the
//! Yew app to pick up and store.

use actix_web::web::{Data, Query, ServiceConfig};
use actix_web::{get, post, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use shared::encode_query_component;

use crate::auth::{start_session, AuthUser};
use crate::db;
use crate::AppState;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(google_enabled)
        .service(google_start)
        .service(google_link_start)
        .service(google_callback);
}

/// Client id/secret and the exact redirect URI registered with Google,
/// read once at startup from `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` /
/// `GOOGLE_REDIRECT_URI`. `None` (any of the three unset) disables the
/// feature entirely — the endpoints below just report that.
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
    /// Must exactly match a redirect URI configured on the Google OAuth
    /// client, e.g. `https://servicebreak.example.com/api/auth/google/callback`.
    pub redirect_uri: String,
    /// The frontend's origin, e.g. `https://servicebreak.example.com` —
    /// derived from `redirect_uri` (same origin, `/api/...` is proxied
    /// through to the backend by nginx/Trunk).
    pub app_base_url: String,
}

impl GoogleConfig {
    pub fn from_env() -> Option<Self> {
        let client_id = std::env::var("GOOGLE_CLIENT_ID").ok()?;
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").ok()?;
        let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI").ok()?;
        let app_base_url = redirect_uri
            .strip_suffix("/api/auth/google/callback")
            .unwrap_or(&redirect_uri)
            .to_owned();
        Some(GoogleConfig { client_id, client_secret, redirect_uri, app_base_url })
    }
}

/// Lets the frontend hide the "Continue with Google" button when the
/// server has no Google OAuth client configured.
#[get("/api/auth/google/enabled")]
async fn google_enabled(state: Data<AppState>) -> HttpResponse {
    HttpResponse::Ok().json(json!({ "enabled": state.google.is_some() }))
}

#[derive(Deserialize)]
struct StartQuery {
    /// `"register"` or `"login"`; anything else is treated as `"login"`.
    #[serde(default)]
    mode: String,
    #[serde(default)]
    invite_code: String,
}

/// A fresh 256-bit opaque state token — same construction as session
/// tokens (see `auth::new_token`), just a separate function since that
/// one's private to `auth`.
fn new_state_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// Kicks off the flow: stores server-side state, then 302s the browser to
/// Google's consent screen.
#[get("/api/auth/google/start")]
async fn google_start(state: Data<AppState>, query: Query<StartQuery>) -> HttpResponse {
    let Some(cfg) = &state.google else {
        return HttpResponse::ServiceUnavailable()
            .json(json!({ "error": "Google sign-in isn't configured on this server" }));
    };

    let mode = if query.mode == "register" { "register" } else { "login" };
    let invite_code = query.invite_code.trim().to_owned();
    if mode == "register" && invite_code.is_empty() {
        return HttpResponse::BadRequest()
            .json(json!({ "error": "an invite code is required to register" }));
    }

    let state_token = new_state_token();
    if let Err(e) = db::create_oauth_state(&state.pool, &state_token, mode, &invite_code).await {
        tracing::error!(error = %e, "failed to store oauth state");
        return HttpResponse::InternalServerError()
            .json(json!({ "error": "could not start Google sign-in" }));
    }

    HttpResponse::Found().append_header(("Location", google_auth_url(cfg, &state_token))).finish()
}

/// Builds Google's consent-screen URL for a given (already-stored) state
/// token. Shared by the plain sign-in/register start and the "link an
/// existing account" start below.
fn google_auth_url(cfg: &GoogleConfig, state_token: &str) -> String {
    format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
         client_id={}&redirect_uri={}&response_type=code&scope={}&state={}\
         &access_type=online&prompt=select_account",
        encode_query_component(&cfg.client_id),
        encode_query_component(&cfg.redirect_uri),
        encode_query_component("openid email profile"),
        state_token,
    )
}

/// Kicks off the "link a Google identity to my existing account" flow: like
/// [`google_start`], but requires an active session (linking happens over
/// `fetch`, not a top-level navigation, so the bearer token can be sent as a
/// header rather than something that would end up in server logs or
/// browser history) and returns the consent-screen URL as JSON for the
/// frontend to navigate to, instead of redirecting itself.
#[post("/api/auth/google/link/start")]
async fn google_link_start(state: Data<AppState>, user: AuthUser) -> HttpResponse {
    let Some(cfg) = &state.google else {
        return HttpResponse::ServiceUnavailable()
            .json(json!({ "error": "Google sign-in isn't configured on this server" }));
    };

    let state_token = new_state_token();
    if let Err(e) = db::create_oauth_link_state(&state.pool, &state_token, user.id).await {
        tracing::error!(error = %e, "failed to store oauth link state");
        return HttpResponse::InternalServerError()
            .json(json!({ "error": "could not start linking your Google account" }));
    }

    HttpResponse::Ok().json(json!({ "url": google_auth_url(cfg, &state_token) }))
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GoogleProfile {
    sub: String,
    email: String,
    #[serde(default)]
    email_verified: bool,
    #[serde(default)]
    given_name: String,
    #[serde(default)]
    family_name: String,
}

async fn exchange_code(
    http: &reqwest::Client,
    cfg: &GoogleConfig,
    code: &str,
) -> Result<TokenResponse, reqwest::Error> {
    http.post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("redirect_uri", cfg.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

async fn fetch_profile(
    http: &reqwest::Client,
    access_token: &str,
) -> Result<GoogleProfile, reqwest::Error> {
    http.get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

/// Redirects to the frontend's `/oauth-complete` route with `message` in
/// the URL fragment, for it to show as a toast.
fn redirect_with_error(app_base_url: &str, message: &str) -> HttpResponse {
    let url = format!("{app_base_url}/oauth-complete#error={}", encode_query_component(message));
    HttpResponse::Found().append_header(("Location", url)).finish()
}

/// Redirects to the frontend's `/oauth-complete` route with `message` in
/// the URL fragment under a distinct key from `redirect_with_error`, so the
/// frontend can route the user straight to `/register` (with a persistent
/// banner) instead of dropping them back on the login page with a toast
/// that vanishes before most people act on it.
fn redirect_to_register(app_base_url: &str, message: &str) -> HttpResponse {
    let url =
        format!("{app_base_url}/oauth-complete#register_required={}", encode_query_component(message));
    HttpResponse::Found().append_header(("Location", url)).finish()
}

/// Redirects to the frontend's `/oauth-complete` route with the new
/// session in the URL fragment, for it to store and treat as a login.
fn redirect_with_session(app_base_url: &str, session: &shared::AuthSession) -> HttpResponse {
    let url = format!(
        "{app_base_url}/oauth-complete#token={}&username={}&is_admin={}",
        encode_query_component(&session.token),
        encode_query_component(&session.username),
        session.is_admin,
    );
    HttpResponse::Found().append_header(("Location", url)).finish()
}

/// Redirects to the frontend's `/oauth-complete` route reporting a
/// successful account link. No session is issued — the user was already
/// signed in, and their existing bearer token (kept in browser storage,
/// untouched by this round trip) is still valid.
fn redirect_linked(app_base_url: &str) -> HttpResponse {
    HttpResponse::Found().append_header(("Location", format!("{app_base_url}/oauth-complete#linked=1"))).finish()
}

/// Where Google sends the browser back to after the consent screen.
/// Always redirects to the frontend (success or failure) rather than
/// returning a JSON error — this is a top-level browser navigation, not an
/// API call the frontend can inspect directly.
#[get("/api/auth/google/callback")]
async fn google_callback(state: Data<AppState>, query: Query<CallbackQuery>) -> HttpResponse {
    let Some(cfg) = &state.google else {
        return redirect_with_error("", "Google sign-in isn't configured on this server");
    };

    if let Some(err) = &query.error {
        return redirect_with_error(&cfg.app_base_url, &format!("Google sign-in was cancelled ({err})"));
    }
    let (Some(code), Some(state_token)) = (&query.code, &query.state) else {
        return redirect_with_error(&cfg.app_base_url, "Google sign-in response was incomplete");
    };

    let (mode, invite_code, link_user_id) = match db::take_oauth_state(&state.pool, state_token).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return redirect_with_error(&cfg.app_base_url, "that sign-in link expired — try again")
        }
        Err(e) => return redirect_with_error(&cfg.app_base_url, &e.to_string()),
    };

    let token = match exchange_code(&state.http, cfg, code).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "google token exchange failed");
            return redirect_with_error(&cfg.app_base_url, "couldn't complete sign-in with Google");
        }
    };
    let profile = match fetch_profile(&state.http, &token.access_token).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "google userinfo fetch failed");
            return redirect_with_error(&cfg.app_base_url, "couldn't read your Google profile");
        }
    };
    if !profile.email_verified {
        return redirect_with_error(&cfg.app_base_url, "your Google account's email isn't verified");
    }

    if mode == "link" {
        let Some(user_id) = link_user_id else {
            return redirect_with_error(&cfg.app_base_url, "that link request was invalid — try again");
        };
        return match db::link_google_account(
            &state.pool,
            user_id,
            &profile.sub,
            &profile.email,
            &profile.given_name,
            &profile.family_name,
        )
        .await
        {
            Ok(()) => redirect_linked(&cfg.app_base_url),
            Err(e) => redirect_with_error(&cfg.app_base_url, &e.to_string()),
        };
    }

    let result = if mode == "register" {
        db::upsert_google_user(
            &state.pool,
            &profile.sub,
            &profile.email,
            &profile.given_name,
            &profile.family_name,
            &invite_code,
        )
        .await
    } else {
        match db::find_user_by_google_sub(&state.pool, &profile.sub).await {
            Ok(Some(u)) => Ok((u.id, u.username, u.is_admin)),
            Ok(None) => {
                return redirect_to_register(
                    &cfg.app_base_url,
                    "No account is linked to that Google login yet. Enter an invite code below, \
                     then tap Continue with Google to finish creating your account.",
                );
            }
            Err(e) => Err(e),
        }
    };

    let (user_id, username, is_admin) = match result {
        Ok(v) => v,
        Err(e) => return redirect_with_error(&cfg.app_base_url, &e.to_string()),
    };

    match start_session(&state.pool, user_id, username, is_admin).await {
        Ok(session) => redirect_with_session(&cfg.app_base_url, &session),
        Err(e) => redirect_with_error(&cfg.app_base_url, &e.to_string()),
    }
}
