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
use shared::{
    AuthSession, Invitation, Parking, PlaceDetail, PlaceEdit, PlaceSummary, PlaceType, Requirement,
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

/// Registers `username` (via a freshly seeded invite) and returns their
/// session token.
async fn register<S, B>(app: &S, pool: &PgPool, username: &str) -> String
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let code = seed_invite(pool).await;
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({ "username": username, "password": TEST_PASSWORD, "invite_code": code }))
        .to_request();
    let res = call_service(app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let session: AuthSession = read_body_json(res).await;
    assert_eq!(session.username, username);
    session.token
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
            "invite_code": "BREAK-FAKE01",
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
async fn invited_users_can_issue_and_track_their_own_invites(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;

    // No invites yet.
    let req =
        TestRequest::get().uri("/api/invites").insert_header(auth(&token)).to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let invites: Vec<Invitation> = read_body_json(res).await;
    assert!(invites.is_empty());

    // Issuing one requires being signed in.
    let req = TestRequest::post().uri("/api/invites").to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::UNAUTHORIZED);

    let req = TestRequest::post().uri("/api/invites").insert_header(auth(&token)).to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let issued: Invitation = read_body_json(res).await;
    assert!(!issued.redeemed);

    let req =
        TestRequest::get().uri("/api/invites").insert_header(auth(&token)).to_request();
    let invites: Vec<Invitation> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(invites, vec![issued.clone()]);

    // A friend redeems it...
    let req = TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({
            "username": "scout-two",
            "password": TEST_PASSWORD,
            "invite_code": issued.code,
        }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::CREATED);

    // ...and it now shows as redeemed for the inviter.
    let req =
        TestRequest::get().uri("/api/invites").insert_header(auth(&token)).to_request();
    let invites: Vec<Invitation> = read_body_json(call_service(&app, req).await).await;
    assert_eq!(invites, vec![Invitation { code: issued.code, redeemed: true }]);
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
