use crate::handlers::auth::AuthenticatedAccount;
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use shared::models::Schema;
use uuid::Uuid;

use crate::AppState;

pub async fn list_schemas(
    Extension(account): Extension<AuthenticatedAccount>,
    State(state): State<AppState>,
) -> Result<Json<Vec<Schema>>, StatusCode> {
    let schemas = sqlx::query_as!(
        Schema,
        "SELECT id, account_id, name, description, created_at, updated_at FROM schemas WHERE account_id = $1",
        account.account_id
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(schemas))
}

#[derive(Deserialize)]
pub struct CreateSchemaPayload {
    pub name: String,
    pub description: Option<String>,
    pub json_schema: Value,
}

pub async fn create_schema(
    Extension(account): Extension<AuthenticatedAccount>,
    State(state): State<AppState>,
    Json(payload): Json<CreateSchemaPayload>,
) -> Result<(StatusCode, Json<Schema>), StatusCode> {
    let mut tx = state.db.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let schema = sqlx::query_as!(
        Schema,
        "INSERT INTO schemas (account_id, name, description) VALUES ($1, $2, $3) RETURNING *",
        account.account_id,
        payload.name,
        payload.description
    )
    .fetch_one(&mut *tx)
    .await;

    let schema = match schema {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Error inserting schema: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let _version = sqlx::query!(
        "INSERT INTO schema_versions (schema_id, version, json_schema) VALUES ($1, $2, $3)",
        schema.id,
        1,
        payload.json_schema
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Error inserting schema version: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(schema)))
}

pub async fn get_schema(
    Extension(account): Extension<AuthenticatedAccount>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Schema>, StatusCode> {
    let schema = sqlx::query_as!(
        Schema,
        "SELECT id, account_id, name, description, created_at, updated_at FROM schemas WHERE id = $1 AND account_id = $2",
        id, account.account_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match schema {
        Some(s) => Ok(Json(s)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn update_schema(
    Extension(account): Extension<AuthenticatedAccount>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateSchemaPayload>,
) -> Result<Json<Schema>, StatusCode> {
    let mut tx = state.db.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let schema = sqlx::query_as!(
        Schema,
        "SELECT id, account_id, name, description, created_at, updated_at FROM schemas WHERE id = $1 AND account_id = $2",
        id, account.account_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if schema.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let schema = sqlx::query_as!(
        Schema,
        "UPDATE schemas SET name = $1, description = $2, updated_at = NOW() WHERE id = $3 RETURNING *",
        payload.name,
        payload.description,
        id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let record = sqlx::query!(
        "SELECT COALESCE(MAX(version), 0) as max_v FROM schema_versions WHERE schema_id = $1",
        id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let next_version = record.max_v.unwrap_or(0) + 1;

    sqlx::query!(
        "INSERT INTO schema_versions (schema_id, version, json_schema) VALUES ($1, $2, $3)",
        id,
        next_version,
        payload.json_schema
    )
    .execute(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.schema_cache.invalidate(&id).await;

    Ok(Json(schema))
}

pub async fn delete_schema(
    Extension(account): Extension<AuthenticatedAccount>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query!(
        "DELETE FROM schemas WHERE id = $1 AND account_id = $2",
        id,
        account.account_id
    )
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    state.schema_cache.invalidate(&id).await;
    Ok(StatusCode::NO_CONTENT)
}
