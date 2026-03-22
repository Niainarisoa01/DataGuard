use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use shared::models::Schema;
use uuid::Uuid;

use crate::AppState;

pub async fn list_schemas(State(state): State<AppState>) -> Result<Json<Vec<Schema>>, StatusCode> {
    let schemas = sqlx::query_as!(
        Schema,
        "SELECT id, account_id, name, description, created_at, updated_at FROM schemas"
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
    pub account_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub json_schema: Value,
}

pub async fn create_schema(
    State(state): State<AppState>,
    Json(payload): Json<CreateSchemaPayload>,
) -> Result<(StatusCode, Json<Schema>), StatusCode> {
    let mut tx = state.db.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let schema = sqlx::query_as!(
        Schema,
        "INSERT INTO schemas (account_id, name, description) VALUES ($1, $2, $3) RETURNING *",
        payload.account_id,
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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Schema>, StatusCode> {
    let schema = sqlx::query_as!(
        Schema,
        "SELECT id, account_id, name, description, created_at, updated_at FROM schemas WHERE id = $1",
        id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match schema {
        Some(s) => Ok(Json(s)),
        None => Err(StatusCode::NOT_FOUND),
    }
}
