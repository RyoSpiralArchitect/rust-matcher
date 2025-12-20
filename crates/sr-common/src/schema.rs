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
}
