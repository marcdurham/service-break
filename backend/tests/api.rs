//! Integration tests exercising the HTTP API against a real Postgres.
//!
//! `#[sqlx::test]` creates a disposable database per test using the server at
//! `DATABASE_URL` (set in `.cargo/config.toml`, served by docker-compose).

use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::http::StatusCode;
use actix_web::test::{call_service, init_service, read_body, read_body_json, TestRequest};
use actix_web::web::Data;
use actix_web::App;
use backend::{handlers, http_client, AppState};
use serde_json::json;
use shared::{
    AuthSession, ImportSummary, Invitation, InviteStatus, InvitesOverview, Parking, PlaceDetail,
    PlaceEdit, PlaceSummary, PlaceType, Requirement,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn app(
    pool: PgPool,
) -> impl Service<actix_http::Request, Response = ServiceResponse<impl MessageBody>, Error = actix_web::Error>
{
    let state = Data::new(AppState {
        pool,
        http: http_client(),
        // Unroutable address: tests must not depend on the live geocoder.
        nominatim_url: "http://127.0.0.1:1".to_owned(),
        // Google sign-in isn't exercised by these tests.
        google: None,
    });
    init_service(App::new().app_data(state).configure(handlers::configure)).await
}

/// Like [`app`], but with a (fake, never actually contacted) Google OAuth
/// client configured — for exercising the parts of the Google sign-in flow
/// that don't require a live round trip to Google itself.
async fn app_with_google(
    pool: PgPool,
) -> impl Service<actix_http::Request, Response = ServiceResponse<impl MessageBody>, Error = actix_web::Error>
{
    let state = Data::new(AppState {
        pool,
        http: http_client(),
        nominatim_url: "http://127.0.0.1:1".to_owned(),
        google: Some(backend::google_auth::GoogleConfig {
            client_id: "test-client-id".to_owned(),
            client_secret: "test-client-secret".to_owned(),
            redirect_uri: "http://127.0.0.1:8020/api/auth/google/callback".to_owned(),
            app_base_url: "http://127.0.0.1:8020".to_owned(),
        }),
    });
    init_service(App::new().app_data(state).configure(handlers::configure)).await
}

const TEST_PASSWORD: &str = "correct-horse-battery";

/// Directly inserts an unredeemed, inviter-less invitation so tests can
/// register a first user in an otherwise-empty database — mirroring the
/// manual bootstrap an operator would run via `psql` on a fresh install.
async fn seed_invite(pool: &PgPool) -> String {
    let code = format!("TEST-{}", Uuid::new_v4().simple());
    sqlx::query("INSERT INTO invitations (code) VALUES ($1)")
        .bind(&code)
        .execute(pool)
        .await
        .unwrap();
    code
}

/// Registers `username` with `invite_code` and returns their session
/// token. The account is brand-new, so it can't send invitations yet.
async fn register_fresh_with_code<S, B>(app: &S, username: &str, invite_code: &str) -> String
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({
            "username": username,
            "password": TEST_PASSWORD,
            "invite_code": invite_code,
        }))
        .to_request();
    let res = call_service(app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let session: AuthSession = read_body_json(res).await;
    assert_eq!(session.username, username);
    session.token
}

/// Registers `username` (via a freshly seeded invite) and returns their
/// session token. The account's created_at is backdated two days so tests
/// can immediately send invitations (new accounts must wait 24 hours).
async fn register<S, B>(app: &S, pool: &PgPool, username: &str) -> String
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let code = seed_invite(pool).await;
    let token = register_fresh_with_code(app, username, &code).await;
    sqlx::query("UPDATE users SET created_at = now() - interval '2 days' WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await
        .unwrap();
    token
}

/// `Authorization: Bearer` header pair for [`TestRequest::insert_header`].
fn auth(token: &str) -> (&'static str, String) {
    ("Authorization", format!("Bearer {token}"))
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
        "coffee": 4,
        "door_ft": 15,
        "door_note": "Right past the counter",
        "parking": "street",
        "purchase_required": "yes",
        "code_required": "yes",
        "amenities": ["restrooms", "coffee"],
        "comment": "Spotless."
    })
}

/// An update body matching the place `new_place_json` creates, so tests
/// start from a no-op edit and mutate only the fields under test.
fn update_place_json() -> serde_json::Value {
    json!({
        "name": "Camber Coffee",
        "place_type": "shop",
        "address": "214 Maple Ave",
        "door_ft": 15,
        "door_note": "Right past the counter",
        "parking": "street",
        "purchase_required": "yes",
        "code_required": "yes",
        "amenities": ["restrooms", "coffee"],
    })
}

#[sqlx::test(migrations = "./migrations")]
async fn create_place_then_list_returns_it(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;

    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&token))
        .set_json(new_place_json("Camber Coffee"))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: PlaceDetail = read_body_json(res).await;
    assert_eq!(created.summary.name, "Camber Coffee");
    assert_eq!(created.summary.review_count, 1);
    assert_eq!(created.summary.clean_avg, Some(5.0));
    // The optional aspect scores from the first review flow into the
    // per-aspect averages; unrated aspects stay unrated.
    assert_eq!(created.summary.coffee_avg, Some(4.0));
    assert_eq!(created.summary.food_avg, None);
    assert_eq!(created.reviews.len(), 1);
    assert_eq!(created.reviews[0].text, "Spotless.");
    assert_eq!(created.reviews[0].coffee, Some(4));
    assert_eq!(created.reviews[0].food, None);
    // The review is attributed to the logged-in account, not the device.
    assert_eq!(created.reviews[0].author, "scout-one");

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
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;
    let mut body = new_place_json("Roadside Place");
    body["lat"] = json!(null);
    body["lng"] = json!(null);
    body["address"] = json!("37.7749, -122.4194");
    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&token))
        .set_json(body)
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: PlaceDetail = read_body_json(res).await;
    assert!((created.summary.lat - 37.7749).abs() < 1e-9);
    assert!((created.summary.lng + 122.4194).abs() < 1e-9);
}

#[sqlx::test(migrations = "./migrations")]
async fn create_place_without_location_is_rejected(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;
    let mut body = new_place_json("Nowhere");
    body["lat"] = json!(null);
    body["lng"] = json!(null);
    body["address"] = json!("");
    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&token))
        .set_json(body)
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn list_sorts_by_distance_and_respects_radius(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;
    // Near place ~0.7 mi north of origin; far place ~7 mi north.
    for (name, lat) in [("Near Place", 47.6197), ("Far Place", 47.7107)] {
        let mut body = new_place_json(name);
        body["lat"] = json!(lat);
        body["lng"] = json!(-122.3422);
        let req = TestRequest::post()
            .uri("/api/places")
            .insert_header(auth(&token))
            .set_json(body)
            .to_request();
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
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;
    let mut park = new_place_json("Elm Street Park");
    park["place_type"] = json!("park");
    park["purchase_required"] = json!("no");
    park["code_required"] = json!("no");
    for body in [new_place_json("Camber Coffee"), park] {
        let req = TestRequest::post()
            .uri("/api/places")
            .insert_header(auth(&token))
            .set_json(body)
            .to_request();
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
async fn list_filters_by_offered_amenities(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;
    // Camber Coffee offers restrooms + coffee (new_place_json default).
    let mut park = new_place_json("Elm Street Park");
    park["place_type"] = json!("park");
    park["amenities"] = json!(["seating"]);
    let mut kiosk = new_place_json("Bare Kiosk");
    kiosk["amenities"] = json!([]);
    for body in [new_place_json("Camber Coffee"), park, kiosk] {
        let req = TestRequest::post()
            .uri("/api/places")
            .insert_header(auth(&token))
            .set_json(body)
            .to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
    }

    // A single amenity narrows to places offering it.
    let req = TestRequest::get().uri("/api/places?amenities=coffee").to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].name, "Camber Coffee");

    // Several amenities match places offering at least one of them.
    let req = TestRequest::get()
        .uri("/api/places?amenities=coffee,seating")
        .to_request();
    let mut names: Vec<String> = read_body_json::<Vec<PlaceSummary>, _>(call_service(&app, req).await)
        .await
        .into_iter()
        .map(|p| p.name)
        .collect();
    names.sort();
    assert_eq!(names, ["Camber Coffee", "Elm Street Park"]);

    // No amenities param leaves every place visible.
    let req = TestRequest::get().uri("/api/places").to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(places.len(), 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn review_updates_average_and_sorting_by_cleanliness(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;
    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&token))
        .set_json(new_place_json("Camber Coffee"))
        .to_request();
    let created: PlaceDetail = read_body_json(call_service(&app, req).await).await;
    let id = created.summary.id;

    let other = register(&app, &pool, "scout-two").await;
    let req = TestRequest::post()
        .uri(&format!("/api/places/{id}/reviews"))
        .insert_header(auth(&other))
        .set_json(json!({
            "device_id": "other-device",
            "clean": 3,
            "coffee": 2,
            "food": 4,
            "text": "Okay.",
        }))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let detail: PlaceDetail = read_body_json(res).await;
    assert_eq!(detail.summary.review_count, 2);
    assert_eq!(detail.summary.clean_avg, Some(4.0));
    // Aspect averages only count the reviews that rated the aspect: coffee
    // was rated 4 (creation) and 2; food only once, a 4.
    assert_eq!(detail.summary.coffee_avg, Some(3.0));
    assert_eq!(detail.summary.food_avg, Some(4.0));
    assert_eq!(detail.reviews.len(), 2);
    assert_eq!(detail.reviews[0].author, "scout-two");
    assert_eq!(detail.reviews[0].coffee, Some(2));
    assert_eq!(detail.reviews[0].food, Some(4));

    // Out-of-range scores are rejected, for the required cleanliness score
    // and the optional aspect scores alike.
    for body in [
        json!({ "device_id": "other-device", "clean": 9, "text": "" }),
        json!({ "device_id": "other-device", "clean": 4, "coffee": 0, "text": "" }),
        json!({ "device_id": "other-device", "clean": 4, "food": 6, "text": "" }),
    ] {
        let req = TestRequest::post()
            .uri(&format!("/api/places/{id}/reviews"))
            .insert_header(auth(&other))
            .set_json(body)
            .to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn saved_places_round_trip(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;
    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&token))
        .set_json(new_place_json("Camber Coffee"))
        .to_request();
    let created: PlaceDetail = read_body_json(call_service(&app, req).await).await;
    let id = created.summary.id;

    let req = TestRequest::put()
        .uri(&format!("/api/devices/dev-a/saved/{id}"))
        .insert_header(auth(&token))
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
        .insert_header(auth(&token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    let req = TestRequest::get().uri("/api/devices/dev-a/saved").to_request();
    let saved: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert!(saved.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn list_searches_name_and_address(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;
    let mut park = new_place_json("Elm Street Park");
    park["address"] = json!("Elm St & 5th");
    for body in [new_place_json("Camber Coffee"), park] {
        let req = TestRequest::post()
            .uri("/api/places")
            .insert_header(auth(&token))
            .set_json(body)
            .to_request();
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
    let app = app(pool.clone()).await;
    let req = TestRequest::get()
        .uri("/api/places/00000000-0000-0000-0000-00000000dead")
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn writes_require_login(pool: PgPool) {
    let app = app(pool.clone()).await;

    // No Authorization header at all.
    let req = TestRequest::post()
        .uri("/api/places")
        .set_json(new_place_json("Sneaky Place"))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    // A made-up token is just as unauthorized.
    let req = TestRequest::post()
        .uri("/api/places/00000000-0000-0000-0000-00000000dead/reviews")
        .insert_header(auth("not-a-real-token"))
        .set_json(json!({ "device_id": "dev-a", "clean": 4, "text": "" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    let req = TestRequest::put()
        .uri("/api/devices/dev-a/saved/00000000-0000-0000-0000-00000000dead")
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    let req = TestRequest::delete()
        .uri("/api/devices/dev-a/saved/00000000-0000-0000-0000-00000000dead")
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    // Reads stay public.
    let req = TestRequest::get().uri("/api/places").to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn register_login_logout_flow(pool: PgPool) {
    let app = app(pool.clone()).await;
    register(&app, &pool, "wanderer").await;

    // Fresh login issues a working token (username matched case-insensitively).
    let req = TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({ "username": "WANDERER", "password": TEST_PASSWORD }))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let session: AuthSession = read_body_json(res).await;
    assert_eq!(session.username, "wanderer");

    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&session.token))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let me: serde_json::Value = read_body_json(res).await;
    assert_eq!(me["username"], "wanderer");

    // Logout invalidates the token.
    let req = TestRequest::post()
        .uri("/api/auth/logout")
        .insert_header(auth(&session.token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&session.token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn login_with_wrong_password_is_rejected(pool: PgPool) {
    let app = app(pool.clone()).await;
    register(&app, &pool, "wanderer").await;

    for (username, password) in [("wanderer", "wrong-password"), ("nobody", TEST_PASSWORD)] {
        let req = TestRequest::post()
            .uri("/api/auth/login")
            .set_json(json!({ "username": username, "password": password }))
            .to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn register_validates_input_and_rejects_taken_names(pool: PgPool) {
    let app = app(pool.clone()).await;
    register(&app, &pool, "wanderer").await;

    // Same name (any case) is a conflict, given a fresh valid invite (the
    // one "wanderer" used is already redeemed).
    let code = seed_invite(&pool).await;
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({ "username": "Wanderer", "password": TEST_PASSWORD, "invite_code": code }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CONFLICT);

    // Bad usernames and short passwords are 400s.
    for body in [
        json!({ "username": "ab", "password": TEST_PASSWORD }),
        json!({ "username": "has space", "password": TEST_PASSWORD }),
        json!({ "username": "fine-name", "password": "short" }),
    ] {
        let req = TestRequest::post().uri("/api/auth/register").set_json(body).to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn registration_requires_a_valid_invite_code(pool: PgPool) {
    let app = app(pool.clone()).await;

    // No invite_code field at all.
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({ "username": "nobody-invited", "password": TEST_PASSWORD }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // A made-up code.
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({
            "username": "nobody-invited",
            "password": TEST_PASSWORD,
            "invite_code": "FAKE0001",
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // An expired (8-day-old) but otherwise valid, unredeemed code.
    let stale = seed_invite(&pool).await;
    sqlx::query("UPDATE invitations SET created_at = now() - interval '8 days' WHERE code = $1")
        .bind(&stale)
        .execute(&pool)
        .await
        .unwrap();
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({
            "username": "too-late",
            "password": TEST_PASSWORD,
            "invite_code": stale,
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // A real but already-redeemed code.
    let code = seed_invite(&pool).await;
    register(&app, &pool, "scout-one").await; // burns a different, freshly seeded code
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({ "username": "scout-two", "password": TEST_PASSWORD, "invite_code": code }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({ "username": "scout-three", "password": TEST_PASSWORD, "invite_code": code }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn google_sign_in_disabled_by_default(pool: PgPool) {
    let app = app(pool.clone()).await;

    let req = TestRequest::get().uri("/api/auth/google/enabled").to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = read_body_json(res).await;
    assert_eq!(body["enabled"], false);

    let req = TestRequest::get().uri("/api/auth/google/start?mode=login").to_request();
    assert_eq!(
        call_service(&app, req).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn google_start_redirects_to_google_and_stores_state(pool: PgPool) {
    let app = app_with_google(pool.clone()).await;

    let req = TestRequest::get().uri("/api/auth/google/enabled").to_request();
    let body: serde_json::Value = read_body_json(call_service(&app, req).await).await;
    assert_eq!(body["enabled"], true);

    let req = TestRequest::get().uri("/api/auth/google/start?mode=login").to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FOUND);
    let location = res.headers().get("Location").unwrap().to_str().unwrap().to_owned();
    assert!(location.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    assert!(location.contains("client_id=test-client-id"));
    assert!(location.contains(
        "redirect_uri=http%3A%2F%2F127.0.0.1%3A8020%2Fapi%2Fauth%2Fgoogle%2Fcallback"
    ));

    // Exactly one single-use state row was stored for the round trip.
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM oauth_states").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn google_register_start_requires_an_invite_code(pool: PgPool) {
    let app = app_with_google(pool.clone()).await;
    let req = TestRequest::get().uri("/api/auth/google/start?mode=register").to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn google_callback_rejects_unknown_or_replayed_state(pool: PgPool) {
    let app = app_with_google(pool.clone()).await;

    let req = TestRequest::get()
        .uri("/api/auth/google/callback?code=abc&state=never-issued")
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FOUND);
    let location = res.headers().get("Location").unwrap().to_str().unwrap().to_owned();
    assert!(location.starts_with("http://127.0.0.1:8020/oauth-complete#error="));
}

#[sqlx::test(migrations = "./migrations")]
async fn google_callback_surfaces_denied_consent(pool: PgPool) {
    let app = app_with_google(pool.clone()).await;
    let req =
        TestRequest::get().uri("/api/auth/google/callback?error=access_denied").to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FOUND);
    let location = res.headers().get("Location").unwrap().to_str().unwrap().to_owned();
    assert!(location.starts_with("http://127.0.0.1:8020/oauth-complete#error="));
}

#[sqlx::test(migrations = "./migrations")]
async fn oauth_state_round_trips_once_then_is_gone(pool: PgPool) {
    backend::db::create_oauth_state(&pool, "state-once", "register", "INVITE1").await.unwrap();

    let (mode, invite) =
        backend::db::take_oauth_state(&pool, "state-once").await.unwrap().unwrap();
    assert_eq!(mode, "register");
    assert_eq!(invite, "INVITE1");

    // Single-use: a replay of the same state finds nothing.
    assert!(backend::db::take_oauth_state(&pool, "state-once").await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn oauth_state_expires(pool: PgPool) {
    backend::db::create_oauth_state(&pool, "state-old", "login", "").await.unwrap();
    sqlx::query("UPDATE oauth_states SET expires_at = now() - interval '1 minute' WHERE state = $1")
        .bind("state-old")
        .execute(&pool)
        .await
        .unwrap();
    assert!(backend::db::take_oauth_state(&pool, "state-old").await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_google_user_creates_then_reuses_the_same_account(pool: PgPool) {
    let code = seed_invite(&pool).await;
    let (id1, username1, is_admin1) = backend::db::upsert_google_user(
        &pool,
        "google-sub-1",
        "scout@example.com",
        "Sam",
        "Scout",
        &code,
    )
    .await
    .unwrap();
    assert!(!is_admin1);
    assert_eq!(username1, "scout");

    // Same google_sub again — signs into the same account, no invite needed
    // even though this one is blank/invalid.
    let (id2, username2, _) = backend::db::upsert_google_user(
        &pool,
        "google-sub-1",
        "scout@example.com",
        "Sam",
        "Scout",
        "",
    )
    .await
    .unwrap();
    assert_eq!(id1, id2);
    assert_eq!(username2, "scout");
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_google_user_disambiguates_username_collisions(pool: PgPool) {
    sqlx::query("INSERT INTO users (username, password_hash) VALUES ('scout', 'x')")
        .execute(&pool)
        .await
        .unwrap();

    let code = seed_invite(&pool).await;
    let (_, username, _) = backend::db::upsert_google_user(
        &pool,
        "google-sub-2",
        "scout@example.com",
        "",
        "",
        &code,
    )
    .await
    .unwrap();
    assert_eq!(username, "scout1");
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_google_user_requires_a_valid_invite_for_new_accounts(pool: PgPool) {
    let err = backend::db::upsert_google_user(
        &pool,
        "google-sub-3",
        "nobody@example.com",
        "",
        "",
        "FAKE0001",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, backend::error::ApiError::BadRequest(_)));
}

#[sqlx::test(migrations = "./migrations")]
async fn find_user_by_google_sub_only_matches_linked_accounts(pool: PgPool) {
    assert!(backend::db::find_user_by_google_sub(&pool, "nope").await.unwrap().is_none());

    let code = seed_invite(&pool).await;
    backend::db::upsert_google_user(&pool, "sub-x", "x@example.com", "", "", &code)
        .await
        .unwrap();
    assert!(backend::db::find_user_by_google_sub(&pool, "sub-x").await.unwrap().is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn google_only_account_cannot_log_in_with_a_password(pool: PgPool) {
    let app = app(pool.clone()).await;
    let code = seed_invite(&pool).await;
    let (_, username, _) = backend::db::upsert_google_user(
        &pool,
        "sub-pw-test",
        "pw-test@example.com",
        "",
        "",
        &code,
    )
    .await
    .unwrap();

    let req = TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({ "username": username, "password": TEST_PASSWORD }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn google_only_account_cannot_change_a_password(pool: PgPool) {
    let app = app(pool.clone()).await;
    let code = seed_invite(&pool).await;
    let (user_id, _, _) = backend::db::upsert_google_user(
        &pool,
        "sub-cp-test",
        "cp-test@example.com",
        "",
        "",
        &code,
    )
    .await
    .unwrap();
    let token = "test-google-only-session".to_owned();
    backend::db::create_session(&pool, &token, user_id).await.unwrap();

    let req = TestRequest::put()
        .uri("/api/auth/password")
        .insert_header(auth(&token))
        .set_json(json!({ "current_password": "whatever", "new_password": "new-password-123" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn invited_users_can_issue_and_track_their_own_invites(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;

    // No invites yet; scout-one used an inviter-less bootstrap code.
    let req =
        TestRequest::get().uri("/api/invites").insert_header(auth(&token)).to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let overview: InvitesOverview = read_body_json(res).await;
    assert!(overview.invites.is_empty());
    assert_eq!(overview.invited_by, None);
    assert!(overview.my_invite_code.is_some());

    // Issuing one requires being signed in.
    let req = TestRequest::post().uri("/api/invites").set_json(json!({})).to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({ "name": "Bobby" }))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let issued: Invitation = read_body_json(res).await;
    assert_eq!(issued.status, InviteStatus::Pending);
    assert_eq!(issued.name, "Bobby");
    assert_eq!(issued.joined_username, None);
    // Just the code — no prefix, short enough to read out loud.
    assert!(!issued.code.contains('-'), "unexpected prefix in {}", issued.code);
    assert_eq!(issued.code.len(), 8);
    assert!(issued.code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));

    let req =
        TestRequest::get().uri("/api/invites").insert_header(auth(&token)).to_request();
    let overview: InvitesOverview = read_body_json(call_service(&app, req).await).await;
    assert_eq!(overview.invites, vec![issued.clone()]);

    // A friend redeems it...
    register_fresh_with_code(&app, "scout-two", &issued.code).await;

    // ...and it now shows as joined for the inviter, with the friend's
    // username and the name the invite was created under.
    let req =
        TestRequest::get().uri("/api/invites").insert_header(auth(&token)).to_request();
    let overview: InvitesOverview = read_body_json(call_service(&app, req).await).await;
    assert_eq!(
        overview.invites,
        vec![Invitation {
            code: issued.code,
            name: "Bobby".to_owned(),
            status: InviteStatus::Joined,
            joined_username: Some("scout-two".to_owned()),
        }]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn new_accounts_can_invite_with_lower_limit(pool: PgPool) {
    let app = app(pool.clone()).await;
    let code = seed_invite(&pool).await;
    // Registered just now — not backdated like the `register` helper does.
    let token = register_fresh_with_code(&app, "newbie", &code).await;

    // New accounts can invite immediately (INVITE_WAIT_HOURS = 0),
    // but are limited to INVITES_PER_DAY_NEW per day.
    for _ in 0..shared::INVITES_PER_DAY_NEW {
        let req = TestRequest::post()
            .uri("/api/invites")
            .insert_header(auth(&token))
            .set_json(json!({}))
            .to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
    }
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({}))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // Once the account is a day old, the limit rises to INVITES_PER_DAY_OLD_AGE.
    sqlx::query("UPDATE users SET created_at = now() - interval '25 hours' WHERE username = $1")
        .bind("newbie")
        .execute(&pool)
        .await
        .unwrap();
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({}))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
}

/// Admins bypass the 24-hour age gate — they're auto-created at startup and
/// need to invite people immediately.
#[sqlx::test(migrations = "./migrations")]
async fn admin_bypasses_the_age_gate(pool: PgPool) {
    let app = app(pool.clone()).await;
    let session = login_admin(&app, &pool).await;
    let token = session.token;

    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({}))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "./migrations")]
async fn invitations_are_limited_to_five_per_day(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;

    // The `register` helper backdates accounts 2 days, so they get the
    // older-user limit of INVITES_PER_DAY_OLD_AGE per day.
    for _ in 0..shared::INVITES_PER_DAY_OLD_AGE {
        let req = TestRequest::post()
            .uri("/api/invites")
            .insert_header(auth(&token))
            .set_json(json!({}))
            .to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
    }
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({}))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // Yesterday's invitations don't count against today.
    sqlx::query(
        "UPDATE invitations SET created_at = now() - interval '2 days' \
         WHERE inviter_id IS NOT NULL",
    )
    .execute(&pool)
    .await
    .unwrap();
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({}))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "./migrations")]
async fn invited_user_can_rename_their_invitation(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;

    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({ "name": "Bobby" }))
        .to_request();
    let issued: Invitation = read_body_json(call_service(&app, req).await).await;

    // The inviter can rename a pending code.
    let req = TestRequest::put()
        .uri(&format!("/api/invites/{}/name", issued.code))
        .insert_header(auth(&token))
        .set_json(json!({ "name": "Robert" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    let friend = register_fresh_with_code(&app, "scout-two", &issued.code).await;

    // The friend sees who invited them and the name on their invitation.
    let req = TestRequest::get().uri("/api/invites").insert_header(auth(&friend)).to_request();
    let overview: InvitesOverview = read_body_json(call_service(&app, req).await).await;
    assert_eq!(overview.invited_by, Some("scout-one".to_owned()));
    assert_eq!(overview.my_invite_code, Some(issued.code.clone()));
    assert_eq!(overview.my_invite_name, Some("Robert".to_owned()));

    // Once registered, the friend can change that name...
    let req = TestRequest::put()
        .uri(&format!("/api/invites/{}/name", issued.code))
        .insert_header(auth(&friend))
        .set_json(json!({ "name": "Bob" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    // ...the inviter can no longer rename the redeemed code...
    let req = TestRequest::put()
        .uri(&format!("/api/invites/{}/name", issued.code))
        .insert_header(auth(&token))
        .set_json(json!({ "name": "Hijack" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);

    // ...and a stranger never could.
    let stranger = register(&app, &pool, "scout-three").await;
    let req = TestRequest::put()
        .uri(&format!("/api/invites/{}/name", issued.code))
        .insert_header(auth(&stranger))
        .set_json(json!({ "name": "Nope" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);

    // Both sides see the friend's chosen name.
    let req = TestRequest::get().uri("/api/invites").insert_header(auth(&token)).to_request();
    let overview: InvitesOverview = read_body_json(call_service(&app, req).await).await;
    assert_eq!(overview.invites[0].name, "Bob");
    assert_eq!(overview.invites[0].joined_username, Some("scout-two".to_owned()));
    let req = TestRequest::get().uri("/api/invites").insert_header(auth(&friend)).to_request();
    let overview: InvitesOverview = read_body_json(call_service(&app, req).await).await;
    assert_eq!(overview.my_invite_name, Some("Bob".to_owned()));
}

#[sqlx::test(migrations = "./migrations")]
async fn pending_invites_show_expired_after_seven_days(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;

    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({}))
        .to_request();
    let issued: Invitation = read_body_json(call_service(&app, req).await).await;

    sqlx::query("UPDATE invitations SET created_at = now() - interval '8 days' WHERE code = $1")
        .bind(&issued.code)
        .execute(&pool)
        .await
        .unwrap();

    let req = TestRequest::get().uri("/api/invites").insert_header(auth(&token)).to_request();
    let overview: InvitesOverview = read_body_json(call_service(&app, req).await).await;
    assert_eq!(overview.invites[0].status, InviteStatus::Expired);
}

/// Creates "Camber Coffee" as `scout-one` and returns (editor's token, id):
/// the place is made by one account and edited by another, proving any
/// logged-in user can edit and the audit rows name the actual editor.
async fn place_for_editing<S, B>(app: &S, pool: &PgPool) -> (String, Uuid)
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let owner = register(app, pool, "scout-one").await;
    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&owner))
        .set_json(new_place_json("Camber Coffee"))
        .to_request();
    let created: PlaceDetail = read_body_json(call_service(app, req).await).await;
    let editor = register(app, pool, "scout-two").await;
    (editor, created.summary.id)
}

#[sqlx::test(migrations = "./migrations")]
async fn edit_place_updates_fields_and_logs_who_changed_what(pool: PgPool) {
    let app = app(pool.clone()).await;
    let (editor, id) = place_for_editing(&app, &pool).await;

    // No audit rows before any edit.
    let req = TestRequest::get().uri(&format!("/api/places/{id}/edits")).to_request();
    let edits: Vec<PlaceEdit> = read_body_json(call_service(&app, req).await).await;
    assert!(edits.is_empty());

    // Change the name, parking, amenities and hours; keep the rest
    // (including the address, which must not trigger geocoding — the test
    // geocoder is unroutable).
    let mut body = update_place_json();
    body["name"] = json!("Camber Coffee House");
    body["parking"] = json!("easy");
    body["amenities"] = json!(["restrooms", "coffee", "seating"]);
    body["hours"] = json!("Open · closes 9:00 PM");
    let req = TestRequest::put()
        .uri(&format!("/api/places/{id}"))
        .insert_header(auth(&editor))
        .set_json(body.clone())
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let detail: PlaceDetail = read_body_json(res).await;
    assert_eq!(detail.summary.name, "Camber Coffee House");
    assert_eq!(detail.summary.parking, Parking::Easy);
    assert_eq!(detail.summary.amenities.len(), 3);
    assert_eq!(detail.hours.as_deref(), Some("Open · closes 9:00 PM"));
    // Untouched fields survive the edit.
    assert_eq!(detail.summary.address, "214 Maple Ave");
    assert!((detail.summary.lat - 47.6117).abs() < 1e-9);
    assert_eq!(detail.summary.review_count, 1);

    // One audit row per changed field, attributed to the editor.
    let req = TestRequest::get().uri(&format!("/api/places/{id}/edits")).to_request();
    let edits: Vec<PlaceEdit> = read_body_json(call_service(&app, req).await).await;
    let mut fields: Vec<&str> = edits.iter().map(|e| e.field.as_str()).collect();
    fields.sort_unstable();
    assert_eq!(fields, vec!["amenities", "hours", "name", "parking"]);
    assert!(edits.iter().all(|e| e.author == "scout-two"));
    let name_edit = edits.iter().find(|e| e.field == "name").unwrap();
    assert_eq!(name_edit.old_value, "Camber Coffee");
    assert_eq!(name_edit.new_value, "Camber Coffee House");
    let hours_edit = edits.iter().find(|e| e.field == "hours").unwrap();
    assert_eq!(hours_edit.old_value, "");
    assert_eq!(hours_edit.new_value, "Open · closes 9:00 PM");

    // Submitting the identical state again is a no-op: no new audit rows.
    let req = TestRequest::put()
        .uri(&format!("/api/places/{id}"))
        .insert_header(auth(&editor))
        .set_json(body)
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::OK);
    let req = TestRequest::get().uri(&format!("/api/places/{id}/edits")).to_request();
    let after: Vec<PlaceEdit> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(after.len(), edits.len());
}

#[sqlx::test(migrations = "./migrations")]
async fn edit_place_relocates_from_latlng_address(pool: PgPool) {
    let app = app(pool.clone()).await;
    let (editor, id) = place_for_editing(&app, &pool).await;

    let mut body = update_place_json();
    body["address"] = json!("37.7749, -122.4194");
    let req = TestRequest::put()
        .uri(&format!("/api/places/{id}"))
        .insert_header(auth(&editor))
        .set_json(body)
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let detail: PlaceDetail = read_body_json(res).await;
    assert!((detail.summary.lat - 37.7749).abs() < 1e-9);
    assert!((detail.summary.lng + 122.4194).abs() < 1e-9);

    // Both the address and the derived location are audited.
    let req = TestRequest::get().uri(&format!("/api/places/{id}/edits")).to_request();
    let edits: Vec<PlaceEdit> = read_body_json(call_service(&app, req).await).await;
    let mut fields: Vec<&str> = edits.iter().map(|e| e.field.as_str()).collect();
    fields.sort_unstable();
    assert_eq!(fields, vec!["address", "location"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn edit_place_validates_input_and_requires_login(pool: PgPool) {
    let app = app(pool.clone()).await;
    let (editor, id) = place_for_editing(&app, &pool).await;

    // Editing is a write: no session, no edit.
    let req = TestRequest::put()
        .uri(&format!("/api/places/{id}"))
        .set_json(update_place_json())
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    // A blank name is rejected.
    let mut body = update_place_json();
    body["name"] = json!("   ");
    let req = TestRequest::put()
        .uri(&format!("/api/places/{id}"))
        .insert_header(auth(&editor))
        .set_json(body)
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // Unknown places 404 for both the edit and its history.
    let missing = "00000000-0000-0000-0000-00000000dead";
    let req = TestRequest::put()
        .uri(&format!("/api/places/{missing}"))
        .insert_header(auth(&editor))
        .set_json(update_place_json())
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);
    let req = TestRequest::get().uri(&format!("/api/places/{missing}/edits")).to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);
}

/// Logs in as the startup-created admin account with its documented
/// default password, returning the session.
async fn login_admin<S, B>(app: &S, pool: &PgPool) -> AuthSession
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    backend::admin::ensure_admin(pool).await.unwrap();
    let req = TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({
            "username": backend::admin::ADMIN_USERNAME,
            "password": backend::admin::DEFAULT_ADMIN_PASSWORD,
        }))
        .to_request();
    let res = call_service(app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    read_body_json(res).await
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_account_logs_in_with_default_password(pool: PgPool) {
    let app = app(pool.clone()).await;
    // Twice: creating the account is idempotent across restarts.
    backend::admin::ensure_admin(&pool).await.unwrap();
    let session = login_admin(&app, &pool).await;
    assert!(session.is_admin);
    assert_eq!(session.username, "admin");

    // /me reports the flag, and regular accounts don't have it.
    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&session.token))
        .to_request();
    let me: serde_json::Value = read_body_json(call_service(&app, req).await).await;
    assert_eq!(me["is_admin"], true);

    let token = register(&app, &pool, "scout-one").await;
    let req = TestRequest::get().uri("/api/auth/me").insert_header(auth(&token)).to_request();
    let me: serde_json::Value = read_body_json(call_service(&app, req).await).await;
    assert_eq!(me["is_admin"], false);
}

/// A syntactically valid, empty backup document.
fn empty_backup() -> serde_json::Value {
    json!({
        "format_version": 1,
        "exported_at": "2026-01-01T00:00:00Z",
        "users": [],
        "invitations": [],
        "places": [],
        "reviews": [],
        "saved_places": [],
    })
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_endpoints_reject_anonymous_and_non_admin_callers(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;

    let req = TestRequest::get().uri("/api/admin/export").to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    let req =
        TestRequest::get().uri("/api/admin/export").insert_header(auth(&token)).to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::FORBIDDEN);

    let req = TestRequest::post()
        .uri("/api/admin/import")
        .insert_header(auth(&token))
        .set_json(empty_backup())
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_export_import_round_trip(pool: PgPool) {
    let app = app(pool.clone()).await;
    let admin = login_admin(&app, &pool).await;

    // A scout adds a place (with its first review), saves it, and mints an
    // invite — so every exported table has something in it.
    let scout = register(&app, &pool, "scout-one").await;
    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&scout))
        .set_json(new_place_json("Camber Coffee"))
        .to_request();
    let created: PlaceDetail = read_body_json(call_service(&app, req).await).await;
    let place_id = created.summary.id;
    let req = TestRequest::put()
        .uri(&format!("/api/devices/dev-a/saved/{place_id}"))
        .insert_header(auth(&scout))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&scout))
        .set_json(json!({}))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);

    // Export: one JSON document — and no password material in it.
    let req = TestRequest::get()
        .uri("/api/admin/export")
        .insert_header(auth(&admin.token))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_body(res).await;
    let text = std::str::from_utf8(&body).unwrap().to_owned();
    assert!(!text.contains("password_hash"));
    assert!(!text.contains("$argon2"));
    let export: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(export["users"].as_array().unwrap().len(), 2); // admin + scout
    assert_eq!(export["places"].as_array().unwrap().len(), 1);
    assert_eq!(export["reviews"].as_array().unwrap().len(), 1);
    assert_eq!(export["saved_places"].as_array().unwrap().len(), 1);
    // The (redeemed) registration invite plus the fresh one.
    assert_eq!(export["invitations"].as_array().unwrap().len(), 2);

    // Simulate "export → re-deploy → restore": wipe everything except the
    // admin account (a fresh install recreates it at startup) and import.
    for sql in [
        "DELETE FROM saved_places",
        "DELETE FROM reviews",
        "DELETE FROM places",
        "DELETE FROM invitations",
        "DELETE FROM users WHERE username <> 'admin'",
    ] {
        sqlx::query(sql).execute(&pool).await.unwrap();
    }

    let req = TestRequest::post()
        .uri("/api/admin/import")
        .insert_header(auth(&admin.token))
        .set_json(export.clone())
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let summary: ImportSummary = read_body_json(res).await;
    assert_eq!(summary.users, 2);
    assert_eq!(summary.places, 1);
    assert_eq!(summary.reviews, 1);
    assert_eq!(summary.saved_places, 1);
    assert_eq!(summary.invitations, 2);
    // Only the recreated scout got a fresh password; the admin (matched by
    // username) kept the one it had.
    assert_eq!(summary.new_passwords.len(), 1);
    let new_password = summary.new_passwords.get("scout-one").unwrap().clone();

    // The place is back under the same id, review author intact.
    let req = TestRequest::get().uri(&format!("/api/places/{place_id}")).to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let detail: PlaceDetail = read_body_json(res).await;
    assert_eq!(detail.summary.name, "Camber Coffee");
    assert_eq!(detail.summary.review_count, 1);
    assert_eq!(detail.reviews[0].author, "scout-one");

    // The scout's old password no longer works; the generated one does.
    let req = TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({ "username": "scout-one", "password": TEST_PASSWORD }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);
    let req = TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({ "username": "scout-one", "password": new_password }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::OK);

    // The admin still works too (session survived, password unchanged).
    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&admin.token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::OK);

    // Backups from an incompatible future format are rejected.
    let mut bad = export;
    bad["format_version"] = json!(999);
    let req = TestRequest::post()
        .uri("/api/admin/import")
        .insert_header(auth(&admin.token))
        .set_json(bad)
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn change_password_works(pool: PgPool) {
    let app = app(pool.clone()).await;
    let code = seed_invite(&pool).await;
    let token = register_fresh_with_code(&app, "alice", &code).await;

    // Wrong current password is rejected.
    let req = TestRequest::put()
        .uri("/api/auth/password")
        .insert_header(auth(&token))
        .set_json(json!({
            "current_password": "wrong-password",
            "new_password": "brand-new-secret"
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // Empty current password is rejected.
    let req = TestRequest::put()
        .uri("/api/auth/password")
        .insert_header(auth(&token))
        .set_json(json!({
            "current_password": "",
            "new_password": "brand-new-secret"
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // Empty new password is rejected.
    let req = TestRequest::put()
        .uri("/api/auth/password")
        .insert_header(auth(&token))
        .set_json(json!({
            "current_password": "correct-horse-battery",
            "new_password": ""
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // Successful change returns 204.
    let req = TestRequest::put()
        .uri("/api/auth/password")
        .insert_header(auth(&token))
        .set_json(json!({
            "current_password": "correct-horse-battery",
            "new_password": "brand-new-secret"
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    // Old token still works (sessions aren't rotated).
    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::OK);

    // Old password no longer works.
    let req = TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({
            "username": "alice",
            "password": "correct-horse-battery"
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    // New password works.
    let req = TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({
            "username": "alice",
            "password": "brand-new-secret"
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn revoked_invite_cannot_be_redeemed(pool: PgPool) {
    let app = app(pool.clone()).await;
    let inviter_token = register(&app, &pool, "inviter").await;

    // Create an invitation.
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&inviter_token))
        .set_json(json!({ "name": "Charlie" }))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let issued: Invitation = read_body_json(res).await;

    // Revoke it.
    let req = TestRequest::delete()
        .uri(&format!("/api/invites/{}", issued.code))
        .insert_header(auth(&inviter_token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    // Try to redeem the revoked code — should fail.
    let res = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({
            "username": "charlie",
            "password": TEST_PASSWORD,
            "invite_code": issued.code
        }))
        .to_request();
    assert_eq!(call_service(&app, res).await.status(), StatusCode::BAD_REQUEST);

    // Verify invitation status is now Expired.
    let req = TestRequest::get()
        .uri("/api/invites")
        .insert_header(auth(&inviter_token))
        .to_request();
    let overview: InvitesOverview = read_body_json(call_service(&app, req).await).await;
    assert_eq!(overview.invites[0].status, InviteStatus::Expired);
}

#[sqlx::test(migrations = "./migrations")]
async fn revoke_rejects_unauthorized_user(pool: PgPool) {
    let app = app(pool.clone()).await;
    // Create two users.
    let _token_a = register(&app, &pool, "alice-revoke").await;
    let token_b = register(&app, &pool, "bob-revoke").await;
    // Alice issues an invite.
    let code = seed_invite(&pool).await;
    // Bob tries to revoke it — should fail with 404 (not found from his perspective).
    let req = TestRequest::delete()
        .uri(&format!("/api/invites/{code}"))
        .insert_header(auth(&token_b))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn revoke_redeemed_invite_fails(pool: PgPool) {
    let app = app(pool.clone()).await;
    let inviter_token = register(&app, &pool, "inviter-redeem").await;
    // Create and redeem an invitation.
    let code = seed_invite(&pool).await;
    let res = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({
            "username": "redeemed-user",
            "password": TEST_PASSWORD,
            "invite_code": code
        }))
        .to_request();
    assert_eq!(call_service(&app, res).await.status(), StatusCode::CREATED);
    // Try to revoke the now-redeemed invitation — should fail.
    let req = TestRequest::delete()
        .uri(&format!("/api/invites/{code}"))
        .insert_header(auth(&inviter_token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn revoke_nonexistent_invite_returns_404(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "alice-nonexist").await;
    // Try to revoke a code that doesn't exist.
    let fake_code = format!("nonexistent-{}", uuid::Uuid::new_v4());
    let req = TestRequest::delete()
        .uri(&format!("/api/invites/{fake_code}"))
        .insert_header(auth(&token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_rejects_empty_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile").await;
    // Try to update with empty given name.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_rejects_too_long_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-long").await;
    // Try to update with a name that's too long.
    let long_name = "a".repeat(41);
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": long_name }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_accepts_valid_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-valid").await;
    // Update with valid given name.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "Alice" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    // Verify the name was saved by checking /me.
    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&token))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = read_body_json(res).await;
    assert_eq!(body["given_name"], "Alice");
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_accepts_both_names(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-both").await;
    // Update with both given and family names.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "Alice", "family_name": "Smith" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    // Verify both names were saved.
    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&token))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = read_body_json(res).await;
    assert_eq!(body["given_name"], "Alice");
    assert_eq!(body["family_name"], "Smith");
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_partial_family_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-partial").await;
    // Update with only family name (no given name).
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "family_name": "Jones" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    // Verify family name was saved and given name is still empty/null.
    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&token))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = read_body_json(res).await;
    // given_name should be either null or empty string
    assert!(body["given_name"].is_null() || body["given_name"].as_str().is_none_or(|s| s.is_empty()));
    assert_eq!(body["family_name"], "Jones");
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_rejects_both_empty_names(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-both-empty").await;
    // Try to update with both names empty.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "", "family_name": "" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_accepts_special_characters_in_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-special").await;
    // Update with special characters in name.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "O'Brien", "family_name": "Smith-Jones" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_accepts_unicode_names(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-unicode").await;
    // Update with Unicode characters in name.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "José", "family_name": "García" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_trims_whitespace_in_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-trim").await;
    // Update with leading/trailing whitespace in name.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "  Alice  ", "family_name": " Smith " }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_rejects_whitespace_only_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-whitespace").await;
    // Try to update with only whitespace in name.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "   " }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_accepts_internal_whitespace_in_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-iw").await;
    // Update with internal whitespace in name.
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "Mary Jane" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
}


#[sqlx::test(migrations = "./migrations")]
async fn admin_invites_bypass_daily_limit(pool: PgPool) {
    let app = app(pool.clone()).await;
    // Create an admin user.
    let token = register(&app, &pool, "admin-test").await;
    sqlx::query("UPDATE users SET is_admin = true WHERE username = $1")
        .bind("admin-test")
        .execute(&pool)
        .await
        .unwrap();

    // Send more than the daily limit (25).
    for i in 0..30 {
        let req = TestRequest::post()
            .uri("/api/invites")
            .insert_header(auth(&token))
            .set_json(json!({ "name": format!("Test invite {i}") }))
            .to_request();
        assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
    }

    // Verify all 30 were created.
    let req = TestRequest::get()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .to_request();
    let overview: InvitesOverview = read_body_json(call_service(&app, req).await).await;
    assert_eq!(overview.invites.len(), 30);
}


#[sqlx::test(migrations = "./migrations")]
async fn invite_name_too_long_rejected(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-name-len").await;
    // Create an invitation with a name that's too long (41 chars).
    let long_name = format!("{}{}", "a".repeat(20), "b".repeat(21));
    assert_eq!(long_name.chars().count(), 41);
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({ "name": long_name }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // But a name at the limit (40 chars) should work.
    let ok_name = format!("{}.{}", "a".repeat(20), "b".repeat(19));
    assert_eq!(ok_name.chars().count(), 40);
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({ "name": ok_name }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_username_rejected(pool: PgPool) {
    let app = app(pool.clone()).await;
    // Register first user.
    let code_a = seed_invite(&pool).await;
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({
            "username": "unique-user",
            "password": TEST_PASSWORD,
            "invite_code": code_a,
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);

    // Try to register same username with a different invite code.
    let code_b = seed_invite(&pool).await;
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({
            "username": "unique-user",
            "password": TEST_PASSWORD,
            "invite_code": code_b,
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_profile_partial_given_name(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-profile-given").await;
    // Update with only given name (no family name).
    let req = TestRequest::patch()
        .uri("/api/auth/profile")
        .insert_header(auth(&token))
        .set_json(json!({ "given_name": "John" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    // Verify given name was saved and family name is still empty/null.
    let req = TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(auth(&token))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = read_body_json(res).await;
    assert_eq!(body["given_name"], "John");
    // family_name should be either null or empty string
    assert!(body["family_name"].is_null() || body["family_name"].as_str().is_none_or(|s| s.is_empty()));
}

#[sqlx::test(migrations = "./migrations")]
async fn rename_invite_authorization(pool: PgPool) {
    let app = app(pool.clone()).await;
    // Alice creates an invite via the API (sets inviter_id).
    let alice_token = register(&app, &pool, "rename-alice").await;
    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&alice_token))
        .set_json(json!({ "name": "Original" }))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let code_a: Invitation = read_body_json(res).await;

    // Alice can rename her own pending invite.
    let req = TestRequest::put()
        .uri(&format!("/api/invites/{}/name", code_a.code))
        .insert_header(auth(&alice_token))
        .set_json(json!({ "name": "RenamedByAlice" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    // Bob (non-inviter, non-redeemer) cannot rename it.
    let bob_token = register(&app, &pool, "rename-bob").await;
    let req = TestRequest::put()
        .uri(&format!("/api/invites/{}/name", code_a.code))
        .insert_header(auth(&bob_token))
        .set_json(json!({ "name": "Hacked" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);

    // Redeemer can rename it.
    let code_b = seed_invite(&pool).await;
    let redeemer_token = register_fresh_with_code(&app, "redeemer-user", &code_b).await;
    let req = TestRequest::put()
        .uri(&format!("/api/invites/{code_b}/name"))
        .insert_header(auth(&redeemer_token))
        .set_json(json!({ "name": "MyInviteName" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_place_soft_deletes_and_hides(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-delete").await;

    // Create a place.
    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&token))
        .set_json(new_place_json("To Be Deleted"))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: PlaceDetail = read_body_json(res).await;
    let place_id = created.summary.id;

    // It shows up in the list.
    let req = TestRequest::get()
        .uri("/api/places")
        .to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert!(places.iter().any(|p| p.id == place_id));

    // DELETE returns 204.
    let req = TestRequest::delete()
        .uri(&format!("/api/places/{place_id}"))
        .insert_header(auth(&token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NO_CONTENT);

    // No longer in the list.
    let req = TestRequest::get()
        .uri("/api/places")
        .to_request();
    let places: Vec<PlaceSummary> = read_body_json(call_service(&app, req).await).await;
    assert!(!places.iter().any(|p| p.id == place_id));

    // GET /api/places/{id} returns 404.
    let req = TestRequest::get()
        .uri(&format!("/api/places/{place_id}"))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);

    // Row still exists in the database (soft delete).
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM places WHERE id = $1")
        .bind(place_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // deleted_at and deleted_by are set.
    let row: (Option<chrono::DateTime<chrono::Utc>>, Option<Uuid>) = sqlx::query_as(
        "SELECT deleted_at, deleted_by FROM places WHERE id = $1",
    )
    .bind(place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.0.is_some());
    assert_eq!(row.1, Some(token_user_id(&pool, "scout-delete").await));
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_place_requires_login(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-del-auth").await;

    let req = TestRequest::post()
        .uri("/api/places")
        .insert_header(auth(&token))
        .set_json(new_place_json("Protected"))
        .to_request();
    let created: PlaceDetail = read_body_json(call_service(&app, req).await).await;
    let place_id = created.summary.id;

    // Unauthenticated DELETE is rejected.
    let req = TestRequest::delete()
        .uri(&format!("/api/places/{place_id}"))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_place_nonexistent_returns_404(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-del-nf").await;
    let fake_id = Uuid::new_v4();

    let req = TestRequest::delete()
        .uri(&format!("/api/places/{fake_id}"))
        .insert_header(auth(&token))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::NOT_FOUND);
}

/// Helper: look up a user's id by username from the test pool.
async fn token_user_id(pool: &PgPool, username: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users WHERE username = $1")
        .bind(username)
        .fetch_one(pool)
        .await
        .unwrap()
}
