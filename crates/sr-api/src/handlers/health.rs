use axum::{Json, extract::State};
use serde_json::json;

use crate::SharedState;
use crate::error::ApiError;

pub async fn health_check(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let client =
        state.pool.get().await.map_err(|err| {
            ApiError::Database(format!("failed to check out pool connection: {err}"))
        })?;

    client
        .simple_query("SELECT 1")
        .await
        .map_err(|err| ApiError::Database(format!("health check failed: {err}")))?;

    Ok(Json(json!({
        "status": "ok",
        "database": "ok",
        "application": env!("CARGO_PKG_NAME"),
    })))
}
