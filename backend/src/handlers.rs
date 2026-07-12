use actix_web::web::{Data, Json, Path, Query, ServiceConfig};
use actix_web::{delete, get, post, put, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use shared::{parse_latlng, NewPlace, NewReview, PlacesQuery, UpdatePlace};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db;
use crate::error::ApiError;
use crate::geocode;
use crate::AppState;

pub fn configure(cfg: &mut ServiceConfig) {
    // Backup imports (admin) can be far larger than the 2 MB default.
    cfg.app_data(actix_web::web::JsonConfig::default().limit(32 * 1024 * 1024));
    crate::auth::configure(cfg);
    crate::admin::configure(cfg);
    cfg.service(health)
        .service(list_places)
        .service(create_place)
        .service(get_place)
        .service(update_place)
        .service(list_place_edits)
        .service(create_review)
        .service(list_saved)
        .service(save_place)
        .service(unsave_place)
        .service(geocode_query);
}

#[derive(Debug, Deserialize)]
struct OriginQuery {
    lat: Option<f64>,
    lng: Option<f64>,
}

impl OriginQuery {
    fn origin(&self) -> Option<(f64, f64)> {
        self.lat.zip(self.lng)
    }
}

#[get("/api/health")]
async fn health() -> HttpResponse {
    HttpResponse::Ok().json(json!({ "status": "ok" }))
}

/// Rejects aspect scores outside 1-5. `clean` is required; `coffee` and
/// `food` are optional but must be in range when present.
fn validate_scores(clean: i16, coffee: Option<i16>, food: Option<i16>) -> Result<(), ApiError> {
    for (name, score) in [("cleanliness", Some(clean)), ("coffee", coffee), ("food", food)] {
        if let Some(s) = score {
            if !(1..=5).contains(&s) {
                return Err(ApiError::BadRequest(format!("{name} must be between 1 and 5")));
            }
        }
    }
    Ok(())
}

#[get("/api/places")]
async fn list_places(
    state: Data<AppState>,
    q: Query<PlacesQuery>,
) -> Result<HttpResponse, ApiError> {
    let places = db::list_places(&state.pool, &q).await?;
    Ok(HttpResponse::Ok().json(places))
}

#[get("/api/places/{id}")]
async fn get_place(
    state: Data<AppState>,
    id: Path<Uuid>,
    q: Query<OriginQuery>,
) -> Result<HttpResponse, ApiError> {
    let place = db::get_place(&state.pool, *id, q.origin()).await?;
    Ok(HttpResponse::Ok().json(place))
}

#[post("/api/places")]
async fn create_place(
    state: Data<AppState>,
    user: AuthUser,
    body: Json<NewPlace>,
) -> Result<HttpResponse, ApiError> {
    let new = body.into_inner();
    if new.name.trim().is_empty() {
        return Err(ApiError::BadRequest("place name is required".to_owned()));
    }
    validate_scores(new.clean, new.coffee, new.food)?;
    let address = new.address.clone().unwrap_or_default().trim().to_owned();

    // Resolve coordinates: explicit lat/lng > "lat, lng" typed in the address
    // field > geocoding the address via Nominatim.
    let (lat, lng, resolved_address) = match (new.lat, new.lng) {
        (Some(lat), Some(lng)) => (lat, lng, address),
        _ => {
            if let Some((lat, lng)) = parse_latlng(&address) {
                (lat, lng, address)
            } else if !address.is_empty() {
                let hit = geocode::geocode(&state.http, &state.nominatim_url, &address)
                    .await?
                    .ok_or_else(|| {
                        ApiError::BadRequest(format!("could not find address {address:?}"))
                    })?;
                (hit.lat, hit.lng, address)
            } else {
                return Err(ApiError::BadRequest(
                    "provide coordinates or an address".to_owned(),
                ));
            }
        }
    };

    let place = db::InsertPlace {
        name: new.name.trim().to_owned(),
        place_type: new.place_type,
        lat,
        lng,
        address: resolved_address,
        door_ft: new.door_ft.max(0),
        door_note: new.door_note.trim().to_owned(),
        parking: new.parking,
        purchase_required: new.purchase_required,
        code_required: new.code_required,
        amenities: new.amenities.clone(),
        device_id: new.device_id.clone(),
        user_id: user.id,
    };
    let id = db::insert_place(&state.pool, &place).await?;
    let review = db::InsertReview {
        place_id: id,
        device_id: &new.device_id,
        user_id: user.id,
        clean: new.clean,
        coffee: new.coffee,
        food: new.food,
        text: new.comment.trim(),
    };
    db::insert_review(&state.pool, &review).await?;
    let detail = db::get_place(&state.pool, id, None).await?;
    Ok(HttpResponse::Created().json(detail))
}

/// Edits a place (any logged-in user). Every changed field is written to
/// the `place_edits` audit log — what changed, when, and by whom.
#[put("/api/places/{id}")]
async fn update_place(
    state: Data<AppState>,
    user: AuthUser,
    id: Path<Uuid>,
    body: Json<UpdatePlace>,
) -> Result<HttpResponse, ApiError> {
    let up = body.into_inner();
    if up.name.trim().is_empty() {
        return Err(ApiError::BadRequest("place name is required".to_owned()));
    }
    let current = db::get_place(&state.pool, *id, None).await?;
    let address = up.address.clone().unwrap_or_default().trim().to_owned();

    // Resolve coordinates like on create — explicit lat/lng > an unchanged
    // address keeps the stored coordinates (no needless geocoding) > "lat,
    // lng" typed into the address > geocoding the new address.
    let (lat, lng, resolved_address) = match (up.lat, up.lng) {
        (Some(lat), Some(lng)) => (lat, lng, address),
        _ if address == current.summary.address => {
            (current.summary.lat, current.summary.lng, address)
        }
        _ => {
            if let Some((lat, lng)) = parse_latlng(&address) {
                (lat, lng, address)
            } else if !address.is_empty() {
                let hit = geocode::geocode(&state.http, &state.nominatim_url, &address)
                    .await?
                    .ok_or_else(|| {
                        ApiError::BadRequest(format!("could not find address {address:?}"))
                    })?;
                (hit.lat, hit.lng, address)
            } else {
                return Err(ApiError::BadRequest(
                    "provide coordinates or an address".to_owned(),
                ));
            }
        }
    };

    let fields = db::UpdateFields {
        name: up.name.trim().to_owned(),
        place_type: up.place_type,
        lat,
        lng,
        address: resolved_address,
        door_ft: up.door_ft.max(0),
        door_note: up.door_note.trim().to_owned(),
        parking: up.parking,
        purchase_required: up.purchase_required,
        code_required: up.code_required,
        amenities: up.amenities.clone(),
        hours: up.hours.map(|h| h.trim().to_owned()).filter(|h| !h.is_empty()),
    };
    db::update_place(&state.pool, *id, user.id, &fields).await?;
    let detail = db::get_place(&state.pool, *id, None).await?;
    Ok(HttpResponse::Ok().json(detail))
}

/// A place's edit history — who changed which field, from and to what,
/// and when. Public, like all reads.
#[get("/api/places/{id}/edits")]
async fn list_place_edits(
    state: Data<AppState>,
    id: Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let edits = db::list_place_edits(&state.pool, *id).await?;
    Ok(HttpResponse::Ok().json(edits))
}

#[post("/api/places/{id}/reviews")]
async fn create_review(
    state: Data<AppState>,
    user: AuthUser,
    id: Path<Uuid>,
    body: Json<NewReview>,
) -> Result<HttpResponse, ApiError> {
    validate_scores(body.clean, body.coffee, body.food)?;
    let review = db::InsertReview {
        place_id: *id,
        device_id: &body.device_id,
        user_id: user.id,
        clean: body.clean,
        coffee: body.coffee,
        food: body.food,
        text: body.text.trim(),
    };
    db::insert_review(&state.pool, &review).await?;
    let detail = db::get_place(&state.pool, *id, None).await?;
    Ok(HttpResponse::Created().json(detail))
}

#[get("/api/devices/{device_id}/saved")]
async fn list_saved(
    state: Data<AppState>,
    device_id: Path<String>,
    q: Query<OriginQuery>,
) -> Result<HttpResponse, ApiError> {
    let places = db::list_saved(&state.pool, &device_id, q.origin()).await?;
    Ok(HttpResponse::Ok().json(places))
}

#[put("/api/devices/{device_id}/saved/{place_id}")]
async fn save_place(
    state: Data<AppState>,
    _user: AuthUser,
    path: Path<(String, Uuid)>,
) -> Result<HttpResponse, ApiError> {
    let (device_id, place_id) = path.into_inner();
    db::save_place(&state.pool, &device_id, place_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[delete("/api/devices/{device_id}/saved/{place_id}")]
async fn unsave_place(
    state: Data<AppState>,
    _user: AuthUser,
    path: Path<(String, Uuid)>,
) -> Result<HttpResponse, ApiError> {
    let (device_id, place_id) = path.into_inner();
    db::unsave_place(&state.pool, &device_id, place_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Debug, Deserialize)]
struct GeocodeParams {
    q: String,
}

#[get("/api/geocode")]
async fn geocode_query(
    state: Data<AppState>,
    params: Query<GeocodeParams>,
) -> Result<HttpResponse, ApiError> {
    let query = params.q.trim();
    if query.is_empty() {
        return Err(ApiError::BadRequest("query is required".to_owned()));
    }
    let hit = geocode::geocode(&state.http, &state.nominatim_url, query)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(hit))
}
