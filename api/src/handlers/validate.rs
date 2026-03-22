use crate::handlers::auth::AuthenticatedAccount;
use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub schema_id: Uuid,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub is_valid: bool,
    pub errors: Option<Vec<String>>,
}

pub async fn validate_payload(
    Extension(account): Extension<AuthenticatedAccount>,
    State(state): State<AppState>,
    Json(payload): Json<ValidateRequest>,
) -> Result<Json<ValidateResponse>, StatusCode> {
    let schema_json = if let Some(cached_schema) = state.schema_cache.get(&payload.schema_id).await
    {
        cached_schema
    } else {
        let record = sqlx::query!(
            "SELECT sv.json_schema FROM schema_versions sv JOIN schemas s ON s.id = sv.schema_id WHERE sv.schema_id = $1 AND s.account_id = $2 ORDER BY sv.version DESC LIMIT 1",
            payload.schema_id,
            account.account_id
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        match record {
            Some(r) => {
                let arc_schema = Arc::new(r.json_schema);
                state.schema_cache.insert(payload.schema_id, arc_schema.clone()).await;
                arc_schema
            }
            None => return Err(StatusCode::NOT_FOUND),
        }
    };

    let compiled_schema = JSONSchema::compile(&schema_json).map_err(|e| {
        tracing::error!("Invalid schema compilation: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let start_time = std::time::Instant::now();
    let is_valid = compiled_schema.is_valid(&payload.data);

    let mut errors = None;
    if !is_valid {
        let result = compiled_schema.validate(&payload.data);
        if let Err(validation_errors) = result {
            let error_strings: Vec<String> = validation_errors.map(|e| format!("{}", e)).collect();
            errors = Some(error_strings);
        }
    }

    let duration_ms = start_time.elapsed().as_millis() as i32;

    // Log usage
    let db = state.db.clone();
    tokio::spawn(async move {
        let _ = sqlx::query!(
            "INSERT INTO usage_logs (account_id, api_key_id, schema_id, endpoint, status_code, is_valid, duration_ms)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            account.account_id,
            account.api_key_id,
            payload.schema_id,
            "/v1/validate",
            200,
            is_valid,
            duration_ms
        )
        .execute(&db)
        .await;
    });

    Ok(Json(ValidateResponse { is_valid, errors }))
}
