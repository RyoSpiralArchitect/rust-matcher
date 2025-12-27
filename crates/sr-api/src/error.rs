use axum::{
    http::{header::RETRY_AFTER, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::{borrow::Cow, future::Future, time::Duration};
use thiserror::Error;
use tracing::error;

use sr_common::db::{
    ConversionStorageError, FeedbackHistoryError, FeedbackStorageError,
    InteractionEventStorageError, MatchFetchError, QueueDashboardError, QueueStorageError,
};

tokio::task_local! {
    static REQUEST_ID: String;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitMeta {
    pub limit: u32,
    pub remaining: u32,
    pub reset_after: Duration,
    pub retry_after: Option<Duration>,
}

impl RateLimitMeta {
    fn ceil_seconds(duration: Duration) -> u64 {
        let nanos = duration.as_nanos();
        let mut secs = nanos / 1_000_000_000;
        if nanos % 1_000_000_000 != 0 {
            secs += 1;
        }
        secs as u64
    }

    fn add_header(headers: &mut HeaderMap, name: &'static str, value: String) {
        if let Ok(parsed) = HeaderValue::from_str(&value) {
            headers.insert(HeaderName::from_static(name), parsed);
        }
    }

    pub fn apply_headers(&self, headers: &mut HeaderMap) {
        Self::add_header(headers, "ratelimit-limit", self.limit.to_string());
        Self::add_header(headers, "ratelimit-remaining", self.remaining.to_string());
        Self::add_header(
            headers,
            "ratelimit-reset",
            Self::ceil_seconds(self.reset_after).to_string(),
        );

        if let Some(retry_after) = self.retry_after {
            if let Ok(parsed) = HeaderValue::from_str(&Self::ceil_seconds(retry_after).to_string())
            {
                headers.insert(RETRY_AFTER, parsed);
            }
        }
    }
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
    #[error("database error")]
    Database { internal: Option<String> },
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
    #[error("too many requests: {message}")]
    TooManyRequests {
        message: String,
        rate_limit: Option<RateLimitMeta>,
    },
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

        match self.sanitized_internal_message() {
            Some(details) => {
                error!(
                    code,
                    status = %status,
                    request_id = request_id.as_deref().unwrap_or(""),
                    error = %self,
                    details,
                    "api_error"
                );
            }
            None => {
                error!(
                    code,
                    status = %status,
                    request_id = request_id.as_deref().unwrap_or(""),
                    error = %self,
                    "api_error"
                );
            }
        }

        let body = Json(ErrorResponse {
            code,
            message: self.public_message().into_owned(),
            request_id,
        });

        let mut response = (status, body).into_response();

        if let ApiError::TooManyRequests {
            rate_limit: Some(rate_limit),
            ..
        } = &self
        {
            rate_limit.apply_headers(response.headers_mut());
        }

        response
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
            ApiError::TooManyRequests { .. } => "too_many_requests",
            ApiError::ServiceUnavailable(_) => "service_unavailable",
            ApiError::Database { .. } => "database_error",
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
            ApiError::TooManyRequests { .. } => Cow::Borrowed("too many requests"),
            ApiError::ServiceUnavailable(_) => Cow::Borrowed("service unavailable"),
            ApiError::Database { .. } | ApiError::Internal(_) => {
                Cow::Borrowed("internal server error")
            }
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Database { .. } | ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn database_error(err: impl ToString) -> Self {
        ApiError::Database {
            internal: Some(err.to_string()),
        }
    }

    fn sanitized_internal_message(&self) -> Option<String> {
        match self {
            ApiError::Database { internal } => internal.as_deref().map(sanitize_message),
            ApiError::Internal(msg) => Some(sanitize_message(msg)),
            ApiError::TooManyRequests { message, .. } => Some(sanitize_message(message)),
            _ => None,
        }
    }
}

impl From<QueueDashboardError> for ApiError {
    fn from(value: QueueDashboardError) -> Self {
        ApiError::database_error(value)
    }
}

impl From<MatchFetchError> for ApiError {
    fn from(value: MatchFetchError) -> Self {
        ApiError::database_error(value)
    }
}

impl From<FeedbackHistoryError> for ApiError {
    fn from(value: FeedbackHistoryError) -> Self {
        ApiError::database_error(value)
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
            other => ApiError::database_error(other),
        }
    }
}

impl From<ConversionStorageError> for ApiError {
    fn from(value: ConversionStorageError) -> Self {
        match value {
            ConversionStorageError::MissingActor => {
                ApiError::BadRequest("conversion actor is required".into())
            }
            other => ApiError::database_error(other),
        }
    }
}

impl From<InteractionEventStorageError> for ApiError {
    fn from(value: InteractionEventStorageError) -> Self {
        match value {
            InteractionEventStorageError::MissingActor => {
                ApiError::BadRequest("interaction event actor is required".into())
            }
            other => ApiError::database_error(other),
        }
    }
}

impl From<QueueStorageError> for ApiError {
    fn from(value: QueueStorageError) -> Self {
        match value {
            QueueStorageError::NotFound(msg) => ApiError::NotFound(msg),
            QueueStorageError::Conflict(msg) => ApiError::Conflict(msg),
            other => ApiError::database_error(other),
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

    #[tokio::test]
    async fn database_errors_mask_public_message() {
        let err = ApiError::database_error("relation secret_table does not exist");
        let response = err.into_response();

        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = body.collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["message"], "internal server error");
    }

    #[test]
    fn rate_limit_headers_are_attached_when_present() {
        let meta = RateLimitMeta {
            limit: 10,
            remaining: 5,
            reset_after: Duration::from_millis(1200),
            retry_after: Some(Duration::from_millis(800)),
        };

        let err = ApiError::TooManyRequests {
            message: "rate limit exceeded".into(),
            rate_limit: Some(meta),
        };

        let response = err.into_response();
        let headers = response.headers();

        assert_eq!(
            headers.get("ratelimit-limit").and_then(|h| h.to_str().ok()),
            Some("10")
        );
        assert_eq!(
            headers
                .get("ratelimit-remaining")
                .and_then(|h| h.to_str().ok()),
            Some("5")
        );
        // ceil(1.2s) = 2
        assert_eq!(
            headers.get("ratelimit-reset").and_then(|h| h.to_str().ok()),
            Some("2")
        );
        assert_eq!(
            headers.get(RETRY_AFTER).and_then(|h| h.to_str().ok()),
            Some("1")
        );
    }
}
