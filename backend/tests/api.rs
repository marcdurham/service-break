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
    AuthSession, Invitation, InviteStatus, InvitesOverview, Parking, PlaceDetail, PlaceSummary,
    PlaceType, Requirement, INVITES_PER_DAY,
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
    assert_eq!(created.reviews.len(), 1);
    assert_eq!(created.reviews[0].text, "Spotless.");
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
        .set_json(json!({ "device_id": "other-device", "clean": 3, "text": "Okay." }))
        .to_request();
    let res = call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let detail: PlaceDetail = read_body_json(res).await;
    assert_eq!(detail.summary.review_count, 2);
    assert_eq!(detail.summary.clean_avg, Some(4.0));
    assert_eq!(detail.reviews.len(), 2);
    assert_eq!(detail.reviews[0].author, "scout-two");

    let req = TestRequest::post()
        .uri(&format!("/api/places/{id}/reviews"))
        .insert_header(auth(&other))
        .set_json(json!({ "device_id": "other-device", "clean": 9, "text": "" }))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);
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
async fn new_accounts_wait_a_day_before_inviting(pool: PgPool) {
    let app = app(pool.clone()).await;
    let code = seed_invite(&pool).await;
    // Registered just now — not backdated like the `register` helper does.
    let token = register_fresh_with_code(&app, "newbie", &code).await;

    let req = TestRequest::post()
        .uri("/api/invites")
        .insert_header(auth(&token))
        .set_json(json!({}))
        .to_request();
    assert_eq!(call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    // Once the account is a day old, inviting works.
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

#[sqlx::test(migrations = "./migrations")]
async fn invitations_are_limited_to_five_per_day(pool: PgPool) {
    let app = app(pool.clone()).await;
    let token = register(&app, &pool, "scout-one").await;

    for _ in 0..INVITES_PER_DAY {
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
