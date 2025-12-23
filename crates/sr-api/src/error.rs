use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use std::borrow::Cow;
use thiserror::Error;

use sr_common::db::{
    CandidateFetchError, FeedbackStorageError, QueueDashboardError, QueueStorageError,
};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("database error: {0}")]
    Database(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal server error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    code: &'static str,
    message: String,
    request_id: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let body = Json(ErrorResponse {
            code: self.code(),
            message: self.public_message().into_owned(),
            request_id: None,
        });

        (status, body).into_response()
    }
}

impl ApiError {
    fn code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Unauthorized(_) => "unauthorized",
            ApiError::Forbidden(_) => "forbidden",
            ApiError::NotFound(_) => "not_found",
            ApiError::Conflict(_) => "conflict",
            ApiError::Database(_) => "database_error",
            ApiError::Internal(_) => "internal_error",
        }
    }

    fn public_message(&self) -> Cow<'static, str> {
        match self {
            ApiError::BadRequest(msg) => Cow::Owned(msg.clone()),
            ApiError::Unauthorized(_) => Cow::Borrowed("unauthorized"),
            ApiError::Forbidden(_) => Cow::Borrowed("forbidden"),
            ApiError::NotFound(msg) => Cow::Owned(msg.clone()),
            ApiError::Conflict(msg) => Cow::Owned(msg.clone()),
            ApiError::Database(_) | ApiError::Internal(_) => Cow::Borrowed("internal server error"),
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Database(_) | ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
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
            FeedbackStorageError::MissingActor => {
                ApiError::BadRequest("feedback actor is required".into())
            }
            other => ApiError::Database(other.to_string()),
        }
    }
}

impl From<QueueStorageError> for ApiError {
    fn from(value: QueueStorageError) -> Self {
        match value {
            QueueStorageError::NotFound(msg) => ApiError::NotFound(msg),
            QueueStorageError::Conflict(msg) => ApiError::Conflict(msg),
            other => ApiError::Database(other.to_string()),
        }
    }
}
