use axum::{Json, extract::State};
use serde_json::json;
use tokio::time::{Duration, timeout};

use crate::SharedState;
use crate::error::ApiError;

const READINESS_TIMEOUT: Duration = Duration::from_secs(1);

pub async fn livez() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz(State(state): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    if !state.readiness.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(ApiError::ServiceUnavailable("shutting_down".into()));
    }

    let client = timeout(READINESS_TIMEOUT, state.pool.get())
        .await
        .map_err(|_| ApiError::ServiceUnavailable("db_pool_timeout".into()))
        .and_then(|result| {
            result.map_err(|err| {
                ApiError::ServiceUnavailable(format!("failed to check out pool connection: {err}"))
            })
        })?;

    timeout(READINESS_TIMEOUT, client.simple_query("SELECT 1"))
        .await
        .map_err(|_| ApiError::ServiceUnavailable("db_ping_timeout".into()))
        .and_then(|result| {
            result
                .map_err(|err| ApiError::ServiceUnavailable(format!("health check failed: {err}")))
        })?;

    Ok(Json(json!({
        "status": "ok",
        "database": "ok",
        "application": env!("CARGO_PKG_NAME"),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicBool};

    use sr_common::db::create_pool_from_url;

    use crate::auth::{AuthConfig, AuthMode, JwtAlgorithm};
    use crate::{default_rate_limits, AppConfig, AppState, MatchConfig};

    fn state_with_readiness(readiness: bool) -> SharedState {
        let pool = create_pool_from_url("postgres://user:pass@localhost:5432/example").unwrap();
        let config = AppConfig {
            database_url: "postgres://user:pass@localhost:5432/example".into(),
            port: 3001,
            cors_origins: Vec::new(),
            auth: AuthConfig {
                mode: AuthMode::ApiKey,
                api_key: Some("test-key".into()),
                jwt_secret: None,
                jwt_public_key: None,
                jwt_algorithm: JwtAlgorithm::Hs256,
            },
            allow_source_text: false,
            job_detail_statement_timeout_ms: 5000,
        };

        Arc::new(AppState {
            pool,
            config,
            match_config: MatchConfig::default(),
            rate_limits: default_rate_limits(),
            readiness: Arc::new(AtomicBool::new(readiness)),
        })
    }

    #[tokio::test]
    async fn readyz_rejects_when_readiness_disabled() {
        let state = state_with_readiness(false);

        let result = readyz(State(state)).await;

        match result {
            Err(ApiError::ServiceUnavailable(code)) => {
                assert!(code.contains("shutting_down"));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
