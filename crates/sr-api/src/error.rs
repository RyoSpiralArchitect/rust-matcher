use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use std::{borrow::Cow, future::Future};
use thiserror::Error;
use tracing::error;

use sr_common::db::{
    ConversionStorageError, FeedbackHistoryError, FeedbackStorageError,
    InteractionEventStorageError, MatchFetchError, QueueDashboardError, QueueStorageError,
};

tokio::task_local! {
    static REQUEST_ID: String;
}

fn sanitize_message(message: &str) -> String {
    const MAX_LEN: usize = 240;

    let mut cleaned = message
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .replace(['\n', '\r'], " ");

    cleaned = cleaned
        .split_whitespace()
        .map(|token| {
            if token.contains("://") {
                "[redacted-url]".to_string()
            } else if let Some((base, _)) = token.split_once('?') {
                if base.is_empty() {
                    "[redacted-query]".to_string()
                } else {
                    format!("{base}?[redacted]")
                }
            } else if token.starts_with('/') || token.contains('\\') {
                "[redacted-path]".to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.len() > MAX_LEN {
        cleaned.truncate(MAX_LEN);
        cleaned.push('…');
    }

    if cleaned.trim().is_empty() {
        "unexpected error".to_string()
    } else {
        cleaned
    }
}

pub async fn with_request_id<Fut, T>(request_id: Option<String>, fut: Fut) -> T
where
    Fut: Future<Output = T>,
{
    if let Some(request_id) = request_id {
        REQUEST_ID.scope(request_id, fut).await
    } else {
        fut.await
    }
}

pub fn current_request_id() -> Option<String> {
    REQUEST_ID.try_with(|value| value.clone()).ok()
}

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
    #[error("too many requests: {0}")]
    TooManyRequests(String),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
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
        let code = self.code();
        let request_id = current_request_id();

        error!(
            code,
            status = %status,
            request_id = request_id.as_deref().unwrap_or(""),
            error = %self,
            "api_error"
        );

        let body = Json(ErrorResponse {
            code,
            message: self.public_message().into_owned(),
            request_id,
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
            ApiError::TooManyRequests(_) => "too_many_requests",
            ApiError::ServiceUnavailable(_) => "service_unavailable",
            ApiError::Database(_) => "database_error",
            ApiError::Internal(_) => "internal_error",
        }
    }

    fn public_message(&self) -> Cow<'static, str> {
        match self {
            ApiError::BadRequest(msg) => Cow::Owned(sanitize_message(msg)),
            ApiError::Unauthorized(_) => Cow::Borrowed("unauthorized"),
            ApiError::Forbidden(_) => Cow::Borrowed("forbidden"),
            ApiError::NotFound(msg) => Cow::Owned(sanitize_message(msg)),
            ApiError::Conflict(msg) => Cow::Owned(sanitize_message(msg)),
            ApiError::TooManyRequests(_) => Cow::Borrowed("too many requests"),
            ApiError::ServiceUnavailable(_) => Cow::Borrowed("service unavailable"),
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
            ApiError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Database(_) | ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<QueueDashboardError> for ApiError {
    fn from(value: QueueDashboardError) -> Self {
        ApiError::Database(value.to_string())
    }
}

impl From<MatchFetchError> for ApiError {
    fn from(value: MatchFetchError) -> Self {
        ApiError::Database(value.to_string())
    }
}

impl From<FeedbackHistoryError> for ApiError {
    fn from(value: FeedbackHistoryError) -> Self {
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

impl From<ConversionStorageError> for ApiError {
    fn from(value: ConversionStorageError) -> Self {
        match value {
            ConversionStorageError::MissingActor => {
                ApiError::BadRequest("conversion actor is required".into())
            }
            other => ApiError::Database(other.to_string()),
        }
    }
}

impl From<InteractionEventStorageError> for ApiError {
    fn from(value: InteractionEventStorageError) -> Self {
        match value {
            InteractionEventStorageError::MissingActor => {
                ApiError::BadRequest("interaction event actor is required".into())
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

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt;
    use serde_json::Value;

    use super::*;

    #[tokio::test]
    async fn includes_request_id_in_response_body_when_present() {
        let err = ApiError::Internal("boom".into());
        let response = with_request_id(Some("req-123".into()), async { err.into_response() }).await;

        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = body.collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["request_id"], "req-123");
    }
}
