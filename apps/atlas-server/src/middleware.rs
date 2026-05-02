use crate::constants::env;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

pub async fn mcp_auth(request: Request, next: Next) -> Result<Response, StatusCode> {
    bearer_auth(env::ATLAS_MCP_TOKEN, "[MCP]", request, next).await
}

pub async fn api_auth(request: Request, next: Next) -> Result<Response, StatusCode> {
    bearer_auth(env::ATLAS_API_TOKEN, "[API]", request, next).await
}

async fn bearer_auth(
    token_env: &str,
    log_prefix: &str,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Ok(expected) = std::env::var(token_env) else {
        return Ok(next.run(request).await);
    };
    if expected.is_empty() {
        return Ok(next.run(request).await);
    }

    let provided = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .or_else(|| {
            request
                .headers()
                .get("x-atlas-token")
                .and_then(|h| h.to_str().ok())
        })
        .unwrap_or("");

    if ct_eq(provided, &expected) {
        Ok(next.run(request).await)
    } else {
        tracing::warn!("{log_prefix} Unauthorized request rejected");
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn ct_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}
