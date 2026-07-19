//! Admin-only operations: the auto-created `admin` account, plus full-data
//! export (backup) and import (restore).
//!
//! The export is one JSON document covering every table except `sessions`
//! — and it deliberately omits password hashes. On import, accounts that
//! don't exist yet are recreated with freshly generated random passwords,
//! returned once in the [`ImportSummary`]; accounts whose username already
//! exists (the importing admin in particular) keep their id and password.

use serde_json::json;
use std::collections::{BTreeMap, HashMap};

use actix_web::web::{Data, Json, Path, ServiceConfig};
use actix_web::{delete, get, patch, post, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::{validate_password, validate_username, ImportSummary};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{hash_password, run_blocking, AdminUser};
use crate::db;
use crate::error::ApiError;
use crate::AppState;

pub const ADMIN_USERNAME: &str = "admin";
/// Documented in README.md and DEPLOY.md — change it right after the first
/// start with `scripts/change-password.sh`.
pub const DEFAULT_ADMIN_PASSWORD: &str = "I brake for coffee";

/// Bumped whenever the export layout changes incompatibly.
pub const EXPORT_FORMAT_VERSION: u32 = 1;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(export_data)
        .service(import_data)
        .service(list_users)
        .service(get_user_detail)
        .service(update_user)
        .service(delete_user)
        .service(get_user_activity);
}

/// Creates the `admin` account with [`DEFAULT_ADMIN_PASSWORD`] if no user
/// of that name exists yet. Runs at startup; idempotent.
pub async fn ensure_admin(pool: &PgPool) -> Result<(), ApiError> {
    let existing: Option<bool> =
        sqlx::query_scalar("SELECT is_admin FROM users WHERE lower(username) = $1")
            .bind(ADMIN_USERNAME)
            .fetch_optional(pool)
            .await?;
    match existing {
        Some(true) => {}
        Some(false) => tracing::warn!(
            "a user named {ADMIN_USERNAME:?} exists but is not an admin; grant it manually \
             with: UPDATE users SET is_admin = TRUE WHERE username = '{ADMIN_USERNAME}'"
        ),
        None => {
            let hash = hash_password(DEFAULT_ADMIN_PASSWORD)?;
            sqlx::query(
                "INSERT INTO users (username, password_hash, is_admin) VALUES ($1, $2, TRUE)",
            )
            .bind(ADMIN_USERNAME)
            .bind(hash)
            .execute(pool)
            .await?;
            tracing::info!("created the admin account with the default password — change it!");
        }
    }
    Ok(())
}

/// Everything worth backing up, as one JSON document. `sessions` are
/// excluded (tokens are secrets and worthless after a re-deploy), and
/// users carry no password hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub format_version: u32,
    pub exported_at: DateTime<Utc>,
    pub users: Vec<ExportUser>,
    pub invitations: Vec<ExportInvitation>,
    pub places: Vec<ExportPlace>,
    pub reviews: Vec<ExportReview>,
    pub saved_places: Vec<ExportSavedPlace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExportUser {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    /// Google sign-in linkage. `#[serde(default)]` so backups made before
    /// Google sign-in existed still import cleanly (as password-only
    /// accounts, same as before).
    #[serde(default)]
    pub google_sub: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExportInvitation {
    pub id: Uuid,
    pub code: String,
    pub inviter_id: Option<Uuid>,
    pub redeemed_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub redeemed_at: Option<DateTime<Utc>>,
}

/// Raw column values (place_type etc. stay strings) so a backup round-trips
/// bit-for-bit even if enum variants evolve.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExportPlace {
    pub id: Uuid,
    pub name: String,
    pub place_type: String,
    pub lat: f64,
    pub lng: f64,
    pub address: String,
    pub door_ft: i32,
    pub door_note: String,
    pub parking: String,
    pub purchase_required: String,
    pub code_required: String,
    pub hours: Option<String>,
    pub amenities: Vec<String>,
    pub device_id: String,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// `#[serde(default)]` so backups made before the Overpass POI layer
    /// still import cleanly, defaulting every place to `"app"`.
    #[serde(default = "default_place_source")]
    pub source: String,
}

fn default_place_source() -> String {
    "app".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExportReview {
    pub id: Uuid,
    pub place_id: Uuid,
    pub device_id: String,
    pub user_id: Option<Uuid>,
    pub clean: i16,
    pub text: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExportSavedPlace {
    pub device_id: String,
    pub place_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/admin/export` — the whole database as one downloadable JSON
/// file (no password hashes, no sessions).
#[get("/api/admin/export")]
async fn export_data(state: Data<AppState>, admin: AdminUser) -> Result<HttpResponse, ApiError> {
    let data = collect_export(&state.pool).await?;
    tracing::info!(
        admin_id = %admin.0.id,
        admin_username = %admin.0.username,
        users = data.users.len(),
        places = data.places.len(),
        reviews = data.reviews.len(),
        "admin exported a backup"
    );
    Ok(HttpResponse::Ok()
        .insert_header((
            "Content-Disposition",
            "attachment; filename=\"service-break-backup.json\"",
        ))
        .json(data))
}

async fn collect_export(pool: &PgPool) -> Result<ExportData, ApiError> {
    let users = sqlx::query_as::<_, ExportUser>(
        "SELECT id, username, is_admin, created_at, google_sub, email FROM users \
         ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await?;
    let invitations = sqlx::query_as::<_, ExportInvitation>(
        "SELECT id, code, inviter_id, redeemed_by, created_at, redeemed_at FROM invitations \
         ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await?;
    let places = sqlx::query_as::<_, ExportPlace>(
        "SELECT id, name, place_type, lat, lng, address, door_ft, door_note, parking, \
         purchase_required, code_required, hours, amenities, device_id, user_id, created_at, \
         source \
         FROM places ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await?;
    let reviews = sqlx::query_as::<_, ExportReview>(
        "SELECT id, place_id, device_id, user_id, clean, text, created_at FROM reviews \
         ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await?;
    let saved_places = sqlx::query_as::<_, ExportSavedPlace>(
        "SELECT device_id, place_id, created_at FROM saved_places ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;
    Ok(ExportData {
        format_version: EXPORT_FORMAT_VERSION,
        exported_at: Utc::now(),
        users,
        invitations,
        places,
        reviews,
        saved_places,
    })
}

/// `GET /api/admin/users` — every account, no password hash.
#[get("/api/admin/users")]
async fn list_users(
    state: Data<AppState>,
    _admin: AdminUser,
) -> Result<HttpResponse, ApiError> {
    let users = db::list_users(&state.pool).await?;
    Ok(HttpResponse::Ok().json(users))
}

/// `GET /api/admin/users/{id}` — one account's editable fields.
#[get("/api/admin/users/{id}")]
async fn get_user_detail(
    state: Data<AppState>,
    _admin: AdminUser,
    path: Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = db::find_user_by_id(&state.pool, *path)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(json!({
        "id": user.id.to_string(),
        "username": user.username,
        "is_admin": user.is_admin,
        "given_name": user.given_name,
        "family_name": user.family_name,
    })))
}

/// Request body for `PATCH /api/admin/users/{id}`.
///
/// Every field is optional: only the ones present in the JSON are applied,
/// so callers can send just `{"is_admin": true}` or
/// `{"password": "new-secret"}` without touching the rest.
#[derive(Debug, Default, Deserialize)]
pub struct UpdateUser {
    /// New username. Validated with [`validate_username`](shared::validate_username).
    pub username: Option<String>,
    /// Reset the password to a plain-text value; validated by
    /// [`validate_password`](shared::validate_password). Omit to leave it.
    pub password: Option<String>,
    /// Flip admin status. The caller's own account can't be demoted — that
    /// would lock them out of this very page.
    pub is_admin: Option<bool>,
    /// Given and family name, each optional; omit to leave untouched.
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
}

/// `POST /api/admin/import` — replaces the database contents with a backup
/// produced by the export endpoint. See [`run_import`].
#[post("/api/admin/import")]
async fn import_data(
    state: Data<AppState>,
    admin: AdminUser,
    body: Json<ExportData>,
) -> Result<HttpResponse, ApiError> {
    let summary = run_import(&state.pool, admin.0.id, body.into_inner()).await?;
    tracing::warn!(
        admin_id = %admin.0.id,
        admin_username = %admin.0.username,
        users = summary.users,
        invitations = summary.invitations,
        places = summary.places,
        reviews = summary.reviews,
        saved_places = summary.saved_places,
        new_accounts = summary.new_passwords.len(),
        "admin imported a backup, replacing database contents"
    );
    Ok(HttpResponse::Ok().json(summary))
}

/// Restores a backup: content tables (places, reviews, saved places,
/// invitations) are replaced wholesale, ids preserved so deep links keep
/// working. Accounts are matched by username — matches keep their id and
/// password; the rest are recreated under their exported id with a fresh
/// random password. The caller's own account is never deleted, so the
/// session performing the import survives.
async fn run_import(
    pool: &PgPool,
    caller_id: Uuid,
    data: ExportData,
) -> Result<ImportSummary, ApiError> {
    if data.format_version != EXPORT_FORMAT_VERSION {
        return Err(ApiError::BadRequest(format!(
            "unsupported backup format version {} (this server expects {EXPORT_FORMAT_VERSION})",
            data.format_version
        )));
    }

    let existing: HashMap<String, Uuid> =
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, lower(username) FROM users")
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|(id, name)| (name, id))
            .collect();

    // Fresh passwords for accounts being recreated, hashed off the async
    // workers (Argon2 is CPU-heavy).
    let to_create: Vec<ExportUser> = data
        .users
        .iter()
        .filter(|u| !existing.contains_key(&u.username.to_lowercase()))
        .cloned()
        .collect();
    let passwords: Vec<String> = to_create.iter().map(|_| generate_password()).collect();
    let hashes: Vec<String> = {
        let passwords = passwords.clone();
        run_blocking(move || passwords.iter().map(|p| hash_password(p)).collect()).await?
    };

    // Exported user id -> id in this database.
    let id_map: HashMap<Uuid, Uuid> = data
        .users
        .iter()
        .map(|u| {
            let actual = existing.get(&u.username.to_lowercase()).copied().unwrap_or(u.id);
            (u.id, actual)
        })
        .collect();
    let map_user = |id: Option<Uuid>| id.and_then(|id| id_map.get(&id).copied());

    let exported_names: Vec<String> =
        data.users.iter().map(|u| u.username.to_lowercase()).collect();

    let mut tx = pool.begin().await?;

    for table in ["saved_places", "reviews", "places", "invitations"] {
        sqlx::query(&format!("DELETE FROM {table}")).execute(&mut *tx).await?;
    }
    // Accounts not in the backup go too — except the caller's, which the
    // running session depends on.
    sqlx::query("DELETE FROM users WHERE id <> $1 AND lower(username) <> ALL($2)")
        .bind(caller_id)
        .bind(&exported_names)
        .execute(&mut *tx)
        .await?;

    for (u, hash) in to_create.iter().zip(&hashes) {
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, is_admin, created_at, google_sub, email) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(u.id)
        .bind(&u.username)
        .bind(hash)
        .bind(u.is_admin)
        .bind(u.created_at)
        .bind(&u.google_sub)
        .bind(&u.email)
        .execute(&mut *tx)
        .await?;
    }
    // Kept accounts follow the backup's admin flag (never demoting the
    // caller out from under their own import).
    for u in &data.users {
        if let Some(&id) = existing.get(&u.username.to_lowercase()) {
            if id != caller_id {
                sqlx::query("UPDATE users SET is_admin = $1 WHERE id = $2")
                    .bind(u.is_admin)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }

    for p in &data.places {
        sqlx::query(
            "INSERT INTO places (id, name, place_type, lat, lng, address, door_ft, door_note, \
             parking, purchase_required, code_required, hours, amenities, device_id, user_id, \
             created_at, source) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
        )
        .bind(p.id)
        .bind(&p.name)
        .bind(&p.place_type)
        .bind(p.lat)
        .bind(p.lng)
        .bind(&p.address)
        .bind(p.door_ft)
        .bind(&p.door_note)
        .bind(&p.parking)
        .bind(&p.purchase_required)
        .bind(&p.code_required)
        .bind(&p.hours)
        .bind(&p.amenities)
        .bind(&p.device_id)
        .bind(map_user(p.user_id))
        .bind(p.created_at)
        .bind(&p.source)
        .execute(&mut *tx)
        .await?;
    }
    for r in &data.reviews {
        sqlx::query(
            "INSERT INTO reviews (id, place_id, device_id, user_id, clean, text, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(r.id)
        .bind(r.place_id)
        .bind(&r.device_id)
        .bind(map_user(r.user_id))
        .bind(r.clean)
        .bind(&r.text)
        .bind(r.created_at)
        .execute(&mut *tx)
        .await?;
    }
    for s in &data.saved_places {
        sqlx::query(
            "INSERT INTO saved_places (device_id, place_id, created_at) VALUES ($1, $2, $3)",
        )
        .bind(&s.device_id)
        .bind(s.place_id)
        .bind(s.created_at)
        .execute(&mut *tx)
        .await?;
    }
    for i in &data.invitations {
        sqlx::query(
            "INSERT INTO invitations (id, code, inviter_id, redeemed_by, created_at, redeemed_at) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(i.id)
        .bind(&i.code)
        .bind(map_user(i.inviter_id))
        .bind(map_user(i.redeemed_by))
        .bind(i.created_at)
        .bind(i.redeemed_at)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let new_passwords: BTreeMap<String, String> =
        to_create.into_iter().map(|u| u.username).zip(passwords).collect();

    Ok(ImportSummary {
        users: data.users.len(),
        invitations: data.invitations.len(),
        places: data.places.len(),
        reviews: data.reviews.len(),
        saved_places: data.saved_places.len(),
        new_passwords,
    })
}

/// 16 hex chars (64 random bits) — enough entropy for a handed-out
/// password the user is expected to change anyway.
/// `PATCH /api/admin/users/{id}` — edit any field on a user account.
///
/// Admins can rename the account, reset its password, flip admin status,
/// and update given/family name. The caller's own account cannot be
/// demoted (would lock them out of this page) or deleted.
#[patch("/api/admin/users/{id}")]
async fn update_user(
    state: Data<AppState>,
    admin: AdminUser,
    path: Path<Uuid>,
    body: Json<UpdateUser>,
) -> Result<HttpResponse, ApiError> {
    let target_id = *path;
    if target_id == admin.0.id {
        return Err(ApiError::BadRequest(
            "you can't edit your own account from here".to_owned(),
        ));
    }

    let user = db::find_user_by_id(&state.pool, target_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let new = body.into_inner();

    if let Some(ref name) = new.username {
        validate_username(name).map_err(|e| ApiError::BadRequest(e.to_owned()))?;
        // Reject duplicate usernames (case-insensitive), except the current one.
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT username FROM users WHERE lower(username) = lower($1) AND id <> $2",
        )
        .bind(name)
        .bind(target_id)
        .fetch_optional(&state.pool)
        .await?;
        if existing.is_some() {
            return Err(ApiError::BadRequest(format!(
                "username {name:?} is already taken"
            )));
        }
    }

    // Password reset: hash off the async workers (Argon2 is CPU-heavy).
    let password_hash = if let Some(ref plain) = new.password {
        validate_password(plain).map_err(|e| ApiError::BadRequest(e.to_owned()))?;
        let plain = plain.clone();
        Some(run_blocking(move || hash_password(&plain)).await?)
    } else {
        None
    };

    // Admin flag: don't allow demoting the caller.
    if let (Some(target_is_admin), true) = (&new.is_admin, user.is_admin) {
        if !*target_is_admin && target_id == admin.0.id {
            return Err(ApiError::BadRequest(
                "you can't remove your own admin flag".to_owned(),
            ));
        }
    }

    let mut tx = state.pool.begin().await?;

    if let Some(name) = &new.username {
        sqlx::query("UPDATE users SET username = $1 WHERE id = $2")
            .bind(name)
            .bind(target_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(hash) = password_hash {
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(&hash)
            .bind(target_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(is_admin) = new.is_admin {
        sqlx::query("UPDATE users SET is_admin = $1 WHERE id = $2")
            .bind(is_admin)
            .bind(target_id)
            .execute(&mut *tx)
            .await?;
    }
    if let (Some(g), Some(f)) = (&new.given_name, &new.family_name) {
        sqlx::query("UPDATE users SET given_name = $1, family_name = $2 WHERE id = $3")
            .bind(g)
            .bind(f)
            .bind(target_id)
            .execute(&mut *tx)
            .await?;
    } else if let Some(g) = &new.given_name {
        sqlx::query("UPDATE users SET given_name = $1 WHERE id = $2")
            .bind(g)
            .bind(target_id)
            .execute(&mut *tx)
            .await?;
    } else if let Some(f) = &new.family_name {
        sqlx::query("UPDATE users SET family_name = $1 WHERE id = $2")
            .bind(f)
            .bind(target_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    // Log each changed field to the target's activity log, attributed to
    // the admin who made the change.
    let actor_id = Some(admin.0.id);
    if let Some(name) = &new.username {
        if *name != user.username {
            db::record_activity(
                &state.pool,
                target_id,
                "profile_change",
                "username",
                &user.username,
                name,
                actor_id,
            )
            .await?;
        }
    }
    if new.password.is_some() {
        db::record_activity(
            &state.pool,
            target_id,
            "profile_change",
            "password",
            "",
            "",
            actor_id,
        )
        .await?;
    }
    if let Some(is_admin_new) = new.is_admin {
        if is_admin_new != user.is_admin {
            db::record_activity(
                &state.pool,
                target_id,
                "profile_change",
                "is_admin",
                &user.is_admin.to_string(),
                &is_admin_new.to_string(),
                actor_id,
            )
            .await?;
        }
    }
    if let Some(g) = &new.given_name {
        if *g != user.given_name {
            db::record_activity(
                &state.pool,
                target_id,
                "profile_change",
                "given_name",
                &user.given_name,
                g,
                actor_id,
            )
            .await?;
        }
    }
    if let Some(f) = &new.family_name {
        if *f != user.family_name {
            db::record_activity(
                &state.pool,
                target_id,
                "profile_change",
                "family_name",
                &user.family_name,
                f,
                actor_id,
            )
            .await?;
        }
    }

    let changed_fields: Vec<&str> = [
        new.username.as_ref().filter(|n| **n != user.username).map(|_| "username"),
        new.password.is_some().then_some("password"),
        new.is_admin.filter(|a| *a != user.is_admin).map(|_| "is_admin"),
        new.given_name.as_ref().filter(|n| **n != user.given_name).map(|_| "given_name"),
        new.family_name.as_ref().filter(|n| **n != user.family_name).map(|_| "family_name"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !changed_fields.is_empty() {
        tracing::info!(
            admin_id = %admin.0.id,
            target_user_id = %target_id,
            target_username = %user.username,
            fields = %changed_fields.join(","),
            "admin edited a user account"
        );
    }

    // Return the refreshed summary so the frontend can update the list in
    // place without a full reload.
    let updated = db::find_user_by_id(&state.pool, target_id).await?;
    let row = updated.ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(json!({
        "id": row.id.to_string(),
        "username": row.username,
        "is_admin": row.is_admin,
        "given_name": row.given_name,
        "family_name": row.family_name,
    })))
}

/// `DELETE /api/admin/users/{id}` — remove an account.
///
/// The caller's own account can't be deleted (would kill their session).
/// All places, reviews and saved lists owned by the user are cascade-deleted
/// via FK constraints; invitations they issued or redeemed become orphaned
/// but stay in the database so the audit trail is intact.
#[delete("/api/admin/users/{id}")]
async fn delete_user(
    state: Data<AppState>,
    admin: AdminUser,
    path: Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let target_id = *path;
    if target_id == admin.0.id {
        return Err(ApiError::BadRequest(
            "you can't delete your own account".to_owned(),
        ));
    }

    let target_user = db::find_user_by_id(&state.pool, target_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(target_id)
        .execute(&state.pool)
        .await?;

    tracing::warn!(
        admin_id = %admin.0.id,
        admin_username = %admin.0.username,
        target_user_id = %target_id,
        target_username = %target_user.username,
        "admin deleted a user account"
    );
    Ok(HttpResponse::NoContent().finish())
}

/// `GET /api/admin/users/{id}/activity` — an account's activity log: logins,
/// failed logins, profile changes, place edits and ratings. Newest first,
/// capped at 50.
#[get("/api/admin/users/{id}/activity")]
async fn get_user_activity(
    state: Data<AppState>,
    _admin: AdminUser,
    path: Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = db::find_user_by_id(&state.pool, *path).await?.ok_or(ApiError::NotFound)?;
    let entries = db::list_user_activity(&state.pool, user.id, &user.username, 50).await?;
    Ok(HttpResponse::Ok().json(entries))
}

fn generate_password() -> String {
    let mut p = Uuid::new_v4().simple().to_string();
    p.truncate(16);
    p
}
