use serde::{Deserialize, Serialize};

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

/// キュージョブ一覧用DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueJobSummary {
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

/// キュージョブ詳細DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueJobDetail {
    /// 基本情報
    #[serde(flatten)]
    pub summary: QueueJobSummary,
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
    fn flatten_queue_job_detail_summary() {
        let summary = QueueJobSummary {
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

        let detail = QueueJobDetail {
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
