/// DDL-1: ses.extraction_queue スキーマ定義
pub const EXTRACTION_QUEUE_DDL: &str = r#"
CREATE TABLE ses.extraction_queue (
    id SERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,
    email_subject TEXT NOT NULL,
    email_received_at TIMESTAMPTZ NOT NULL,
    subject_hash VARCHAR(16) NOT NULL,

    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 50,
    locked_by VARCHAR(100),

    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    last_error TEXT,

    partial_fields JSONB,
    decision_reason TEXT,

    recommended_method VARCHAR(20),
    final_method VARCHAR(20),

    extractor_version VARCHAR(20),
    rule_version VARCHAR(20),

    manual_review_reason TEXT,
    reprocess_after TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    processing_started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    llm_latency_ms INTEGER,

    requires_manual_review BOOLEAN NOT NULL DEFAULT false,
    canary_target BOOLEAN NOT NULL DEFAULT false,

    CONSTRAINT chk_status CHECK (status IN ('pending', 'processing', 'completed')),
    CONSTRAINT chk_recommended_method CHECK (recommended_method IN ('rust_recommended', 'llm_recommended')),
    CONSTRAINT chk_final_method CHECK (final_method IS NULL OR final_method IN ('rust_completed', 'llm_completed', 'manual_review')),
    CONSTRAINT chk_priority CHECK (priority >= 0 AND priority <= 100)
);

CREATE INDEX idx_extraction_queue_status_priority ON ses.extraction_queue(status, priority DESC, next_retry_at);
CREATE INDEX idx_extraction_queue_message_id ON ses.extraction_queue(message_id);
CREATE INDEX idx_extraction_queue_subject_hash ON ses.extraction_queue(subject_hash, created_at);
CREATE INDEX idx_extraction_queue_canary ON ses.extraction_queue(canary_target, created_at);
CREATE INDEX idx_extraction_queue_reprocess ON ses.extraction_queue(reprocess_after) WHERE reprocess_after IS NOT NULL;
CREATE INDEX idx_extraction_queue_review_reason ON ses.extraction_queue(manual_review_reason) WHERE manual_review_reason IS NOT NULL;
"#;

/// Proposed schema for daily match results snapshots.
pub const MATCH_RESULTS_DDL: &str = r#"
CREATE TABLE ses.match_results (
    id SERIAL PRIMARY KEY,
    talent_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,

    is_knockout BOOLEAN NOT NULL,
    ko_reasons JSONB,
    needs_manual_review BOOLEAN NOT NULL DEFAULT false,

    score_total FLOAT,
    score_breakdown JSONB,

    engine_version VARCHAR(20),
    rule_version VARCHAR(20),

    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(talent_id, project_id, created_at::date)
);

CREATE INDEX idx_match_results_talent ON ses.match_results(talent_id, created_at);
CREATE INDEX idx_match_results_project ON ses.match_results(project_id, created_at);
CREATE INDEX idx_match_results_score ON ses.match_results(score_total DESC) WHERE NOT is_knockout;
"#;

/// 保存場所: `ses.llm_comparison_results` (LLM shadow/AB比較ログ)
pub const LLM_COMPARISON_RESULTS_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS ses.llm_comparison_results (
    id BIGSERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL,
    primary_provider VARCHAR(50) NOT NULL,
    shadow_provider VARCHAR(50) NOT NULL,
    primary_response JSONB NOT NULL,
    shadow_response JSONB,
    primary_latency_ms INTEGER,
    shadow_latency_ms INTEGER,
    diff_summary JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_llm_comparison_message ON ses.llm_comparison_results(message_id);
CREATE INDEX idx_llm_comparison_created ON ses.llm_comparison_results(created_at);
CREATE INDEX idx_llm_comparison_providers ON ses.llm_comparison_results(primary_provider, shadow_provider);
"#;

/// Unified event log for GUI and sales feedback.
pub const FEEDBACK_EVENTS_DDL: &str = r#"
CREATE TABLE ses.feedback_events (
    id BIGSERIAL PRIMARY KEY,

    -- 紐付け（interaction_logs への FK を推奨）
    interaction_id BIGINT REFERENCES ses.interaction_logs(id),
    match_result_id INTEGER REFERENCES ses.match_results(id),
    match_run_id VARCHAR(64),
    engine_version VARCHAR(20),
    config_version VARCHAR(20),
    project_id BIGINT NOT NULL,
    talent_id BIGINT NOT NULL,

    -- フィードバック内容（統一ENUM: GUI評価 + 営業プロセス）
    feedback_type TEXT NOT NULL,
    -- 許容値:
    --   GUI評価: thumbs_up, thumbs_down, review_ok, review_ng, review_pending
    --   営業プロセス: accepted, rejected, interview_scheduled, no_response
    CONSTRAINT chk_feedback_type CHECK (feedback_type IN (
        'thumbs_up', 'thumbs_down', 'review_ok', 'review_ng', 'review_pending',
        'accepted', 'rejected', 'interview_scheduled', 'no_response'
    )),

    -- NG理由（review_ng / thumbs_down / rejected 時のみ）
    ng_reason_category TEXT,  -- tanka / skill / availability / location / flow / other
    CONSTRAINT chk_ng_reason_category CHECK (
        ng_reason_category IS NULL OR ng_reason_category IN (
            'tanka', 'skill', 'availability', 'location', 'flow', 'other'
        )
    ),

    -- 自由記述・タグ
    comment TEXT,
    feedback_tags JSONB,  -- ["単価NG", "スキル不足"] 等の自由配列

    -- 取り消しフラグ（間違い訂正用）
    is_revoked BOOLEAN NOT NULL DEFAULT false,
    revoked_at TIMESTAMPTZ,

    -- 誰が・どこから
    actor TEXT NOT NULL,   -- user_id / "sales" / "ops" / "system"
    source TEXT NOT NULL,  -- "gui" / "crm" / "api" / "import"

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- インデックス
CREATE INDEX idx_feedback_interaction ON ses.feedback_events(interaction_id);
CREATE INDEX idx_feedback_match ON ses.feedback_events(match_result_id);
CREATE INDEX idx_feedback_match_run ON ses.feedback_events(match_run_id);
CREATE INDEX idx_feedback_project_talent ON ses.feedback_events(project_id, talent_id);
CREATE INDEX idx_feedback_type ON ses.feedback_events(feedback_type, created_at DESC);
CREATE INDEX idx_feedback_actor ON ses.feedback_events(actor, created_at DESC);
CREATE INDEX idx_feedback_not_revoked ON ses.feedback_events(interaction_id, created_at DESC)
    WHERE is_revoked = false;

COMMENT ON TABLE ses.feedback_events IS '営業/GUIフィードバックの統一イベントログ（Two-Tower学習の正解ラベル源）';
"#;

/// Interaction logging for recommendations and downstream training views.
pub const INTERACTION_LOGS_DDL: &str = r#"
CREATE TABLE ses.interaction_logs (
    id BIGSERIAL PRIMARY KEY,

    -- マッチング情報
    match_result_id INTEGER REFERENCES ses.match_results(id),
    talent_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    match_run_id VARCHAR(64),       -- engine_version + config_version を含む実行単位
    engine_version VARCHAR(20),
    config_version VARCHAR(20),

    -- Two-Tower 予測
    two_tower_score FLOAT,          -- 予測スコア
    two_tower_embedder VARCHAR(50), -- hash / onnx / candle
    two_tower_version VARCHAR(20),  -- モデルバージョン

    -- ビジネスルールスコア（比較用）
    business_score FLOAT,

    -- 結果（後から更新）
    outcome VARCHAR(20),  -- accepted / rejected / no_response / NULL
    feedback_at TIMESTAMPTZ,

    -- メタデータ
    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- インデックス
    CONSTRAINT interaction_logs_unique UNIQUE (talent_id, project_id, created_at::date)
);

CREATE INDEX idx_interaction_logs_match_run ON ses.interaction_logs(match_run_id, created_at DESC);
CREATE INDEX idx_interaction_logs_outcome ON ses.interaction_logs(outcome, created_at DESC)
    WHERE outcome IS NOT NULL;

CREATE OR REPLACE VIEW ses.training_pairs AS
SELECT
    il.talent_id,
    il.project_id,
    il.two_tower_score,
    il.business_score,
    il.outcome,
    CASE
        WHEN il.outcome = 'accepted' THEN 1.0
        WHEN il.outcome = 'rejected' THEN 0.0
        ELSE NULL  -- no_response は除外
    END AS label,
    il.created_at
FROM ses.interaction_logs il
WHERE il.outcome IN ('accepted', 'rejected');

CREATE OR REPLACE VIEW ses.training_stats AS
SELECT
    COUNT(*) FILTER (WHERE outcome = 'accepted') AS accepted_count,
    COUNT(*) FILTER (WHERE outcome = 'rejected') AS rejected_count,
    COUNT(*) FILTER (WHERE outcome IS NULL) AS pending_count,
    MIN(created_at) AS first_log_at,
    MAX(created_at) AS last_log_at,
    COUNT(DISTINCT DATE_TRUNC('day', created_at)) AS active_days
FROM ses.interaction_logs;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddl_contains_required_columns() {
        for required in [
            "message_id",
            "email_subject",
            "status",
            "retry_count",
            "final_method",
            "llm_latency_ms",
            "manual_review_reason",
            "reprocess_after",
            "idx_extraction_queue_status_priority",
        ] {
            assert!(EXTRACTION_QUEUE_DDL.contains(required));
        }
    }

    #[test]
    fn match_results_schema_contains_indexes_and_uniques() {
        for required in [
            "talent_id",
            "project_id",
            "score_breakdown",
            "UNIQUE(talent_id, project_id, created_at::date)",
            "idx_match_results_score",
        ] {
            assert!(MATCH_RESULTS_DDL.contains(required));
        }
    }

    #[test]
    fn feedback_events_schema_includes_indexes_and_flags() {
        for required in [
            "interaction_id",
            "feedback_type",
            "match_run_id",
            "engine_version",
            "config_version",
            "is_revoked",
            "idx_feedback_project_talent",
            "idx_feedback_match_run",
            "idx_feedback_not_revoked",
            "chk_feedback_type",
            "chk_ng_reason_category",
            "COMMENT ON TABLE ses.feedback_events",
        ] {
            assert!(FEEDBACK_EVENTS_DDL.contains(required));
        }
    }

    #[test]
    fn interaction_logs_schema_covers_views_and_unique_constraint() {
        for required in [
            "two_tower_score",
            "business_score",
            "match_run_id",
            "engine_version",
            "config_version",
            "interaction_logs_unique",
            "idx_interaction_logs_match_run",
            "idx_interaction_logs_outcome",
            "CREATE OR REPLACE VIEW ses.training_pairs",
            "CREATE OR REPLACE VIEW ses.training_stats",
        ] {
            assert!(INTERACTION_LOGS_DDL.contains(required));
        }
    }

    #[test]
    fn llm_comparison_schema_includes_indexes_and_diff_summary() {
        for required in [
            "primary_provider",
            "shadow_provider",
            "diff_summary",
            "idx_llm_comparison_message",
            "idx_llm_comparison_providers",
        ] {
            assert!(LLM_COMPARISON_RESULTS_DDL.contains(required));
        }
    }
}
