use axum::async_trait;
use axum::extract::FromRef;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use clap::ValueEnum;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use crate::error::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum AuthMode {
    ApiKey,
    Jwt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum JwtAlgorithm {
    Hs256,
    Hs384,
    Hs512,
    Rs256,
    Rs384,
    Rs512,
    Es256,
    Es384,
    Eddsa,
}

impl JwtAlgorithm {
    fn algorithm(&self) -> Algorithm {
        match self {
            JwtAlgorithm::Hs256 => Algorithm::HS256,
            JwtAlgorithm::Hs384 => Algorithm::HS384,
            JwtAlgorithm::Hs512 => Algorithm::HS512,
            JwtAlgorithm::Rs256 => Algorithm::RS256,
            JwtAlgorithm::Rs384 => Algorithm::RS384,
            JwtAlgorithm::Rs512 => Algorithm::RS512,
            JwtAlgorithm::Es256 => Algorithm::ES256,
            JwtAlgorithm::Es384 => Algorithm::ES384,
            JwtAlgorithm::Eddsa => Algorithm::EdDSA,
        }
    }

    pub fn key_kind(&self) -> JwtKeyKind {
        match self {
            JwtAlgorithm::Hs256 | JwtAlgorithm::Hs384 | JwtAlgorithm::Hs512 => JwtKeyKind::Secret,
            JwtAlgorithm::Rs256 | JwtAlgorithm::Rs384 | JwtAlgorithm::Rs512 => JwtKeyKind::RsaPublicKey,
            JwtAlgorithm::Es256 | JwtAlgorithm::Es384 => JwtKeyKind::EcPublicKey,
            JwtAlgorithm::Eddsa => JwtKeyKind::EdPublicKey,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JwtKeyKind {
    Secret,
    RsaPublicKey,
    EcPublicKey,
    EdPublicKey,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub mode: AuthMode,
    pub api_key: Option<String>,
    pub jwt_secret: Option<String>,
    pub jwt_public_key: Option<String>,
    pub jwt_algorithm: JwtAlgorithm,
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
    let header = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing Authorization header".into()))?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("expected Bearer token".into()))?;

    let algorithm = config.jwt_algorithm.algorithm();
    let validation = Validation::new(algorithm);

    let decoding_key = match config.jwt_algorithm.key_kind() {
        JwtKeyKind::Secret => DecodingKey::from_secret(
            config
                .jwt_secret
                .as_deref()
                .ok_or_else(|| ApiError::Unauthorized("missing JWT_SECRET".into()))?
                .as_bytes(),
        ),
        JwtKeyKind::RsaPublicKey => DecodingKey::from_rsa_pem(
            config
                .jwt_public_key
                .as_deref()
                .ok_or_else(|| ApiError::Unauthorized("missing JWT_PUBLIC_KEY".into()))?
                .as_bytes(),
        )
        .map_err(|err| ApiError::Unauthorized(format!("invalid RSA public key: {err}")))?,
        JwtKeyKind::EcPublicKey => DecodingKey::from_ec_pem(
            config
                .jwt_public_key
                .as_deref()
                .ok_or_else(|| ApiError::Unauthorized("missing JWT_PUBLIC_KEY".into()))?
                .as_bytes(),
        )
        .map_err(|err| ApiError::Unauthorized(format!("invalid EC public key: {err}")))?,
        JwtKeyKind::EdPublicKey => DecodingKey::from_ed_pem(
            config
                .jwt_public_key
                .as_deref()
                .ok_or_else(|| ApiError::Unauthorized("missing JWT_PUBLIC_KEY".into()))?
                .as_bytes(),
        )
        .map_err(|err| ApiError::Unauthorized(format!("invalid EdDSA public key: {err}")))?,
    };

    let data = decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|err| ApiError::Unauthorized(format!("invalid token: {err}")))?;

    Ok(AuthUser {
        subject: data.claims.sub,
    })
}
