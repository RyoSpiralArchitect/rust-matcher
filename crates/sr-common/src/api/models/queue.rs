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

/// キューダッシュボード集計
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueueDashboard {
    /// ステータス別件数
    pub status_counts: StatusCounts,
    /// 手動レビュー待ち件数
    pub manual_review_count: i64,
    /// エラー件数（last_error IS NOT NULL）
    pub error_count: i64,
    /// 処理中（10分以上）の滞留件数
    pub stale_processing_count: i64,
    /// 最終更新時刻
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusCounts {
    pub pending: i64,
    pub processing: i64,
    pub completed: i64,
}

/// キューダッシュボード一覧用DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueJobDashboardItem {
    /// ジョブID（message_id）
    pub message_id: String,
    /// 件名
    pub subject: String,
    /// ステータス
    pub status: String,
    /// 推奨メソッド
    pub recommended_method: Option<String>,
    /// 最終メソッド
    pub final_method: Option<String>,
    /// ロック者
    pub locked_by: Option<String>,
    /// 処理開始時刻
    pub processing_started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 最終エラー
    pub last_error: Option<String>,
    /// 判定理由
    pub decision_reason: Option<String>,
    /// 手動レビュー必要
    pub requires_manual_review: bool,
    /// リトライ回数
    pub retry_count: i32,
    /// 受信日時
    pub received_at: chrono::DateTime<chrono::Utc>,
    /// 更新日時
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// キューダッシュボード詳細DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueJobDashboardDetail {
    /// 基本情報
    #[serde(flatten)]
    pub summary: QueueJobDashboardItem,
    /// 抽出フィールド（JSON）
    pub partial_fields: Option<serde_json::Value>,
    /// 優先度
    pub priority: i32,
    /// LLMレイテンシ（ms）
    pub llm_latency_ms: Option<i64>,
    /// 手動レビュー理由
    pub manual_review_reason: Option<String>,
    /// 作成日時
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 完了日時
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueJobListItem {
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
    pub items: Vec<QueueJobListItem>,
    pub limit: i64,
    pub offset: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueJobDetail {
    pub job: QueueJobListItem,
    pub partial_fields: Option<serde_json::Value>,
    pub last_error: Option<String>,
    pub llm_latency_ms: Option<i32>,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueJobDetailResponse {
    pub job: QueueJobListItem,
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
    pub match_result_id: Option<i64>,
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
    pub match_result_id: Option<i64>,
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
    pub days: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_dashboard_defaults_zero_counts() {
        let dashboard = QueueDashboard::default();
        assert_eq!(dashboard.status_counts.pending, 0);
        assert_eq!(dashboard.status_counts.processing, 0);
        assert_eq!(dashboard.status_counts.completed, 0);
        assert_eq!(dashboard.manual_review_count, 0);
        assert_eq!(dashboard.error_count, 0);
        assert_eq!(dashboard.stale_processing_count, 0);
    }

    #[test]
    fn flatten_queue_job_dashboard_detail_summary() {
        let summary = QueueJobDashboardItem {
            message_id: "msg-1".into(),
            subject: "案件A".into(),
            status: "pending".into(),
            recommended_method: Some("rust_recommended".into()),
            final_method: None,
            locked_by: Some("worker-1".into()),
            processing_started_at: None,
            last_error: None,
            decision_reason: Some("score_high".into()),
            requires_manual_review: false,
            retry_count: 0,
            received_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let detail = QueueJobDashboardDetail {
            summary,
            partial_fields: Some(serde_json::json!({"monthly_tanka_min": 80})),
            priority: 50,
            llm_latency_ms: Some(1200),
            manual_review_reason: None,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };

        let json = serde_json::to_value(&detail).expect("serialization should succeed");
        assert!(json.get("message_id").is_some());
        assert!(json.get("partial_fields").is_some());
    }
}
