use crate::{build_app, AppState};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use moka::future::Cache;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_account(pool: &PgPool) -> (Uuid, Uuid, String) {
    let account_id = Uuid::new_v4();
    let api_key_id = Uuid::new_v4();
    let secret = "supersecret123";

    sqlx::query!(
        "INSERT INTO accounts (id, email, plan) VALUES ($1, $2, 'pro')",
        account_id,
        "test@dataguard.local"
    )
    .execute(pool)
    .await
    .unwrap();

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(secret.as_bytes(), &salt).unwrap().to_string();

    sqlx::query!(
        "INSERT INTO api_keys (id, account_id, key_hash, name) VALUES ($1, $2, $3, 'Test Key')",
        api_key_id,
        account_id,
        password_hash
    )
    .execute(pool)
    .await
    .unwrap();

    let full_key = format!("dg_live_{}_{}", api_key_id, secret);
    (account_id, api_key_id, full_key)
}

async fn execute_request(app: Router, req: Request<Body>) -> Response<Body> {
    app.oneshot(req).await.unwrap()
}

#[sqlx::test(migrations = "../migrations")]
async fn test_auth_missing_key(pool: PgPool) {
    let state = AppState { db: pool, schema_cache: Cache::new(10) };
    let app = build_app(state);

    let req = Request::builder().uri("/v1/schemas").method("GET").body(Body::empty()).unwrap();

    let response = execute_request(app, req).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn test_auth_invalid_key(pool: PgPool) {
    let state = AppState { db: pool, schema_cache: Cache::new(10) };
    let app = build_app(state);

    let req = Request::builder()
        .uri("/v1/schemas")
        .method("GET")
        .header("X-Api-Key", "dg_live_invalid_key_format")
        .body(Body::empty())
        .unwrap();

    let response = execute_request(app, req).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn test_auth_valid_key(pool: PgPool) {
    let state = AppState { db: pool.clone(), schema_cache: Cache::new(10) };
    let (_, _, full_key) = setup_test_account(&pool).await;
    let app = build_app(state);

    let req = Request::builder()
        .uri("/v1/schemas")
        .method("GET")
        .header("X-Api-Key", full_key)
        .body(Body::empty())
        .unwrap();

    let response = execute_request(app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn test_validate_engine_scenarios(pool: PgPool) {
    let state = AppState { db: pool.clone(), schema_cache: Cache::new(10) };
    let (account_id, _, full_key) = setup_test_account(&pool).await;
    let schema_id = Uuid::new_v4();

    let json_schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" },
            "email": { "type": "string", "format": "email" }
        },
        "required": ["name", "age"]
    });

    sqlx::query!(
        "INSERT INTO schemas (id, account_id, name) VALUES ($1, $2, 'User Schema')",
        schema_id,
        account_id
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO schema_versions (schema_id, version, json_schema) VALUES ($1, 1, $2)",
        schema_id,
        json_schema
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = build_app(state);

    // Scenario A: Valid
    let req_body_valid = json!({
        "schema_id": schema_id,
        "data": { "name": "Alice", "age": 30, "email": "alice@local.dev" }
    });
    let req_valid = Request::builder()
        .uri("/v1/validate")
        .method("POST")
        .header("X-Api-Key", &full_key)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(req_body_valid.to_string()))
        .unwrap();
    let resp_valid = execute_request(app.clone(), req_valid).await;
    assert_eq!(resp_valid.status(), StatusCode::OK);
    let bytes = resp_valid.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["is_valid"], true);

    // Scenario B: Missing required field
    let req_body_missing = json!({
        "schema_id": schema_id,
        "data": { "name": "Bob" }
    });
    let req_missing = Request::builder()
        .uri("/v1/validate")
        .method("POST")
        .header("X-Api-Key", &full_key)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(req_body_missing.to_string()))
        .unwrap();
    let resp_missing = execute_request(app.clone(), req_missing).await;
    let bytes_missing = resp_missing.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes_missing).unwrap();
    println!("MISSING BODY RESP: {:?}", body);
    assert_eq!(body["is_valid"], false);
    assert!(body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.as_str().unwrap().contains("required")));

    // Scenario C: Wrong type
    let req_body_type = json!({
        "schema_id": schema_id,
        "data": { "name": "Charlie", "age": "vingt" }
    });
    let req_type = Request::builder()
        .uri("/v1/validate")
        .method("POST")
        .header("X-Api-Key", &full_key)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(req_body_type.to_string()))
        .unwrap();
    let resp_type = execute_request(app.clone(), req_type).await;
    let bytes_type = resp_type.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes_type).unwrap();
    assert_eq!(body["is_valid"], false);
    assert!(body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.as_str().unwrap().contains("type")));

    // Scenario D: Wrong format
    let req_body_format = json!({
        "schema_id": schema_id,
        "data": { "name": "Dave", "age": 40, "email": "not-an-email" }
    });
    let req_format = Request::builder()
        .uri("/v1/validate")
        .method("POST")
        .header("X-Api-Key", &full_key)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(req_body_format.to_string()))
        .unwrap();
    let resp_format = execute_request(app.clone(), req_format).await;
    let bytes_format = resp_format.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes_format).unwrap();
    assert_eq!(body["is_valid"], false, "FORMAT RESP BODY: {:?}", body);
    assert!(body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.as_str().unwrap().contains("email")));
}
