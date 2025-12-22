use axum::{Json, extract::State};
use sr_common::api::queue_dashboard::QueueDashboard;
use sr_common::db::fetch_dashboard;

use crate::SharedState;
use crate::auth::AuthUser;
use crate::error::ApiError;

pub async fn dashboard(
    State(state): State<SharedState>,
    _auth: AuthUser,
) -> Result<Json<QueueDashboard>, ApiError> {
    let dashboard = fetch_dashboard(&state.pool).await?;
    Ok(Json(dashboard))
}
