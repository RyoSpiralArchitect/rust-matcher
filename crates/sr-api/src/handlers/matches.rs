use axum::{
    extract::{Path, State},
    Json,
};

use sr_common::api::match_request::MatchRequest;
use sr_common::api::match_response::MatchResponse;
use sr_common::db::{fetch_candidates_for_project, fetch_match_by_id};

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::SharedState;

const DEFAULT_MATCH_LIMIT: usize = 50;

pub async fn run_match(
    State(state): State<SharedState>,
    _auth: AuthUser,
    Json(request): Json<MatchRequest>,
) -> Result<Json<Vec<MatchResponse>>, ApiError> {
    let project_id = request
        .project
        .id
        .ok_or_else(|| ApiError::BadRequest("project.id is required".into()))?;

    let limit = request.limit.unwrap_or(DEFAULT_MATCH_LIMIT).clamp(1, 200) as u32;

    let talent_filter: Option<Vec<i64>> = request.talent_ids;
    let responses = fetch_candidates_for_project(
        &state.pool,
        project_id,
        request.include_softko,
        limit,
        0,
        talent_filter.as_deref(),
        &state.match_config,
    )
    .await?;

    Ok(Json(responses))
}

pub async fn get_match(
    State(state): State<SharedState>,
    Path(match_id): Path<i64>,
    _auth: AuthUser,
) -> Result<Json<MatchResponse>, ApiError> {
    let response = fetch_match_by_id(&state.pool, match_id, &state.match_config)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("match {match_id} not found")))?;

    Ok(Json(response))
}
