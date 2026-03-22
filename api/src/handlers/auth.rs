use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;
use crate::AppState;

#[derive(Clone, Debug)]
pub struct AuthenticatedAccount {
    pub account_id: Uuid,
    pub api_key_id: Uuid,
}

// Key format expected: dg_live_<api_key_id_uuid>_<secret>
pub async fn auth_middleware(
    state: axum::extract::State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request.headers().get("X-Api-Key");

    let api_key = match auth_header {
        Some(header_val) => header_val.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    if !api_key.starts_with("dg_live_") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let parts: Vec<&str> = api_key["dg_live_".len()..].split('_').collect();
    if parts.len() != 2 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let key_id = Uuid::parse_str(parts[0]).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let secret = parts[1];

    // Fetch the hash and account from DB
    let record = sqlx::query!(
        r#"
        SELECT k.account_id, k.key_hash 
        FROM api_keys k
        WHERE k.id = $1
        "#,
        key_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let record = match record {
        Some(r) => r,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    // Verify argon2 hash
    let parsed_hash = PasswordHash::new(&record.key_hash).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if Argon2::default().verify_password(secret.as_bytes(), &parsed_hash).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Key is valid, update last_used_at in background
    let db = state.db.clone();
    tokio::spawn(async move {
        let _ = sqlx::query!("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1", key_id)
            .execute(&db)
            .await;
    });

    let auth_acc = AuthenticatedAccount {
        account_id: record.account_id,
        api_key_id: key_id,
    };

    request.extensions_mut().insert(auth_acc);

    Ok(next.run(request).await)
}
