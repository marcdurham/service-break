//! Thin async client for the backend API (proxied by Trunk at /api).

use gloo_net::http::Request;
use shared::{NewPlace, NewReview, PlaceDetail, PlaceSummary, PlacesQuery};
use uuid::Uuid;

pub type ApiResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
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
    let res = Request::post("/api/places")
        .json(new)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        let body: serde_json::Value = res.json().await.unwrap_or_default();
        return Err(body["error"].as_str().unwrap_or("could not add place").to_owned());
    }
    res.json().await.map_err(err)
}

pub async fn create_review(place_id: Uuid, review: &NewReview) -> ApiResult<PlaceDetail> {
    let res = Request::post(&format!("/api/places/{place_id}/reviews"))
        .json(review)
        .map_err(err)?
        .send()
        .await
        .map_err(err)?;
    if res.status() >= 400 {
        return Err("could not post review".to_owned());
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
    Request::put(&format!("/api/devices/{device_id}/saved/{place_id}"))
        .send()
        .await
        .map_err(err)?;
    Ok(())
}

pub async fn unsave_place(device_id: &str, place_id: Uuid) -> ApiResult<()> {
    Request::delete(&format!("/api/devices/{device_id}/saved/{place_id}"))
        .send()
        .await
        .map_err(err)?;
    Ok(())
}
