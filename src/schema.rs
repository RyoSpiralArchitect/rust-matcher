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

    created_at TIMESTAMPTZ DEFAULT NOW(),
    processing_started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    llm_latency_ms INTEGER,

    requires_manual_review BOOLEAN NOT NULL DEFAULT false,
    canary_target BOOLEAN NOT NULL DEFAULT false,

    CONSTRAINT chk_status CHECK (status IN ('pending', 'processing', 'completed')),
    CONSTRAINT chk_recommended_method CHECK (recommended_method IN ('rust_recommended', 'llm_recommended')),
    CONSTRAINT chk_final_method CHECK (final_method IS NULL OR final_method IN ('rust_completed', 'llm_completed', 'manual_review'))
);
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
        ] {
            assert!(EXTRACTION_QUEUE_DDL.contains(required));
        }
    }
}
