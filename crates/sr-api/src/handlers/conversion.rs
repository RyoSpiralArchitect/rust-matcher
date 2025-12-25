use axum::{extract::State, Json};
use sr_common::api::conversion::{ConversionRequest, ConversionResponse};
use sr_common::db::insert_conversion_event;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::SharedState;

pub async fn submit_conversion(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(payload): Json<ConversionRequest>,
) -> Result<Json<ConversionResponse>, ApiError> {
    let response = insert_conversion_event(&state.pool, &auth.subject, &payload).await?;
    Ok(Json(response))
}
