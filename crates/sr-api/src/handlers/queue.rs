use axum::{
    Json, debug_handler,
    extract::{Path, Query, State},
};
use sr_common::api::queue_dashboard::QueueDashboard;
use sr_common::api::queue_job::{
    JobDetailIncludes, Pagination, QueueJobDetailResponse, QueueJobFilter, QueueJobListResponse,
};
use sr_common::db::{
    fetch_dashboard, get_job_detail_with_includes, list_jobs as fetch_listed_jobs,
    retry_job as retry_queue_job,
};

use crate::SharedState;
use crate::auth::AuthUser;
use crate::error::ApiError;

#[derive(Debug, Default, serde::Deserialize)]
pub struct ListJobsParams {
    #[serde(flatten)]
    pub filter: QueueJobFilter,
    #[serde(flatten)]
    pub pagination: Pagination,
}

fn validate_filter(filter: &QueueJobFilter) -> Result<(), ApiError> {
    if let Some(status) = &filter.status {
        match status.as_str() {
            "pending" | "processing" | "completed" => {}
            other => {
                return Err(ApiError::BadRequest(format!(
                    "unsupported status filter: {other}"
                )));
            }
        }
    }

    if let Some(final_method) = &filter.final_method {
        match final_method.as_str() {
            "rust_completed" | "llm_completed" | "manual_review" => {}
            other => {
                return Err(ApiError::BadRequest(format!(
                    "unsupported final_method filter: {other}"
                )));
            }
        }
    }

    Ok(())
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct JobDetailParams {
    pub include: Option<String>,
    pub limit: Option<i64>,
    pub days: Option<i64>,
}

fn build_detail_includes(params: &JobDetailParams) -> Result<JobDetailIncludes, ApiError> {
    let mut includes = JobDetailIncludes {
        limit: params.limit.unwrap_or(20),
        days: params.days.unwrap_or(30),
        ..Default::default()
    };

    if includes.limit <= 0 || includes.limit > 200 {
        return Err(ApiError::BadRequest(
            "limit must be between 1 and 200".into(),
        ));
    }

    if includes.days <= 0 {
        return Err(ApiError::BadRequest("days must be positive".into()));
    }

    if let Some(include) = &params.include {
        for part in include.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }

            match trimmed {
                "record" => {}
                "entity" => includes.include_entity = true,
                "matches" => includes.include_matches = true,
                "interactions" => includes.include_interactions = true,
                "feedback" => includes.include_feedback = true,
                "source_text" => includes.include_source_text = true,
                other => {
                    return Err(ApiError::BadRequest(format!(
                        "unsupported include flag: {other}"
                    )));
                }
            }
        }
    }

    Ok(includes)
}

pub async fn dashboard(
    State(state): State<SharedState>,
    _auth: AuthUser,
) -> Result<Json<QueueDashboard>, ApiError> {
    let dashboard = fetch_dashboard(&state.pool).await?;
    Ok(Json(dashboard))
}

#[debug_handler]
pub async fn list_jobs(
    State(state): State<SharedState>,
    Query(params): Query<ListJobsParams>,
    _auth: AuthUser,
) -> Result<Json<QueueJobListResponse>, ApiError> {
    validate_filter(&params.filter)?;

    let pagination = params.pagination;
    if pagination.limit <= 0 || pagination.limit > 200 {
        return Err(ApiError::BadRequest(
            "limit must be between 1 and 200".into(),
        ));
    }
    if pagination.offset < 0 {
        return Err(ApiError::BadRequest("offset must be >= 0".into()));
    }

    let jobs = fetch_listed_jobs(&state.pool, &params.filter, &pagination).await?;
    Ok(Json(jobs))
}

pub async fn get_job(
    State(state): State<SharedState>,
    _auth: AuthUser,
    Path(id): Path<i64>,

    Query(params): Query<JobDetailParams>,
) -> Result<Json<QueueJobDetailResponse>, ApiError> {
    let includes = build_detail_includes(&params)?;

    let job =
        get_job_detail_with_includes(&state.pool, id, includes, state.config.allow_source_text)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("job {id} not found")))?;

    Ok(Json(job))
}

pub async fn retry_job(
    State(state): State<SharedState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    retry_queue_job(&state.pool, id).await?;
    Ok(Json(
        serde_json::json!({ "success": true, "status": "pending" }),
    ))
}

#[cfg(test)]
fn assert_query_bounds() {
    fn assert_send_sync<T: Send + Sync + serde::de::DeserializeOwned>() {}
    assert_send_sync::<ListJobsParams>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_filter_rejects_invalid_status() {
        let filter = QueueJobFilter {
            status: Some("unknown".into()),
            ..Default::default()
        };

        let err = validate_filter(&filter).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[test]
    fn validate_filter_rejects_invalid_final_method() {
        let filter = QueueJobFilter {
            final_method: Some("other".into()),
            ..Default::default()
        };

        let err = validate_filter(&filter).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[test]
    fn validate_filter_allows_supported_values() {
        let filter = QueueJobFilter {
            status: Some("pending".into()),
            final_method: Some("rust_completed".into()),
            ..Default::default()
        };

        assert!(validate_filter(&filter).is_ok());
    }
}
