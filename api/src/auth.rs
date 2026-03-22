use axum::{
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};

pub async fn auth_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get("X-Api-Key");

    if let Some(api_key) = auth_header {
        if let Ok(key_str) = api_key.to_str() {
            // TODO: Here we will hash the key and check against the DB `api_keys.key_hash`
            // For the MVP testing phase, we just check if it's not empty
            if !key_str.is_empty() {
                // You can inject the identified AccountId into the request extensions here
                return Ok(next.run(req).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}
