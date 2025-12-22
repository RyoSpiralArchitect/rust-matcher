use axum::async_trait;
use axum::extract::FromRef;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use clap::ValueEnum;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;

use crate::error::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum AuthMode {
    ApiKey,
    Jwt,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub mode: AuthMode,
    pub api_key: Option<String>,
    pub jwt_secret: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    #[allow(dead_code)]
    pub subject: String,
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    #[allow(dead_code)]
    exp: Option<usize>,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    AuthConfig: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let config = AuthConfig::from_ref(state);

        match config.mode {
            AuthMode::ApiKey => authorize_api_key(parts, &config),
            AuthMode::Jwt => authorize_jwt(parts, &config),
        }
    }
}

fn authorize_api_key(parts: &Parts, config: &AuthConfig) -> Result<AuthUser, ApiError> {
    let expected = config
        .api_key
        .as_deref()
        .ok_or_else(|| ApiError::Unauthorized("missing SR_API_KEY".into()))?;

    let provided = parts
        .headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing X-API-Key header".into()))?;

    if provided != expected {
        return Err(ApiError::Unauthorized("invalid API key".into()));
    }

    Ok(AuthUser {
        subject: "api_key".to_string(),
    })
}

fn authorize_jwt(parts: &Parts, config: &AuthConfig) -> Result<AuthUser, ApiError> {
    let secret = config
        .jwt_secret
        .as_deref()
        .ok_or_else(|| ApiError::Unauthorized("missing JWT_SECRET".into()))?;

    let header = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing Authorization header".into()))?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("expected Bearer token".into()))?;

    let validation = Validation::new(Algorithm::HS256);

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|err| ApiError::Unauthorized(format!("invalid token: {err}")))?;

    Ok(AuthUser {
        subject: data.claims.sub,
    })
}
