use actix_web::web::{Data, Json, Path, Query, ServiceConfig};
use actix_web::{delete, get, post, put, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use shared::{parse_latlng, NewPlace, NewReview, PlacesQuery};
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
    if !(1..=5).contains(&new.clean) {
        return Err(ApiError::BadRequest(
            "cleanliness must be between 1 and 5".to_owned(),
        ));
    }
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
    db::insert_review(&state.pool, id, &new.device_id, user.id, new.clean, new.comment.trim())
        .await?;
    let detail = db::get_place(&state.pool, id, None).await?;
    Ok(HttpResponse::Created().json(detail))
}

#[post("/api/places/{id}/reviews")]
async fn create_review(
    state: Data<AppState>,
    user: AuthUser,
    id: Path<Uuid>,
    body: Json<NewReview>,
) -> Result<HttpResponse, ApiError> {
    if !(1..=5).contains(&body.clean) {
        return Err(ApiError::BadRequest(
            "cleanliness must be between 1 and 5".to_owned(),
        ));
    }
    db::insert_review(&state.pool, *id, &body.device_id, user.id, body.clean, body.text.trim())
        .await?;
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
