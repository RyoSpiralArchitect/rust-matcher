use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

fn default_limit() -> i64 {
    50
}

/// Pagination parameters for queue job listings.
#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: default_limit(),
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct QueueJobFilter {
    pub status: Option<String>,
    pub requires_manual_review: Option<bool>,
    pub canary_target: Option<bool>,
    pub final_method: Option<String>,
    pub manual_review_reason: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueJobSummary {
    pub id: i64,
    pub message_id: String,
    pub status: String,
    pub priority: i32,
    pub retry_count: i32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub final_method: Option<String>,
    pub requires_manual_review: bool,
    pub manual_review_reason: Option<String>,
    pub decision_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueJobListResponse {
    pub items: Vec<QueueJobSummary>,
    pub limit: i64,
    pub offset: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueJobDetail {
    pub job: QueueJobSummary,
    pub partial_fields: Option<serde_json::Value>,
    pub last_error: Option<String>,
    pub llm_latency_ms: Option<i32>,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
