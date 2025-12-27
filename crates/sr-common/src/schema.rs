use once_cell::sync::Lazy;

use crate::timezone::RUN_DATE_TIMEZONE;

/// Shared expression for deriving run_date/event_date columns.
pub static RUN_DATE_EXPRESSION: Lazy<String> =
    Lazy::new(|| format!("(created_at AT TIME ZONE '{}')::date", RUN_DATE_TIMEZONE));

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

    created_at TIMESTAMPTZ DEFAULT clock_timestamp(),
    processing_started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ DEFAULT clock_timestamp(),

    llm_latency_ms INTEGER,

    requires_manual_review BOOLEAN NOT NULL DEFAULT false,
    canary_target BOOLEAN NOT NULL DEFAULT false,

    CONSTRAINT chk_status CHECK (status IN ('pending', 'processing', 'completed')),
    CONSTRAINT chk_recommended_method CHECK (recommended_method IN ('rust_recommended', 'llm_recommended')),
    CONSTRAINT chk_final_method CHECK (final_method IS NULL OR final_method IN ('rust_completed', 'llm_completed', 'manual_review')),
    CONSTRAINT chk_priority CHECK (priority >= 0 AND priority <= 100),
    CONSTRAINT chk_retry_count CHECK (retry_count >= 0 AND retry_count <= 100)
);

CREATE INDEX idx_extraction_queue_status_priority ON ses.extraction_queue(status, priority DESC, next_retry_at);
CREATE INDEX idx_extraction_queue_pending ON ses.extraction_queue(created_at, id) WHERE status = 'pending';
CREATE INDEX idx_extraction_queue_status_created ON ses.extraction_queue(status, created_at, id);
CREATE INDEX idx_extraction_queue_message_id ON ses.extraction_queue(message_id);
CREATE INDEX idx_extraction_queue_subject_hash ON ses.extraction_queue(subject_hash, created_at);
CREATE INDEX idx_extraction_queue_canary ON ses.extraction_queue(canary_target, created_at);
CREATE INDEX idx_extraction_queue_reprocess ON ses.extraction_queue(reprocess_after) WHERE reprocess_after IS NOT NULL;
CREATE INDEX idx_extraction_queue_review_reason ON ses.extraction_queue(manual_review_reason) WHERE manual_review_reason IS NOT NULL;
CREATE INDEX idx_extraction_queue_partial_fields_json ON ses.extraction_queue USING GIN(partial_fields jsonb_path_ops);
"#;

/// Gmail案件メールの生データ（唯一の真実）
pub const ANKEN_EMAILS_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS ses.anken_emails (
    id BIGSERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,
    sender_address TEXT,
    sender_name TEXT,
    subject TEXT NOT NULL,
    body_text TEXT NOT NULL,
    received_at TIMESTAMPTZ NOT NULL,
    thread_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idx_anken_emails_received_at ON ses.anken_emails (received_at DESC);
CREATE INDEX IF NOT EXISTS idx_anken_emails_message_id ON ses.anken_emails (message_id);
"#;

/// Gmail人材メールの生データ
pub const JINZAI_EMAILS_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS ses.jinzai_emails (
    id BIGSERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,
    sender_address TEXT,
    sender_name TEXT,
    subject TEXT NOT NULL,
    body_text TEXT NOT NULL,
    received_at TIMESTAMPTZ NOT NULL,
    thread_id TEXT,
    skillsheet_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idx_jinzai_emails_received_at ON ses.jinzai_emails (received_at DESC);
CREATE INDEX IF NOT EXISTS idx_jinzai_emails_message_id ON ses.jinzai_emails (message_id);
"#;

/// Snapshot of parsed talent payloads keyed by message_id.
pub const TALENTS_ENUM_DDL: &str = r#"
CREATE TABLE ses.talents_enum (
    id BIGSERIAL PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,
    talent_name TEXT NOT NULL,
    summary_text TEXT,
    desired_price_min INTEGER,
    available_date DATE,
    received_at TIMESTAMPTZ NOT NULL,
    source_text TEXT
);

CREATE INDEX idx_talents_enum_message_id ON ses.talents_enum(message_id);
"#;

/// Master talent table for Lark-sourced talent data.
/// This is the authoritative source for talent matching.
pub const TALENTS_DDL: &str = r#"
CREATE TABLE ses.talents (
    id BIGSERIAL PRIMARY KEY,

    -- 基本情報
    name TEXT NOT NULL,
    age INTEGER,
    birth_year INTEGER,
    nearest_station TEXT,

    -- 単価・稼働
    desired_price INTEGER,           -- 参考原価（月額）
    current_utilization REAL,        -- 現稼働率 (0.0-1.0)
    available_date DATE,             -- 稼働開始可能日

    -- 営業ステータス
    sales_status TEXT,               -- NG, SPONTO稼働中, 他社稼働中, etc.
    sales_rep TEXT,                  -- 担当営業
    priority_rank TEXT,              -- 注力ランク

    -- スキル・能力
    skill_tags TEXT[],               -- スキルタグ配列
    project_preferences TEXT[],      -- 案件希望配列
    usage_types TEXT[],              -- 用途（SES, コンサル, 受託）

    -- 能力レベル（◎〇△✕ → 数値変換: ◎=3, 〇=2, △=1, ✕=0）
    capability_pm INTEGER,           -- PM/PMO
    capability_se INTEGER,           -- SE
    capability_bpo INTEGER,          -- BPO
    capability_consul INTEGER,       -- コンサル

    -- 言語
    english_level TEXT,

    -- 商流
    business_relationship TEXT,      -- SPONTOから見る商流

    -- 連絡先
    phone TEXT,
    email TEXT,
    linkedin_url TEXT,

    -- スキルシート
    skill_sheet_url TEXT,
    skill_sheet_url_2 TEXT,

    -- メタデータ
    memo TEXT,
    inflow_source TEXT,              -- 流入経路
    inflow_date DATE,                -- 流入日
    registration_date DATE,          -- 登録日

    -- Lark同期用
    lark_record_id TEXT UNIQUE,      -- Lark上のレコードID（将来の同期用）

    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX idx_talents_name ON ses.talents(name);
CREATE INDEX idx_talents_sales_status ON ses.talents(sales_status);
CREATE INDEX idx_talents_available ON ses.talents(available_date) WHERE available_date IS NOT NULL;
CREATE INDEX idx_talents_price ON ses.talents(desired_price) WHERE desired_price IS NOT NULL;
CREATE INDEX idx_talents_skills ON ses.talents USING GIN(skill_tags);

COMMENT ON TABLE ses.talents IS 'マスター人材テーブル（Larkから同期）';
"#;

/// Snapshot of parsed project payloads keyed by message_id.
pub const PROJECTS_ENUM_DDL: &str = r#"
CREATE TABLE ses.projects_enum (
    project_code BIGINT PRIMARY KEY,
    message_id VARCHAR(255) NOT NULL UNIQUE,
    project_name TEXT NOT NULL,
    monthly_tanka_min INTEGER,
    monthly_tanka_max INTEGER,
    start_date DATE,
    source_text TEXT,
    requires_manual_review BOOLEAN DEFAULT false,
    manual_review_reason TEXT
);

CREATE INDEX idx_projects_enum_message_id ON ses.projects_enum(message_id);
"#;

/// Proposed schema for daily match results snapshots.
/// run_date is a generated column based on created_at in RUN_DATE_TIMEZONE.
/// Same-day updates overwrite the previous record (UPSERT pattern).
pub static MATCH_RESULTS_DDL: Lazy<String> = Lazy::new(|| {
    format!(
        r#"
CREATE TABLE ses.match_results (
    id BIGSERIAL PRIMARY KEY,
    talent_id BIGINT NOT NULL,
    project_id BIGINT NOT NULL,

    is_knockout BOOLEAN NOT NULL,
    ko_reasons JSONB,
    needs_manual_review BOOLEAN NOT NULL DEFAULT false,

    score_total DOUBLE PRECISION,
    score_breakdown JSONB,
    CONSTRAINT chk_score_total_range CHECK (score_total IS NULL OR (score_total >= 0.0 AND score_total <= 1.0)),

    engine_version VARCHAR(20),
    rule_version VARCHAR(20),

    -- 最後にこのスナップショットを更新した実行ID（ULID/UUID）
    last_match_run_id VARCHAR(64) NOT NULL,

    -- ソフトデリート対応（監査用）
    is_deleted BOOLEAN NOT NULL DEFAULT false,
    deleted_at TIMESTAMPTZ,
    deleted_by TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),

    -- 基準タイムゾーンの日付（自動算出 - アプリは触れない）
    run_date DATE GENERATED ALWAYS AS (
        {run_date}
    ) STORED,

    CONSTRAINT uniq_match_results_active UNIQUE (talent_id, project_id, run_date, last_match_run_id)
        WHERE deleted_at IS NULL
);

CREATE INDEX idx_match_results_talent_run_date ON ses.match_results(talent_id, run_date DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_match_results_project_run_date ON ses.match_results(project_id, run_date DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_match_results_project_score_created
  ON ses.match_results(project_id, score_total DESC, created_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_match_results_score ON ses.match_results(score_total DESC)
  WHERE NOT is_knockout AND deleted_at IS NULL;
CREATE INDEX idx_match_results_match_run ON ses.match_results(last_match_run_id)
  WHERE last_match_run_id IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_match_results_score_breakdown_json
  ON ses.match_results USING GIN(score_breakdown jsonb_path_ops)
  WHERE score_breakdown IS NOT NULL AND deleted_at IS NULL;
"#,
        run_date = RUN_DATE_EXPRESSION.as_str(),
    )
});

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
    created_at TIMESTAMPTZ DEFAULT clock_timestamp()
);

CREATE INDEX idx_llm_comparison_message ON ses.llm_comparison_results(message_id);
CREATE INDEX idx_llm_comparison_created ON ses.llm_comparison_results(created_at);
CREATE INDEX idx_llm_comparison_providers ON ses.llm_comparison_results(primary_provider, shadow_provider);
"#;

/// Unified event log for GUI and sales feedback.
pub static FEEDBACK_EVENTS_DDL: Lazy<String> = Lazy::new(|| {
    format!(
        r#"
CREATE TABLE ses.feedback_events (
    id BIGSERIAL PRIMARY KEY,

    -- 紐付け（interaction_logs への FK を推奨）
    interaction_id BIGINT REFERENCES ses.interaction_logs(id) ON DELETE CASCADE,
    match_result_id BIGINT REFERENCES ses.match_results(id) ON DELETE CASCADE,
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
    -- revoke は元行を UPDATE で表現（append-only ではない）
    is_revoked BOOLEAN NOT NULL DEFAULT false,
    revoked_at TIMESTAMPTZ,
    revoked_by TEXT,  -- 誰が取り消したか（監査用）

    -- 誰が・どこから
    actor TEXT NOT NULL,   -- user_id / "sales" / "ops" / "system"
    source TEXT NOT NULL,  -- "gui" / "crm" / "api" / "import"

    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),

    -- パーティションキー（指定タイムゾーンの日付）
    event_date DATE GENERATED ALWAYS AS (
        {run_date}
    ) STORED,

    CONSTRAINT uniq_feedback_events_actor_type UNIQUE (interaction_id, feedback_type, actor, event_date)
)
PARTITION BY RANGE (event_date);

-- パーティション
CREATE TABLE IF NOT EXISTS ses.feedback_events_default PARTITION OF ses.feedback_events DEFAULT;

DO $$
DECLARE
    month_start DATE := date_trunc('month', now())::date;
    next_month DATE := (month_start + INTERVAL '1 month')::date;
    prev_month DATE := (month_start - INTERVAL '1 month')::date;
BEGIN
    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS ses.feedback_events_p%s PARTITION OF ses.feedback_events FOR VALUES FROM (%L) TO (%L)',
        to_char(prev_month, 'YYYYMM'), prev_month, month_start
    );
    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS ses.feedback_events_p%s PARTITION OF ses.feedback_events FOR VALUES FROM (%L) TO (%L)',
        to_char(month_start, 'YYYYMM'), month_start, next_month
    );
END $$;

-- インデックス
CREATE INDEX idx_feedback_events_interaction ON ses.feedback_events(interaction_id);
CREATE INDEX idx_feedback_events_match_result ON ses.feedback_events(match_result_id);
CREATE INDEX idx_feedback_events_match_run ON ses.feedback_events(match_run_id);
CREATE INDEX idx_feedback_events_project_talent ON ses.feedback_events(project_id, talent_id);
CREATE INDEX idx_feedback_events_type_created_at ON ses.feedback_events(feedback_type, created_at DESC);
CREATE INDEX idx_feedback_events_actor_created_at ON ses.feedback_events(actor, created_at DESC);
CREATE INDEX idx_feedback_events_not_revoked ON ses.feedback_events(interaction_id, created_at DESC)
    WHERE is_revoked = false;

-- 既存の created_at を再利用して日付パーティションの一意性を維持
CREATE OR REPLACE FUNCTION ses.feedback_events_align_created_at()
RETURNS TRIGGER AS $$
DECLARE
    existing_created_at TIMESTAMPTZ;
BEGIN
    SELECT created_at INTO existing_created_at
    FROM ses.feedback_events
    WHERE interaction_id = NEW.interaction_id
      AND feedback_type = NEW.feedback_type
      AND actor = NEW.actor
    ORDER BY created_at DESC
    LIMIT 1;

    IF existing_created_at IS NOT NULL THEN
        NEW.created_at := existing_created_at;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_feedback_events_align_created_at ON ses.feedback_events;
CREATE TRIGGER trg_feedback_events_align_created_at
BEFORE INSERT ON ses.feedback_events
FOR EACH ROW EXECUTE FUNCTION ses.feedback_events_align_created_at();

COMMENT ON TABLE ses.feedback_events IS '営業/GUIフィードバックの統一イベントログ（Two-Tower学習の正解ラベル源）';
"#,
        run_date = RUN_DATE_EXPRESSION.as_str(),
    )
});

/// GUI行動ログ: FBを押さなくても残る「良い兆候」イベント
/// idempotency_key で同じ操作のリトライを冪等に処理
///
/// ユニーク戦略:
/// - idempotency_key: グローバルユニーク（同一リクエストの再送防止）
/// - shortlisted: interaction + actor で1回だけ（トグル状態は meta.active で表現）
/// - その他: 複数回OK（閲覧回数、再連絡など価値がある）
pub const INTERACTION_EVENTS_DDL: &str = r#"
CREATE TABLE ses.interaction_events (
    id BIGSERIAL PRIMARY KEY,
    interaction_id BIGINT NOT NULL REFERENCES ses.interaction_logs(id) ON DELETE CASCADE,

    -- イベント種別
    -- Phase 1: viewed_candidate_detail, copied_template, clicked_contact, shortlisted
    event_type TEXT NOT NULL,
    CONSTRAINT chk_interaction_event_type CHECK (event_type IN (
        'viewed_candidate_detail',
        'copied_template',
        'clicked_contact',
        'shortlisted'
    )),

    -- 誰が・どこから
    actor TEXT NOT NULL,          -- JWTならsub、APIキーなら固定ID
    source TEXT NOT NULL DEFAULT 'gui',

    -- 冪等性キー（同じ操作のリトライで重複INSERT防止）
    idempotency_key TEXT NOT NULL UNIQUE,

    -- 追加情報
    meta JSONB,  -- { "template": "default" }, { "active": true } など

    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

-- shortlisted は 1回だけ（トグルなら meta.active で状態管理、latest が正）
CREATE UNIQUE INDEX uniq_interaction_shortlist_once
    ON ses.interaction_events(interaction_id, actor)
    WHERE event_type = 'shortlisted';

CREATE INDEX idx_interaction_events_interaction ON ses.interaction_events(interaction_id, created_at DESC);
CREATE INDEX idx_interaction_events_actor ON ses.interaction_events(actor, created_at DESC);
CREATE INDEX idx_interaction_events_type ON ses.interaction_events(event_type, created_at DESC);

COMMENT ON TABLE ses.interaction_events IS 'GUI行動ログ（FBなしでも良い兆候を取る）';
"#;

/// CVログ: 面談化/成約など実際のビジネス成果
/// interaction_id が取れない場合は talent_id/project_id で紐づけ
pub const CONVERSION_EVENTS_DDL: &str = r#"
CREATE TABLE ses.conversion_events (
    id BIGSERIAL PRIMARY KEY,

    -- 紐づけ（interaction_id が取れれば最高、取れなければ talent/project で）
    interaction_id BIGINT REFERENCES ses.interaction_logs(id) ON DELETE CASCADE,
    talent_id BIGINT NOT NULL,
    project_id BIGINT NOT NULL,

    -- ステージ
    -- 進行順: contacted → entry → interview_scheduled → offer → contract_signed
    -- 離脱: lost（どの段階でも発生しうる）
    stage TEXT NOT NULL,
    CONSTRAINT chk_conversion_stage CHECK (stage IN (
        'contacted',           -- 連絡済み
        'entry',               -- エントリー完了
        'interview_scheduled', -- 面談設定
        'offer',               -- オファー
        'contract_signed',     -- 成約
        'lost'                 -- 離脱/NG
    )),

    -- 誰が・どこから
    actor TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'gui',  -- gui / crm / import

    -- 追加情報
    meta JSONB,  -- { "lost_reason": "tanka" } など

    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX idx_conversion_events_interaction ON ses.conversion_events(interaction_id, created_at DESC);
CREATE INDEX idx_conversion_events_talent_project ON ses.conversion_events(talent_id, project_id, created_at DESC);
CREATE INDEX idx_conversion_events_stage ON ses.conversion_events(stage, created_at DESC);

COMMENT ON TABLE ses.conversion_events IS 'CV（面談化/成約）ログ（Two-Tower学習の強いシグナル）';
"#;

/// Interaction logging for recommendations and downstream training views.
/// run_date is a generated column based on created_at in JST timezone.
/// UNIQUE is per (match_run_id, talent_id, project_id) to allow multiple runs per day.
pub static INTERACTION_LOGS_DDL: Lazy<String> = Lazy::new(|| {
    format!(
        r#"
CREATE TABLE ses.interaction_logs (
    id BIGSERIAL PRIMARY KEY,

    -- マッチング情報
    match_result_id BIGINT REFERENCES ses.match_results(id) ON DELETE SET NULL,
    talent_id BIGINT NOT NULL,
    project_id BIGINT NOT NULL,
    match_run_id VARCHAR(64) NOT NULL,  -- 実行インスタンスID（ULID/UUID、毎回生成）
    engine_version VARCHAR(20),
    config_version VARCHAR(20),

    -- Two-Tower 予測
    two_tower_score DOUBLE PRECISION,  -- 予測スコア
    two_tower_embedder VARCHAR(50),    -- hash / onnx / candle
    two_tower_version VARCHAR(20),     -- モデルバージョン

    -- ビジネスルールスコア（比較用）
    business_score DOUBLE PRECISION,

    -- 結果（後から更新）
    -- 許容値: accepted, rejected, interview_scheduled, review_ok, review_ng,
    --         thumbs_up, thumbs_down, no_response, NULL（初期値）
    -- ※ 'pending' 文字列は使わない（初期状態 = NULL）
    outcome VARCHAR(20),
    feedback_at TIMESTAMPTZ,

    -- A/Bテスト
    variant VARCHAR(50),  -- 'control', 'two_tower_10pct', ...

    -- メタデータ
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),

    -- 基準タイムゾーンの日付（自動算出 - アプリは触れない、検索/集計用）
    run_date DATE GENERATED ALWAYS AS (
        {run_date}
    ) STORED,

    -- 同一 run 内の二重INSERT（リトライ/バグ）を抑止、別 run なら同日でも記録可
    CONSTRAINT interaction_logs_unique_run_pair UNIQUE (match_run_id, talent_id, project_id)
);

CREATE INDEX idx_interaction_logs_match_run ON ses.interaction_logs(match_run_id, created_at DESC);
CREATE INDEX idx_interaction_logs_match_result ON ses.interaction_logs(match_result_id);
CREATE INDEX idx_interaction_logs_talent_run_date ON ses.interaction_logs(talent_id, run_date DESC, created_at DESC);
CREATE INDEX idx_interaction_logs_project_run_date ON ses.interaction_logs(project_id, run_date DESC, created_at DESC);
CREATE INDEX idx_interaction_logs_outcome ON ses.interaction_logs(outcome, created_at DESC)
    WHERE outcome IS NOT NULL;

CREATE OR REPLACE VIEW ses.training_pairs AS
SELECT
    il.talent_id,
    il.project_id,
    il.two_tower_score,
    il.two_tower_embedder,
    il.two_tower_version,
    il.business_score,
    il.outcome,
    il.variant,
    CASE
        WHEN il.outcome = 'accepted' THEN 1.0
        WHEN il.outcome = 'rejected' THEN 0.0
        WHEN il.outcome = 'thumbs_up' THEN 1.0
        WHEN il.outcome = 'thumbs_down' THEN 0.0
        WHEN il.outcome = 'review_ok' THEN 1.0
        WHEN il.outcome = 'review_ng' THEN 0.0
        WHEN il.outcome = 'interview_scheduled' THEN 0.8
        ELSE NULL
    END AS label,
    il.run_date,
    il.created_at
FROM ses.interaction_logs il
WHERE il.outcome IS NOT NULL
  AND il.outcome <> 'no_response';
-- ※ 'pending' は NULL で表現するため、outcome IS NOT NULL で除外済み

CREATE OR REPLACE VIEW ses.training_stats AS
SELECT
    COUNT(*) FILTER (WHERE outcome = 'accepted') AS accepted_count,
    COUNT(*) FILTER (WHERE outcome = 'rejected') AS rejected_count,
    COUNT(*) FILTER (WHERE outcome IS NULL) AS pending_count,
    -- Cold Start判定用: training_pairsで使えるラベル総数
    -- ※ 'pending' は NULL で表現するため、outcome IS NOT NULL で除外済み
    COUNT(*) FILTER (WHERE outcome IS NOT NULL AND outcome <> 'no_response') AS labeled_count,
    MIN(created_at) AS first_log_at,
    MAX(created_at) AS last_log_at,
    COUNT(DISTINCT run_date) AS active_days  -- JST基準のrun_dateを使用
FROM ses.interaction_logs;

-- 統合学習ラベル: CV + FB + 行動ログを優先順位で統合
-- ※ training_pairs は後方互換のため残す。新規学習はこちらを使用推奨
--
-- ラベルスケール設計:
--   CV層（最強）: contract_signed=1.0, offer=0.9, interview=0.8, entry=0.7, contacted=0.4, lost=0.0
--   FB層（中間）: accepted/thumbs_up/review_ok=1.0, interview=0.8, rejected/thumbs_down/review_ng=0.0
--   行動層（弱）: shortlisted/clicked_contact=0.3, copied_template=0.2, viewed_detail=0.1
--
-- best_stage 採用理由:
--   外部要因で lost になっても "マッチング品質" まで否定しないため
--   final_stage が欲しい分析は別VIEWで ORDER BY created_at DESC にして作る
--
-- Phase 1: interaction_id があるCVだけ学習に使う（安全）
-- Phase 2: CRM import は mapping を作って interaction_id に紐付ける
CREATE OR REPLACE VIEW ses.training_labels AS
WITH cv_best AS (
    -- interaction_id ごとに最強のCVステージを取得
    SELECT DISTINCT ON (interaction_id)
        interaction_id,
        stage AS cv_stage,
        CASE stage
            WHEN 'contract_signed'     THEN 1.0
            WHEN 'offer'               THEN 0.9
            WHEN 'interview_scheduled' THEN 0.8
            WHEN 'entry'               THEN 0.7
            WHEN 'contacted'           THEN 0.4
            WHEN 'lost'                THEN 0.0
            ELSE NULL
        END AS cv_label
    FROM ses.conversion_events
    WHERE interaction_id IS NOT NULL
    ORDER BY interaction_id,
        CASE stage
            WHEN 'contract_signed'     THEN 1
            WHEN 'offer'               THEN 2
            WHEN 'interview_scheduled' THEN 3
            WHEN 'entry'               THEN 4
            WHEN 'contacted'           THEN 5
            WHEN 'lost'                THEN 6
            ELSE 999
        END ASC,
        created_at DESC
),
behavior_best AS (
    -- interaction_id ごとに最強の行動イベントを取得
    SELECT DISTINCT ON (interaction_id)
        interaction_id,
        event_type AS behavior_type,
        CASE event_type
            WHEN 'shortlisted'             THEN 0.3
            WHEN 'clicked_contact'         THEN 0.3
            WHEN 'copied_template'         THEN 0.2
            WHEN 'viewed_candidate_detail' THEN 0.1
            ELSE NULL
        END AS behavior_label
    FROM ses.interaction_events
    ORDER BY interaction_id,
        CASE event_type
            WHEN 'shortlisted'             THEN 1
            WHEN 'clicked_contact'         THEN 2
            WHEN 'copied_template'         THEN 3
            WHEN 'viewed_candidate_detail' THEN 4
            ELSE 999
        END ASC,
        created_at DESC
),
fb AS (
    -- FBラベル: training_pairs と同じスケール（accepted = 1.0）
    SELECT
        id AS interaction_id,
        CASE
            WHEN outcome = 'accepted'            THEN 1.0
            WHEN outcome = 'rejected'            THEN 0.0
            WHEN outcome = 'thumbs_up'           THEN 1.0
            WHEN outcome = 'thumbs_down'         THEN 0.0
            WHEN outcome = 'review_ok'           THEN 1.0
            WHEN outcome = 'review_ng'           THEN 0.0
            WHEN outcome = 'interview_scheduled' THEN 0.8
            ELSE NULL
        END AS fb_label
    FROM ses.interaction_logs
    WHERE outcome IS NOT NULL
      AND outcome <> 'no_response'
)
SELECT
    il.id AS interaction_id,
    il.talent_id,
    il.project_id,
    il.two_tower_score,
    il.two_tower_embedder,
    il.two_tower_version,
    il.business_score,
    il.variant,
    il.run_date,
    il.created_at,

    -- 元データ（デバッグ/分析用）
    cv.cv_stage,
    il.outcome AS fb_outcome,
    bb.behavior_type,

    -- signal_source: どのソースからラベルが来たか
    CASE
        WHEN cv.cv_label IS NOT NULL THEN 'conversion'
        WHEN fb.fb_label IS NOT NULL THEN 'feedback'
        WHEN bb.behavior_label IS NOT NULL THEN 'behavior'
        ELSE NULL
    END AS signal_source,

    -- label: 優先順位で統合（CV > FB > 行動ログ）
    -- NULL = 未知（学習には使わない、負例扱いしない）
    COALESCE(cv.cv_label, fb.fb_label, bb.behavior_label) AS label

FROM ses.interaction_logs il
LEFT JOIN cv_best cv       ON cv.interaction_id = il.id
LEFT JOIN fb               ON fb.interaction_id = il.id
LEFT JOIN behavior_best bb ON bb.interaction_id = il.id
WHERE cv.cv_label IS NOT NULL
   OR fb.fb_label IS NOT NULL
   OR bb.behavior_label IS NOT NULL;
"#,
        run_date = RUN_DATE_EXPRESSION.as_str(),
    )
});

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
            "idx_extraction_queue_status_created",
            "idx_extraction_queue_partial_fields_json",
        ] {
            assert!(EXTRACTION_QUEUE_DDL.contains(required));
        }
    }

    #[test]
    fn talents_enum_schema_covers_lookup_and_source_text() {
        for required in [
            "talent_name",
            "message_id",
            "source_text",
            "received_at",
            "idx_talents_enum_message_id",
        ] {
            assert!(TALENTS_ENUM_DDL.contains(required));
        }
    }

    #[test]
    fn projects_enum_schema_covers_lookup_and_manual_review() {
        for required in [
            "project_code",
            "message_id",
            "source_text",
            "requires_manual_review",
            "idx_projects_enum_message_id",
        ] {
            assert!(PROJECTS_ENUM_DDL.contains(required));
        }
    }

    #[test]
    fn match_results_schema_contains_indexes_and_uniques() {
        for required in [
            "talent_id",
            "project_id",
            "score_breakdown",
            "last_match_run_id VARCHAR(64) NOT NULL",
            "updated_at",
            "run_date DATE GENERATED ALWAYS",
            RUN_DATE_TIMEZONE,
            "is_deleted",
            "deleted_at",
            "deleted_by",
            "uniq_match_results_active UNIQUE (talent_id, project_id, run_date, last_match_run_id)",
            "idx_match_results_talent_run_date",
            "idx_match_results_project_run_date",
            "idx_match_results_project_score_created",
            "idx_match_results_score",
            "idx_match_results_match_run",
            "idx_match_results_score_breakdown_json",
        ] {
            assert!(
                MATCH_RESULTS_DDL.as_str().contains(required),
                "missing: {required}"
            );
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
            "revoked_by", // 監査用
            "idx_feedback_events_project_talent",
            "idx_feedback_events_match_run",
            "idx_feedback_events_not_revoked",
            "chk_feedback_type",
            "chk_ng_reason_category",
            "UNIQUE (interaction_id, feedback_type, actor, event_date)",
            "PARTITION BY RANGE (event_date)",
            "COMMENT ON TABLE ses.feedback_events",
        ] {
            assert!(
                FEEDBACK_EVENTS_DDL.as_str().contains(required),
                "missing: {required}"
            );
        }
    }

    #[test]
    fn interaction_logs_schema_covers_views_and_unique_constraint() {
        for required in [
            "two_tower_score",
            "two_tower_embedder",
            "two_tower_version",
            "variant",
            "business_score",
            "match_run_id VARCHAR(64) NOT NULL",
            "engine_version",
            "config_version",
            "run_date DATE GENERATED ALWAYS",
            RUN_DATE_TIMEZONE,
            "interaction_logs_unique_run_pair",
            "UNIQUE (match_run_id, talent_id, project_id)",
            "idx_interaction_logs_match_run",
            "idx_interaction_logs_match_result",
            "idx_interaction_logs_talent_run_date",
            "idx_interaction_logs_project_run_date",
            "idx_interaction_logs_outcome",
            "CREATE OR REPLACE VIEW ses.training_pairs",
            "CREATE OR REPLACE VIEW ses.training_stats",
            "labeled_count", // Cold Start判定用
        ] {
            assert!(
                INTERACTION_LOGS_DDL.as_str().contains(required),
                "missing: {required}"
            );
        }
    }

    #[test]
    fn training_pairs_view_exposes_two_tower_and_business_scores() {
        for required in [
            "il.two_tower_score",
            "il.two_tower_embedder",
            "il.two_tower_version",
            "il.business_score",
            "il.variant",
            "CASE",
        ] {
            assert!(
                INTERACTION_LOGS_DDL.as_str().contains(required),
                "missing in training_pairs view: {required}"
            );
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

    #[test]
    fn interaction_events_schema_covers_event_types_and_idempotency() {
        for required in [
            "interaction_id",
            "event_type",
            "actor",
            "source",
            "idempotency_key TEXT NOT NULL UNIQUE",
            "meta JSONB",
            "viewed_candidate_detail",
            "copied_template",
            "clicked_contact",
            "shortlisted",
            "chk_interaction_event_type",
            "idx_interaction_events_interaction",
            "idx_interaction_events_actor",
            "idx_interaction_events_type",
            "uniq_interaction_shortlist_once",
            "WHERE event_type = 'shortlisted'",
            "COMMENT ON TABLE ses.interaction_events",
        ] {
            assert!(
                INTERACTION_EVENTS_DDL.contains(required),
                "missing: {required}"
            );
        }
    }

    #[test]
    fn conversion_events_schema_covers_stages_and_indexes() {
        for required in [
            "interaction_id",
            "talent_id",
            "project_id",
            "stage",
            "actor",
            "source",
            "meta JSONB",
            "contacted",
            "entry",
            "interview_scheduled",
            "offer",
            "contract_signed",
            "lost",
            "chk_conversion_stage",
            "idx_conversion_events_interaction",
            "idx_conversion_events_talent_project",
            "idx_conversion_events_stage",
            "COMMENT ON TABLE ses.conversion_events",
        ] {
            assert!(
                CONVERSION_EVENTS_DDL.contains(required),
                "missing: {required}"
            );
        }
    }

    #[test]
    fn interaction_logs_has_training_labels_view() {
        for required in [
            "CREATE OR REPLACE VIEW ses.training_labels",
            "cv_best",
            "behavior_best",
            "fb AS", // fb CTE for feedback labels
            "fb_label",
            "signal_source",
            "COALESCE(cv.cv_label, fb.fb_label, bb.behavior_label)",
            "WHERE interaction_id IS NOT NULL", // Phase 1: only CVs with interaction_id
            "best_stage 採用理由",
            "Phase 1:",
            "Phase 2:",
        ] {
            assert!(
                INTERACTION_LOGS_DDL.as_str().contains(required),
                "missing: {required}"
            );
        }
    }
}
