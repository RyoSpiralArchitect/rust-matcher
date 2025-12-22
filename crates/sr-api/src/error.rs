use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;

use sr_common::db::{CandidateFetchError, FeedbackStorageError, QueueDashboardError};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("database error: {0}")]
    Database(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("internal server error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Database(_) | ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(ErrorResponse {
            error: self.to_string(),
        });

        (status, body).into_response()
    }
}

impl From<QueueDashboardError> for ApiError {
    fn from(value: QueueDashboardError) -> Self {
        ApiError::Database(value.to_string())
    }
}

impl From<CandidateFetchError> for ApiError {
    fn from(value: CandidateFetchError) -> Self {
        ApiError::Database(value.to_string())
    }
}

impl From<FeedbackStorageError> for ApiError {
    fn from(value: FeedbackStorageError) -> Self {
        match value {
            FeedbackStorageError::InteractionNotFound(id) => {
                ApiError::NotFound(format!("interaction not found: {id}"))
            }
            other => ApiError::Database(other.to_string()),
        }
    }
}
