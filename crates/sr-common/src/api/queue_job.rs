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

#[derive(Debug, Clone, Serialize)]
pub struct QueueJobDetailResponse {
    pub job: QueueJobSummary,
    pub partial_fields: Option<serde_json::Value>,
    pub last_error: Option<String>,
    pub llm_latency_ms: Option<i32>,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub entity: Option<JobEntity>,
    pub pairs: Option<Vec<PairDetail>>, // match_results + interactions/feedback folded per pair
    pub source_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobEntity {
    Talent(TalentSnapshot),
    Project(ProjectSnapshot),
    Both {
        talent: TalentSnapshot,
        project: ProjectSnapshot,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct TalentSnapshot {
    pub id: i64,
    pub message_id: String,
    pub talent_name: Option<String>,
    pub summary_text: Option<String>,
    pub desired_price_min: Option<i32>,
    pub available_date: Option<chrono::NaiveDate>,
    pub received_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSnapshot {
    pub project_code: i64,
    pub message_id: Option<String>,
    pub project_name: String,
    pub monthly_tanka_min: Option<i32>,
    pub monthly_tanka_max: Option<i32>,
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_text: Option<String>,
    pub requires_manual_review: bool,
    pub manual_review_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchResultRow {
    pub id: i64,
    pub talent_id: i64,
    pub project_id: i64,
    pub is_knockout: bool,
    pub ko_reasons: Vec<String>,
    pub needs_manual_review: bool,
    pub score_total: Option<f32>,
    pub score_breakdown: Option<serde_json::Value>,
    pub engine_version: Option<String>,
    pub rule_version: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InteractionLogRow {
    pub id: i64,
    pub match_result_id: Option<i32>,
    pub talent_id: i64,
    pub project_id: i64,
    pub match_run_id: Option<String>,
    pub engine_version: Option<String>,
    pub config_version: Option<String>,
    pub two_tower_score: Option<f64>,
    pub business_score: Option<f64>,
    pub outcome: Option<String>,
    pub feedback_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedbackEventRow {
    pub id: i64,
    pub interaction_id: Option<i64>,
    pub match_result_id: Option<i32>,
    pub match_run_id: Option<String>,
    pub engine_version: Option<String>,
    pub config_version: Option<String>,
    pub project_id: i64,
    pub talent_id: i64,
    pub feedback_type: String,
    pub ng_reason_category: Option<String>,
    pub comment: Option<String>,
    pub actor: String,
    pub source: String,
    pub is_revoked: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairDetail {
    pub match_result: MatchResultRow,
    pub latest_interaction: Option<InteractionLogRow>,
    pub feedback_events: Vec<FeedbackEventRow>,
}

#[derive(Debug, Clone, Default)]
pub struct JobDetailIncludes {
    pub include_entity: bool,
    pub include_matches: bool,
    pub include_interactions: bool,
    pub include_feedback: bool,
    pub include_source_text: bool,
    pub limit: i64,
    pub days: i64,
}
