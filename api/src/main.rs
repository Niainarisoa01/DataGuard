use axum::{
    routing::{get, post},
    Router,
    extract::DefaultBodyLimit,
};
use shared::db::create_pool;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;

use moka::future::Cache;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: shared::db::DbPool,
    pub schema_cache: Cache<Uuid, Arc<serde_json::Value>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setup connection pool
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:672643@localhost:5432/dataguard".to_string());
    let pool = create_pool(&database_url).await?;

    let schema_cache = Cache::builder()
        .max_capacity(10_000)
        .time_to_live(std::time::Duration::from_secs(60 * 60)) // 1 hour
        .build();

    let state = AppState { db: pool, schema_cache };

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/v1/schemas", get(handlers::schemas::list_schemas).post(handlers::schemas::create_schema))
        .route("/v1/schemas/:id", get(handlers::schemas::get_schema))
        .route("/v1/validate", post(handlers::validate::validate_payload))
        .route("/v1/validate/csv/:schema_id", post(handlers::csv::validate_csv))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024)) // 1GB limite pour CSV massive
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
