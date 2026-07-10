//! Integration tests exercising the HTTP API against a real Postgres.
//!
//! `#[sqlx::test]` creates a disposable database per test using the server at
//! `DATABASE_URL` (set in `.cargo/config.toml`, served by docker-compose).

use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::http::StatusCode;
use actix_web::test::{call_service, init_service, read_body_json, TestRequest};
use actix_web::web::Data;
use actix_web::App;
use backend::{handlers, http_client, AppState};
use serde_json::json;
use shared::{Parking, PlaceDetail, PlaceSummary, PlaceType, Requirement};
use sqlx::PgPool;

async fn app(
    pool: PgPool,
) -> impl Service<actix_http::Request, Response = ServiceResponse<impl MessageBody>, Error = actix_web::Error>
{
    let state = Data::new(AppState {
        pool,
        http: http_client(),
        // Unroutable address: tests must not depend on the live geocoder.
        nominatim_url: "http://127.0.0.1:1".to_owned(),
    });
    init_service(App::new().app_data(state).configure(handlers::configure)).await
}

fn new_place_json(name: &str) -> serde_json::Value {
    json!({
        "device_id": "test-device-1",
        "name": name,
        "place_type": "shop",
        "lat": 47.6117,
        "lng": -122.3402,
        "address": "214 Maple Ave",
        "clean": 5,
        "door_ft": 15,
        "door_note": "Right past the counter",
        "parking": "street",
        "purchase_required": "yes",
        "code_required": "yes",
        "amenities": ["restrooms", "coffee"],
        "comment": "Spotless."
    })
}

#[sqlx::test(migrations = "./migrations")]
async fn create_place_then_list_returns_it(pool: PgPool) {
    let app = app(pool).await;

    let req = TestRequest::post()
        .uri("/api/places")
        .set_json(new_place_json("Camber Coffee"))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: PlaceDetail = read_body_json(res).await;
    assert_eq!(created.summary.name, "Camber Coffee");
    assert_eq!(created.summary.review_count, 1);
    assert_eq!(created.summary.clean_avg, Some(5.0));
    assert_eq!(created.reviews.len(), 1);
    assert_eq!(created.reviews[0].text, "Spotless.");

    let req = TestRequest::get().uri("/api/places").to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let places: Vec<PlaceSummary> = read_body_json(res).await;
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].id, created.summary.id);
    assert_eq!(places[0].place_type, PlaceType::Shop);
    assert_eq!(places[0].parking, Parking::Street);
    assert_eq!(places[0].purchase_required, Requirement::Yes);
    assert_eq!(places[0].code_required, Requirement::Yes);
    assert_eq!(places[0].amenities.len(), 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn create_place_accepts_latlng_typed_into_address(pool: PgPool) {
    let app = app(pool).await;
    let mut body = new_place_json("Roadside Place");
    body["lat"] = json!(null);
    body["lng"] = json!(null);
    body["address"] = json!("37.7749, -122.4194");
    let req = TestRequest::post().uri("/api/places").set_json(body).to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: PlaceDetail = read_body_json(res).await;
    assert!((created.summary.lat - 37.7749).abs() < 1e-9);
    assert!((created.summary.lng + 122.4194).abs() < 1e-9);
}

#[sqlx::test(migrations = "./migrations")]
async fn create_place_without_location_is_rejected(pool: PgPool) {
    let app = app(pool).await;
    let mut body = new_place_json("Nowhere");
    body["lat"] = json!(null);
    body["lng"] = json!(null);
    body["address"] = json!("");
    let req = TestRequest::post().uri("/api/places").set_json(body).to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn list_sorts_by_distance_and_respects_radius(pool: PgPool) {
    let app = app(pool).await;
    // Near place ~0.7 mi north of origin; far place ~7 mi north.
    for (name, lat) in [("Near Place", 47.6197), ("Far Place", 47.7107)] {
        let mut body = new_place_json(name);
        body["lat"] = json!(lat);
        body["lng"] = json!(-122.3422);
        let req = TestRequest::post().uri("/api/places").set_json(body).to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
    }

    let req = TestRequest::get()
        .uri("/api/places?lat=47.6097&lng=-122.3422")
        .to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(places.len(), 2);
    assert_eq!(places[0].name, "Near Place");
    let near_dist = places[0].distance_mi.expect("distance");
    assert!((near_dist - 0.69).abs() < 0.05, "got {near_dist}");

    let req = TestRequest::get()
        .uri("/api/places?lat=47.6097&lng=-122.3422&radius_mi=2")
        .to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].name, "Near Place");
}

#[sqlx::test(migrations = "./migrations")]
async fn list_filters_by_type_and_purchase(pool: PgPool) {
    let app = app(pool).await;
    let mut park = new_place_json("Elm Street Park");
    park["place_type"] = json!("park");
    park["purchase_required"] = json!("no");
    park["code_required"] = json!("no");
    for body in [new_place_json("Camber Coffee"), park] {
        let req = TestRequest::post().uri("/api/places").set_json(body).to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
    }

    let req = TestRequest::get().uri("/api/places?types=park").to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].place_type, PlaceType::Park);

    let req = TestRequest::get().uri("/api/places?no_purchase=true").to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].name, "Elm Street Park");
}

#[sqlx::test(migrations = "./migrations")]
async fn review_updates_average_and_sorting_by_cleanliness(pool: PgPool) {
    let app = app(pool).await;
    let req = TestRequest::post()
        .uri("/api/places")
        .set_json(new_place_json("Camber Coffee"))
        .to_request();
    let created: PlaceDetail = read_body_json(call_service(&app, req).await).await;
    let id = created.summary.id;

    let req = TestRequest::post()
        .uri(&format!("/api/places/{id}/reviews"))
        .set_json(json!({ "device_id": "other-device", "clean": 3, "text": "Okay." }))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let detail: PlaceDetail = read_body_json(res).await;
    assert_eq!(detail.summary.review_count, 2);
    assert_eq!(detail.summary.clean_avg, Some(4.0));
    assert_eq!(detail.reviews.len(), 2);

    let req = TestRequest::post()
        .uri(&format!("/api/places/{id}/reviews"))
        .set_json(json!({ "device_id": "other-device", "clean": 9, "text": "" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn saved_places_round_trip(pool: PgPool) {
    let app = app(pool).await;
    let req = TestRequest::post()
        .uri("/api/places")
        .set_json(new_place_json("Camber Coffee"))
        .to_request();
    let created: PlaceDetail = read_body_json(call_service(&app, req).await).await;
    let id = created.summary.id;

    let req = TestRequest::put()
        .uri(&format!("/api/devices/dev-a/saved/{id}"))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    let req = TestRequest::get().uri("/api/devices/dev-a/saved").to_request();
    let saved: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, id);

    let req = TestRequest::get().uri("/api/devices/dev-b/saved").to_request();
    let saved: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert!(saved.is_empty());

    let req = TestRequest::delete()
        .uri(&format!("/api/devices/dev-a/saved/{id}"))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    let req = TestRequest::get().uri("/api/devices/dev-a/saved").to_request();
    let saved: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert!(saved.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn list_searches_name_and_address(pool: PgPool) {
    let app = app(pool).await;
    let mut park = new_place_json("Elm Street Park");
    park["address"] = json!("Elm St & 5th");
    for body in [new_place_json("Camber Coffee"), park] {
        let req = TestRequest::post().uri("/api/places").set_json(body).to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
    }

    // Case-insensitive name match.
    let req = TestRequest::get().uri("/api/places?q=camber").to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].name, "Camber Coffee");

    // Address match, URL-encoded.
    let req = TestRequest::get().uri("/api/places?q=elm%20st").to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].name, "Elm Street Park");

    // LIKE wildcards in the query are literal, not wildcards.
    let req = TestRequest::get().uri("/api/places?q=%25").to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert!(places.is_empty());

    let req = TestRequest::get().uri("/api/places?q=nomatch").to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert!(places.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn get_unknown_place_is_404(pool: PgPool) {
    let app = app(pool).await;
    let req = TestRequest::get()
        .uri("/api/places/00000000-0000-0000-0000-00000000dead")
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);
}
