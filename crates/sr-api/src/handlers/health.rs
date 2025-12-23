use axum::{extract::State, Json};
use serde_json::json;
use tokio::time::{timeout, Duration};

use crate::error::ApiError;
use crate::SharedState;

const READINESS_TIMEOUT: Duration = Duration::from_secs(1);

pub async fn livez() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz(State(state): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let client = timeout(READINESS_TIMEOUT, state.pool.get())
        .await
        .map_err(|_| ApiError::ServiceUnavailable("db_pool_timeout".into()))
        .and_then(|result| {
            result.map_err(|err| ApiError::ServiceUnavailable(format!(
                "failed to check out pool connection: {err}"
            )))
        })?;

    timeout(READINESS_TIMEOUT, client.simple_query("SELECT 1"))
        .await
        .map_err(|_| ApiError::ServiceUnavailable("db_ping_timeout".into()))
        .and_then(|result| {
            result.map_err(|err| {
                ApiError::ServiceUnavailable(format!("health check failed: {err}"))
            })
        })?;

    Ok(Json(json!({
        "status": "ok",
        "database": "ok",
        "application": env!("CARGO_PKG_NAME"),
    })))
}
