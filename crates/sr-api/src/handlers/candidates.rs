use axum::extract::{Path, Query, State};
use serde::Deserialize;
use sr_common::api::match_response::MatchResponse;
use sr_common::db::fetch_candidates_for_project;
use tracing::info;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::handlers::pagination::validate_pagination;
use crate::{CacheableJson, SharedState, SHORT_CACHE_CONTROL};

#[derive(Debug, Deserialize, Default)]
pub struct CandidateQuery {
    #[serde(default)]
    pub include_softko: bool,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

const fn default_limit() -> u32 {
    50
}

#[derive(Debug, serde::Serialize)]
pub struct CandidateListResponse {
    pub project_id: i64,
    pub candidates: Vec<MatchResponse>,
}

pub async fn list_candidates(
    State(state): State<SharedState>,
    Path(project_id): Path<i64>,
    Query(query): Query<CandidateQuery>,
    auth: AuthUser,
) -> Result<CacheableJson<CandidateListResponse>, ApiError> {
    let (limit, offset) = validate_pagination(i64::from(query.limit), i64::from(query.offset))?;
    let limit = limit as u32;
    let offset = offset as u32;
    info!(
        user = %auth.subject,
        project_id,
        include_softko = query.include_softko,
        limit,
        offset,
        "listing candidates"
    );

    let match_config = state.match_config.read().await.clone();
    let candidates = fetch_candidates_for_project(
        &state.pool,
        project_id,
        query.include_softko,
        limit,
        offset,
        None,
        &match_config,
    )
    .await?;

    Ok(CacheableJson::new(
        CandidateListResponse {
            project_id,
            candidates,
        },
        SHORT_CACHE_CONTROL,
    ))
}
