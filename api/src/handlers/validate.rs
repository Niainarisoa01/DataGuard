use axum::{extract::State, Json, http::StatusCode};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;

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
    State(state): State<AppState>,
    Json(payload): Json<ValidateRequest>,
) -> Result<Json<ValidateResponse>, StatusCode> {
    
    // Check cache first
    let schema_json = if let Some(cached_schema) = state.schema_cache.get(&payload.schema_id).await {
        cached_schema
    } else {
        // Fetch from DB
        let record = sqlx::query!(
            "SELECT json_schema FROM schema_versions WHERE schema_id = $1 ORDER BY version DESC LIMIT 1",
            payload.schema_id
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
    
    let is_valid = compiled_schema.is_valid(&payload.data);

    let mut errors = None;
    if !is_valid {
        let result = compiled_schema.validate(&payload.data);
        if let Err(validation_errors) = result {
            let error_strings: Vec<String> = validation_errors
                .map(|e| format!("{}", e))
                .collect();
            errors = Some(error_strings);
        }
    }

    Ok(Json(ValidateResponse { is_valid, errors }))
}
