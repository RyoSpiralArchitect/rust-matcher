use axum::{
    extract::{Path, State},
    Json,
};
use tracing::info;

use sr_common::api::match_request::MatchRequest;
use sr_common::api::match_response::MatchResponse;
use sr_common::db::{fetch_candidates_for_project, fetch_match_by_id};

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::{CacheableJson, SharedState, SHORT_CACHE_CONTROL};

const DEFAULT_MATCH_LIMIT: usize = 50;

pub async fn run_match(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(request): Json<MatchRequest>,
) -> Result<Json<Vec<MatchResponse>>, ApiError> {
    let project_id = request
        .project
        .id
        .ok_or_else(|| ApiError::BadRequest("project.id is required".into()))?;

    let limit = request.limit.unwrap_or(DEFAULT_MATCH_LIMIT).clamp(1, 200) as u32;

    let talent_filter: Option<Vec<i64>> = request.talent_ids;
    info!(
        user = %auth.subject,
        project_id,
        limit,
        include_softko = request.include_softko,
        talent_filter_len = talent_filter.as_ref().map(Vec::len).unwrap_or(0),
        "running match request"
    );
    let match_config = state.match_config.read().await.clone();
    let responses = fetch_candidates_for_project(
        &state.pool,
        project_id,
        request.include_softko,
        limit,
        0,
        talent_filter.as_deref(),
        &match_config,
    )
    .await?;

    Ok(Json(responses))
}

pub async fn get_match(
    State(state): State<SharedState>,
    Path(match_id): Path<i64>,
    auth: AuthUser,
) -> Result<CacheableJson<MatchResponse>, ApiError> {
    info!(user = %auth.subject, match_id, "fetching match result");
    let match_config = state.match_config.read().await.clone();
    let response = fetch_match_by_id(&state.pool, match_id, &match_config)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("match {match_id} not found")))?;

    Ok(CacheableJson::new(response, SHORT_CACHE_CONTROL))
}
