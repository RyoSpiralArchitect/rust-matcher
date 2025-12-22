use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::PoolError;
use serde_json::Value;
use tokio_postgres::Error as PgError;
use tokio_postgres::Row;
use tokio_postgres::types::Json;
use tokio_postgres::types::ToSql;
use tracing::instrument;

use crate::api::queue_job::{
    FeedbackEventRow, InteractionLogRow, JobDetailIncludes, JobEntity, MatchResultRow, Pagination,
    PairDetail, ProjectSnapshot, QueueJobDetail, QueueJobDetailResponse, QueueJobFilter,
    QueueJobListResponse, QueueJobSummary, TalentSnapshot,
};
use crate::db::PgPool;
use crate::queue::{ExtractionJob, QueueStatus};

#[derive(Debug, thiserror::Error)]
pub enum QueueStorageError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
    #[error("failed to map queue row: {0}")]
    Mapping(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
}

fn normalize_json(value: &Option<Value>) -> Option<Json<&Value>> {
    value.as_ref().map(Json)
}

/// Insert or update a queue row based on `message_id`.
#[instrument(skip(pool, job))]
pub async fn upsert_extraction_job(
    pool: &PgPool,
    job: &ExtractionJob,
) -> Result<u64, QueueStorageError> {
    let client = pool.get().await?;

    let stmt = client
        .prepare(
            "INSERT INTO ses.extraction_queue (
                message_id,
                email_subject,
                email_received_at,
                subject_hash,
                status,
                priority,
                locked_by,
                retry_count,
                next_retry_at,
                last_error,
                partial_fields,
                decision_reason,
                recommended_method,
                final_method,
                extractor_version,
                rule_version,
                created_at,
                processing_started_at,
                completed_at,
                updated_at,
                llm_latency_ms,
                requires_manual_review,
                manual_review_reason,
                reprocess_after,
                canary_target
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25
            )
            ON CONFLICT (message_id) DO UPDATE SET
                email_subject = EXCLUDED.email_subject,
                email_received_at = EXCLUDED.email_received_at,
                subject_hash = EXCLUDED.subject_hash,
                status = EXCLUDED.status,
                priority = EXCLUDED.priority,
                locked_by = EXCLUDED.locked_by,
                retry_count = EXCLUDED.retry_count,
                next_retry_at = EXCLUDED.next_retry_at,
                last_error = EXCLUDED.last_error,
                partial_fields = EXCLUDED.partial_fields,
                decision_reason = EXCLUDED.decision_reason,
                recommended_method = EXCLUDED.recommended_method,
                final_method = EXCLUDED.final_method,
                extractor_version = EXCLUDED.extractor_version,
                rule_version = EXCLUDED.rule_version,
                processing_started_at = EXCLUDED.processing_started_at,
                completed_at = EXCLUDED.completed_at,
                updated_at = EXCLUDED.updated_at,
                llm_latency_ms = EXCLUDED.llm_latency_ms,
                requires_manual_review = EXCLUDED.requires_manual_review,
                manual_review_reason = EXCLUDED.manual_review_reason,
                reprocess_after = EXCLUDED.reprocess_after,
                canary_target = EXCLUDED.canary_target;",
        )
        .await?;

    let recommended = job.recommended_method.as_ref().map(|r| r.as_str());
    let final_method = job.final_method.as_ref().map(|f| f.as_str());

    let rows = client
        .execute(
            &stmt,
            &[
                &job.message_id,
                &job.email_subject,
                &job.email_received_at,
                &job.subject_hash,
                &job.status.as_str(),
                &job.priority,
                &job.locked_by,
                &i32::try_from(job.retry_count).unwrap_or(i32::MAX),
                &job.next_retry_at,
                &job.last_error,
                &normalize_json(&job.partial_fields),
                &job.decision_reason,
                &recommended,
                &final_method,
                &job.extractor_version,
                &job.rule_version,
                &job.created_at,
                &job.processing_started_at,
                &job.completed_at,
                &job.updated_at,
                &job.llm_latency_ms,
                &job.requires_manual_review,
                &job.manual_review_reason,
                &job.reprocess_after,
                &job.canary_target,
            ],
        )
        .await?;

    Ok(rows)
}

/// Reset long-running processing jobs back to pending.
#[instrument(skip(pool))]
pub async fn recover_stuck_jobs(
    pool: &PgPool,
    now: DateTime<Utc>,
    max_processing: Duration,
) -> Result<u64, QueueStorageError> {
    let client = pool.get().await?;
    let cutoff = now - max_processing;

    let stmt = client
        .prepare(
            "UPDATE ses.extraction_queue SET
                status = 'pending',
                locked_by = NULL,
                next_retry_at = $1,
                updated_at = $1
            WHERE status = 'processing'
              AND COALESCE(processing_started_at, updated_at) <= $2",
        )
        .await?;

    let rows = client.execute(&stmt, &[&now, &cutoff]).await?;
    Ok(rows)
}

/// Return a safe pending copy for enqueueing without leaking processing metadata.
pub fn pending_copy(job: &ExtractionJob, received_at: DateTime<Utc>) -> ExtractionJob {
    let mut pending = job.clone();
    pending.status = QueueStatus::Pending;
    pending.retry_count = 0;
    pending.locked_by = None;
    pending.next_retry_at = None;
    pending.last_error = None;
    pending.final_method = None;
    pending.processing_started_at = None;
    pending.completed_at = None;
    pending.llm_latency_ms = None;
    pending.reprocess_after = None;
    pending.email_received_at = received_at;
    pending.updated_at = Utc::now();
    pending
}

fn parse_status(value: &str) -> Result<QueueStatus, QueueStorageError> {
    match value {
        "pending" => Ok(QueueStatus::Pending),
        "processing" => Ok(QueueStatus::Processing),
        "completed" => Ok(QueueStatus::Completed),
        other => Err(QueueStorageError::Mapping(format!(
            "unknown status: {other}"
        ))),
    }
}

fn parse_recommended(value: &str) -> Result<crate::queue::RecommendedMethod, QueueStorageError> {
    use crate::queue::RecommendedMethod;

    match value {
        "rust_recommended" => Ok(RecommendedMethod::RustRecommended),
        "llm_recommended" => Ok(RecommendedMethod::LlmRecommended),
        other => Err(QueueStorageError::Mapping(format!(
            "unknown recommended_method: {other}"
        ))),
    }
}

fn parse_final(value: &str) -> Result<crate::queue::FinalMethod, QueueStorageError> {
    use crate::queue::FinalMethod;

    match value {
        "rust_completed" => Ok(FinalMethod::RustCompleted),
        "llm_completed" => Ok(FinalMethod::LlmCompleted),
        "manual_review" => Ok(FinalMethod::ManualReview),
        other => Err(QueueStorageError::Mapping(format!(
            "unknown final_method: {other}"
        ))),
    }
}

fn row_to_job(row: &Row) -> Result<ExtractionJob, QueueStorageError> {
    Ok(ExtractionJob {
        id: row
            .try_get::<_, i64>("id")
            .map_err(QueueStorageError::from)
            .and_then(|id| {
                u64::try_from(id).map_err(|e| QueueStorageError::Mapping(e.to_string()))
            })?,
        message_id: row.try_get("message_id")?,
        email_subject: row.try_get("email_subject")?,
        email_received_at: row.try_get("email_received_at")?,
        subject_hash: row.try_get("subject_hash")?,
        status: parse_status(row.try_get::<_, String>("status")?.as_str())?,
        priority: row.try_get("priority")?,
        locked_by: row.try_get("locked_by")?,
        retry_count: row
            .try_get::<_, i32>("retry_count")
            .map_err(QueueStorageError::from)
            .and_then(|v| {
                u32::try_from(v).map_err(|e| QueueStorageError::Mapping(e.to_string()))
            })?,
        next_retry_at: row.try_get("next_retry_at")?,
        last_error: row.try_get("last_error")?,
        partial_fields: row.try_get("partial_fields")?,
        decision_reason: row.try_get("decision_reason")?,
        recommended_method: row
            .try_get::<_, Option<String>>("recommended_method")?
            .map(|s| parse_recommended(&s))
            .transpose()?,
        final_method: row
            .try_get::<_, Option<String>>("final_method")?
            .map(|s| parse_final(&s))
            .transpose()?,
        extractor_version: row.try_get("extractor_version")?,
        rule_version: row.try_get("rule_version")?,
        created_at: row.try_get("created_at")?,
        processing_started_at: row.try_get("processing_started_at")?,
        completed_at: row.try_get("completed_at")?,
        updated_at: row.try_get("updated_at")?,
        llm_latency_ms: row.try_get("llm_latency_ms")?,
        requires_manual_review: row.try_get("requires_manual_review")?,
        manual_review_reason: row.try_get("manual_review_reason")?,
        reprocess_after: row.try_get("reprocess_after")?,
        canary_target: row.try_get("canary_target")?,
    })
}

fn row_to_job_summary(row: &Row) -> QueueJobSummary {
    QueueJobSummary {
        id: row.get("id"),
        message_id: row.get("message_id"),
        status: row.get("status"),
        priority: row.get("priority"),
        retry_count: row.get("retry_count"),
        next_retry_at: row.get("next_retry_at"),
        final_method: row.get("final_method"),
        requires_manual_review: row.get("requires_manual_review"),
        manual_review_reason: row.get("manual_review_reason"),
        decision_reason: row.get("decision_reason"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_job_detail(row: &Row) -> QueueJobDetail {
    QueueJobDetail {
        job: row_to_job_summary(row),
        partial_fields: row.get("partial_fields"),
        last_error: row.get("last_error"),
        llm_latency_ms: row.get("llm_latency_ms"),
        processing_started_at: row.get("processing_started_at"),
        completed_at: row.get("completed_at"),
    }
}

fn row_to_job_detail_response(row: &Row) -> QueueJobDetailResponse {
    let base = row_to_job_detail(row);
    QueueJobDetailResponse {
        job: base.job,
        partial_fields: base.partial_fields,
        last_error: base.last_error,
        llm_latency_ms: base.llm_latency_ms,
        processing_started_at: base.processing_started_at,
        completed_at: base.completed_at,
        entity: None,
        pairs: None,
        source_preview: None,
    }
}

const SOURCE_PREVIEW_LIMIT: usize = 4000;

fn truncate_source_preview(text: &str) -> String {
    if text.chars().count() <= SOURCE_PREVIEW_LIMIT {
        return text.to_string();
    }

    text.chars()
        .take(SOURCE_PREVIEW_LIMIT)
        .collect::<String>()
}

/// Lock and return the next pending job ordered by priority and created_at.
#[instrument(skip(pool))]
pub async fn lock_next_pending_job(
    pool: &PgPool,
    worker_id: &str,
    now: DateTime<Utc>,
) -> Result<Option<ExtractionJob>, QueueStorageError> {
    let client = pool.get().await?;
    let stmt = client
        .prepare(
            "UPDATE ses.extraction_queue
SET
    status = 'processing',
    locked_by = $1,
    processing_started_at = $2,
    updated_at = $2
WHERE id = (
    SELECT id
    FROM ses.extraction_queue
    WHERE status = 'pending'
      AND (next_retry_at IS NULL OR next_retry_at <= $2)
      AND (reprocess_after IS NULL OR reprocess_after <= $2)
    ORDER BY priority DESC, created_at
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)
RETURNING *;",
        )
        .await?;

    let row = client.query_opt(&stmt, &[&worker_id, &now]).await?;
    row.map(|r| row_to_job(&r)).transpose()
}

#[instrument(skip(pool))]
pub async fn list_jobs(
    pool: &PgPool,
    filter: &QueueJobFilter,
    pagination: &Pagination,
) -> Result<QueueJobListResponse, QueueStorageError> {
    let client = pool.get().await?;

    let fetch_limit = pagination.limit + 1;

    let mut values: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();
    let mut query = String::from(
        "SELECT id, message_id, status, priority, retry_count, next_retry_at, final_method, requires_manual_review, manual_review_reason, decision_reason, created_at, updated_at FROM ses.extraction_queue WHERE 1=1",
    );

    if let Some(status) = &filter.status {
        query.push_str(&format!(" AND status = ${}", values.len() + 1));
        values.push(Box::new(status.clone()));
    }

    if let Some(requires_manual_review) = filter.requires_manual_review {
        query.push_str(&format!(
            " AND requires_manual_review = ${}",
            values.len() + 1
        ));
        values.push(Box::new(requires_manual_review));
    }

    if let Some(canary_target) = filter.canary_target {
        query.push_str(&format!(" AND canary_target = ${}", values.len() + 1));
        values.push(Box::new(canary_target));
    }

    if let Some(final_method) = &filter.final_method {
        query.push_str(&format!(" AND final_method = ${}", values.len() + 1));
        values.push(Box::new(final_method.clone()));
    }

    if let Some(manual_review_reason) = &filter.manual_review_reason {
        query.push_str(&format!(
            " AND manual_review_reason = ${}",
            values.len() + 1
        ));
        values.push(Box::new(manual_review_reason.clone()));
    }

    if let Some(created_after) = filter.created_after {
        query.push_str(&format!(" AND created_at >= ${}", values.len() + 1));
        values.push(Box::new(created_after));
    }

    if let Some(created_before) = filter.created_before {
        query.push_str(&format!(" AND created_at <= ${}", values.len() + 1));
        values.push(Box::new(created_before));
    }

    query.push_str(&format!(
        " ORDER BY created_at DESC, id DESC LIMIT ${} OFFSET ${}",
        values.len() + 1,
        values.len() + 2
    ));

    values.push(Box::new(fetch_limit));
    values.push(Box::new(pagination.offset));

    let params: Vec<&(dyn ToSql + Sync)> = values
        .iter()
        .map(|v| v.as_ref() as &(dyn ToSql + Sync))
        .collect();
    let rows = client.query(&query, &params).await?;

    let mut items: Vec<QueueJobSummary> = rows.iter().map(row_to_job_summary).collect();
    let has_more = (items.len() as i64) > pagination.limit;
    if has_more {
        items.pop();
    }

    Ok(QueueJobListResponse {
        items,
        limit: pagination.limit,
        offset: pagination.offset,
        has_more,
    })
}

#[instrument(skip(pool))]
pub async fn get_job_by_id(
    pool: &PgPool,
    id: i64,
) -> Result<Option<QueueJobDetail>, QueueStorageError> {
    let includes = JobDetailIncludes {
        limit: 1,
        ..Default::default()
    };

    get_job_detail_with_includes(pool, id, includes, false)
        .await
        .map(|opt| {
            opt.map(|detail| QueueJobDetail {
                job: detail.job,
                partial_fields: detail.partial_fields,
                last_error: detail.last_error,
                llm_latency_ms: detail.llm_latency_ms,
                processing_started_at: detail.processing_started_at,
                completed_at: detail.completed_at,
            })
        })
}

async fn fetch_talent_snapshot(
    client: &tokio_postgres::Client,
    message_id: &str,
    include_source: bool,
) -> Result<Option<TalentSnapshot>, QueueStorageError> {
    let stmt = client
        .prepare(
            "SELECT id, message_id, talent_name, summary_text, desired_price_min, available_date, received_at, source_text FROM ses.talents_enum WHERE message_id = $1 LIMIT 1",
        )
        .await?;

    let row = client.query_opt(&stmt, &[&message_id]).await?;
    let snapshot = row.map(|r| TalentSnapshot {
        id: r.get::<_, i64>("id"),
        message_id: r.get("message_id"),
        talent_name: r.get("talent_name"),
        summary_text: r.get("summary_text"),
        desired_price_min: r.get("desired_price_min"),
        available_date: r.get("available_date"),
        received_at: r.get("received_at"),
        source_text: if include_source {
            r.get("source_text")
        } else {
            None
        },
    });

    Ok(snapshot)
}

async fn fetch_project_snapshot(
    client: &tokio_postgres::Client,
    message_id: &str,
    include_source: bool,
) -> Result<Option<ProjectSnapshot>, QueueStorageError> {
    let stmt = client
        .prepare(
            "SELECT project_code, message_id, project_name, monthly_tanka_min, monthly_tanka_max, start_date, source_text, requires_manual_review, manual_review_reason FROM ses.projects_enum WHERE message_id = $1 LIMIT 1",
        )
        .await?;

    let row = client.query_opt(&stmt, &[&message_id]).await?;
    let snapshot = row.map(|r| ProjectSnapshot {
        project_code: r.get::<_, i32>("project_code") as i64,
        message_id: r.get("message_id"),
        project_name: r.get("project_name"),
        monthly_tanka_min: r.get("monthly_tanka_min"),
        monthly_tanka_max: r.get("monthly_tanka_max"),
        start_date: r.get("start_date"),
        source_text: if include_source {
            r.get("source_text")
        } else {
            None
        },
        requires_manual_review: r.get("requires_manual_review"),
        manual_review_reason: r.get("manual_review_reason"),
    });

    Ok(snapshot)
}

fn map_match_result(row: &Row) -> MatchResultRow {
    let ko_reasons_value: Option<Value> = row.get("ko_reasons");
    let ko_reasons = match ko_reasons_value {
        Some(Value::Array(items)) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => vec![],
    };

    MatchResultRow {
        id: row.get::<_, i32>("id") as i64,
        talent_id: row.get::<_, i32>("talent_id") as i64,
        project_id: row.get::<_, i32>("project_id") as i64,
        is_knockout: row.get("is_knockout"),
        ko_reasons,
        needs_manual_review: row.get("needs_manual_review"),
        score_total: row.get::<_, Option<f64>>("score_total").map(|v| v as f32),
        score_breakdown: row.get("score_breakdown"),
        engine_version: row.get("engine_version"),
        rule_version: row.get("rule_version"),
        created_at: row.get("created_at"),
    }
}

async fn fetch_match_results(
    client: &tokio_postgres::Client,
    talent_id: Option<i64>,
    project_id: Option<i64>,
    days: i64,
    limit: i64,
) -> Result<Vec<MatchResultRow>, QueueStorageError> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    if let Some(talent_id) = talent_id {
        let stmt = client
            .prepare(
                "SELECT id, talent_id, project_id, is_knockout, ko_reasons, needs_manual_review, score_total, score_breakdown, engine_version, rule_version, created_at FROM ses.match_results WHERE talent_id = $1 AND created_at >= NOW() - ($2::int * INTERVAL '1 day') ORDER BY created_at DESC LIMIT $3",
            )
            .await?;
        let rows = client.query(&stmt, &[&talent_id, &days, &limit]).await?;
        for row in rows {
            let mapped = map_match_result(&row);
            seen.insert(mapped.id);
            results.push(mapped);
        }
    }

    if let Some(project_id) = project_id {
        let stmt = client
            .prepare(
                "SELECT id, talent_id, project_id, is_knockout, ko_reasons, needs_manual_review, score_total, score_breakdown, engine_version, rule_version, created_at FROM ses.match_results WHERE project_id = $1 AND created_at >= NOW() - ($2::int * INTERVAL '1 day') ORDER BY created_at DESC LIMIT $3",
            )
            .await?;
        let rows = client.query(&stmt, &[&project_id, &days, &limit]).await?;
        for row in rows {
            let mapped = map_match_result(&row);
            if seen.insert(mapped.id) {
                results.push(mapped);
            }
        }
    }

    Ok(results)
}

async fn fetch_interactions(
    client: &tokio_postgres::Client,
    match_result_ids: &[i32],
) -> Result<Vec<InteractionLogRow>, QueueStorageError> {
    if match_result_ids.is_empty() {
        return Ok(Vec::new());
    }

    let stmt = client
        .prepare(
            "SELECT id, match_result_id, talent_id, project_id, match_run_id, engine_version, config_version, two_tower_score, business_score, outcome, feedback_at, created_at FROM ses.interaction_logs WHERE match_result_id = ANY($1::int[]) ORDER BY created_at DESC",
        )
        .await?;

    let rows = client.query(&stmt, &[&match_result_ids]).await?;

    let interactions = rows
        .into_iter()
        .map(|row| InteractionLogRow {
            id: row.get::<_, i64>("id"),
            match_result_id: row
                .get::<_, Option<i32>>("match_result_id")
                .map(|v| v as i64),
            talent_id: row.get::<_, i32>("talent_id") as i64,
            project_id: row.get::<_, i32>("project_id") as i64,
            match_run_id: row.get("match_run_id"),
            engine_version: row.get("engine_version"),
            config_version: row.get("config_version"),
            two_tower_score: row.get("two_tower_score"),
            business_score: row.get("business_score"),
            outcome: row.get("outcome"),
            feedback_at: row.get("feedback_at"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(interactions)
}

async fn fetch_feedback_events(
    client: &tokio_postgres::Client,
    interaction_ids: &[i64],
    match_result_ids: &[i32],
) -> Result<Vec<FeedbackEventRow>, QueueStorageError> {
    if interaction_ids.is_empty() && match_result_ids.is_empty() {
        return Ok(Vec::new());
    }

    let stmt = client
        .prepare(
            "SELECT id, interaction_id, match_result_id, match_run_id, engine_version, config_version, project_id, talent_id, feedback_type, ng_reason_category, comment, actor, source, is_revoked, created_at FROM ses.feedback_events WHERE (interaction_id IS NOT NULL AND interaction_id = ANY($1::bigint[])) OR (match_result_id IS NOT NULL AND match_result_id = ANY($2::int[])) ORDER BY created_at DESC",
        )
        .await?;

    let rows = client
        .query(&stmt, &[&interaction_ids, &match_result_ids])
        .await?;

    let events = rows
        .into_iter()
        .map(|row| FeedbackEventRow {
            id: row.get::<_, i64>("id"),
            interaction_id: row.get::<_, Option<i64>>("interaction_id"),
            match_result_id: row
                .get::<_, Option<i32>>("match_result_id")
                .map(|v| v as i64),
            match_run_id: row.get("match_run_id"),
            engine_version: row.get("engine_version"),
            config_version: row.get("config_version"),
            project_id: row.get::<_, i64>("project_id"),
            talent_id: row.get::<_, i64>("talent_id"),
            feedback_type: row.get("feedback_type"),
            ng_reason_category: row.get("ng_reason_category"),
            comment: row.get("comment"),
            actor: row.get("actor"),
            source: row.get("source"),
            is_revoked: row.get("is_revoked"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(events)
}

fn latest_interactions_by_match(
    interactions: Vec<InteractionLogRow>,
) -> HashMap<i64, InteractionLogRow> {
    let mut map: HashMap<i64, InteractionLogRow> = HashMap::new();
    for interaction in interactions {
        if let Some(match_id) = interaction.match_result_id {
            let should_replace = match map.get(&match_id) {
                Some(existing) => interaction.created_at > existing.created_at,
                None => true,
            };
            if should_replace {
                map.insert(match_id, interaction);
            }
        }
    }
    map
}

fn group_feedback_events(
    events: Vec<FeedbackEventRow>,
) -> (
    HashMap<i64, Vec<FeedbackEventRow>>,
    HashMap<i64, Vec<FeedbackEventRow>>,
) {
    let mut by_interaction: HashMap<i64, Vec<FeedbackEventRow>> = HashMap::new();
    let mut by_match: HashMap<i64, Vec<FeedbackEventRow>> = HashMap::new();

    for event in events {
        if let Some(interaction_id) = event.interaction_id {
            by_interaction
                .entry(interaction_id)
                .or_default()
                .push(event.clone());
        }

        if let Some(match_id) = event.match_result_id {
            by_match.entry(match_id).or_default().push(event);
        }
    }

    (by_interaction, by_match)
}

pub async fn get_job_detail_with_includes(
    pool: &PgPool,
    id: i64,
    mut includes: JobDetailIncludes,
    allow_source_text: bool,
) -> Result<Option<QueueJobDetailResponse>, QueueStorageError> {
    if includes.include_interactions || includes.include_feedback {
        includes.include_matches = true;
    }

    let client = pool.get().await?;

    let row = client
        .query_opt(
            "SELECT id, message_id, status, priority, retry_count, next_retry_at, final_method, requires_manual_review, manual_review_reason, decision_reason, created_at, updated_at, partial_fields, last_error, llm_latency_ms, processing_started_at, completed_at FROM ses.extraction_queue WHERE id = $1",
            &[&id],
        )
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let mut detail = row_to_job_detail_response(&row);
    let message_id = detail.job.message_id.clone();

    let include_source = includes.include_source_text && allow_source_text;
    let talent_snapshot =
        if includes.include_entity || includes.include_matches || includes.include_source_text {
            fetch_talent_snapshot(&client, &message_id, include_source).await?
        } else {
            None
        };
    let project_snapshot =
        if includes.include_entity || includes.include_matches || includes.include_source_text {
            fetch_project_snapshot(&client, &message_id, include_source).await?
        } else {
            None
        };

    if includes.include_entity {
        detail.entity = match (talent_snapshot.clone(), project_snapshot.clone()) {
            (Some(talent), Some(project)) => Some(JobEntity::Both { talent, project }),
            (Some(talent), None) => Some(JobEntity::Talent(talent)),
            (None, Some(project)) => Some(JobEntity::Project(project)),
            (None, None) => None,
        };
    }

    if include_source {
        detail.source_preview = talent_snapshot
            .as_ref()
            .and_then(|t| t.source_text.clone())
            .or_else(|| {
                project_snapshot
                    .as_ref()
                    .and_then(|p| p.source_text.clone())
            })
            .map(|text| truncate_source_preview(&text));
    }

    if includes.include_matches {
        let matches = fetch_match_results(
            &client,
            talent_snapshot.as_ref().map(|t| t.id),
            project_snapshot.as_ref().map(|p| p.project_code),
            includes.days,
            includes.limit,
        )
        .await?;

        let match_ids: Vec<i64> = matches.iter().map(|m| m.id).collect();
        let match_ids_i32: Vec<i32> = match_ids.iter().map(|id| *id as i32).collect();
        let mut pairs = Vec::new();

        let interaction_map = if includes.include_interactions || includes.include_feedback {
            let interactions = fetch_interactions(&client, &match_ids_i32).await?;
            latest_interactions_by_match(interactions)
        } else {
            HashMap::new()
        };

        let feedback_maps = if includes.include_feedback {
            let interaction_ids: Vec<i64> = interaction_map.values().map(|i| i.id).collect();
            let events = fetch_feedback_events(&client, &interaction_ids, &match_ids_i32).await?;
            group_feedback_events(events)
        } else {
            (HashMap::new(), HashMap::new())
        };

        for match_result in matches {
            let latest_interaction = interaction_map.get(&match_result.id).cloned();
            let mut feedback_events = Vec::new();

            if let Some(interaction) = &latest_interaction {
                if let Some(events) = feedback_maps.0.get(&interaction.id) {
                    feedback_events.extend(events.clone());
                }
            }

            if feedback_events.is_empty() {
                if let Some(events) = feedback_maps.1.get(&match_result.id) {
                    feedback_events.extend(events.clone());
                }
            }

            pairs.push(PairDetail {
                match_result,
                latest_interaction,
                feedback_events,
            });
        }

        detail.pairs = Some(pairs);
    }

    Ok(Some(detail))
}

#[instrument(skip(pool))]
pub async fn retry_job(pool: &PgPool, id: i64) -> Result<(), QueueStorageError> {
    let client = pool.get().await?;

    let updated = client
        .execute(
            "UPDATE ses.extraction_queue SET status = 'pending', locked_by = NULL, processing_started_at = NULL, completed_at = NULL, next_retry_at = NULL, retry_count = 0, requires_manual_review = false, manual_review_reason = NULL, updated_at = NOW() WHERE id = $1 AND status = 'completed'",
            &[&id],
        )
        .await?;

    if updated == 1 {
        return Ok(());
    }

    let status_row = client
        .query_opt(
            "SELECT status FROM ses.extraction_queue WHERE id = $1",
            &[&id],
        )
        .await?;

    match status_row {
        None => Err(QueueStorageError::NotFound(format!("job {id} not found"))),
        Some(row) => {
            let status: String = row.get("status");
            Err(QueueStorageError::Conflict(format!(
                "job {id} is {status} and cannot be retried"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{FinalMethod, RecommendedMethod};

    fn sample_job() -> ExtractionJob {
        let mut job = ExtractionJob::new("m1", "s", Utc::now(), "hash");
        job.status = QueueStatus::Processing;
        job.retry_count = 2;
        job.locked_by = Some("worker".into());
        job.next_retry_at = Some(Utc::now());
        job.last_error = Some("oops".into());
        job.recommended_method = Some(RecommendedMethod::RustRecommended);
        job.final_method = Some(FinalMethod::RustCompleted);
        job.processing_started_at = Some(Utc::now());
        job.completed_at = Some(Utc::now());
        job.llm_latency_ms = Some(123);
        job.reprocess_after = Some(Utc::now());
        job.partial_fields = Some(serde_json::json!({"k":"v"}));
        job.requires_manual_review = true;
        job.manual_review_reason = Some("check".into());
        job
    }

    #[test]
    fn pending_copy_resets_runtime_fields() {
        let job = sample_job();
        let received = Utc::now();

        let pending = pending_copy(&job, received);
        assert_eq!(pending.status, QueueStatus::Pending);
        assert_eq!(pending.retry_count, 0);
        assert!(pending.locked_by.is_none());
        assert!(pending.next_retry_at.is_none());
        assert!(pending.last_error.is_none());
        assert!(pending.final_method.is_none());
        assert!(pending.processing_started_at.is_none());
        assert!(pending.completed_at.is_none());
        assert!(pending.llm_latency_ms.is_none());
        assert!(pending.reprocess_after.is_none());
        assert_eq!(pending.email_received_at, received);
        assert_eq!(pending.manual_review_reason, Some("check".into()));
        assert_eq!(pending.requires_manual_review, true);
        assert_eq!(
            pending.recommended_method,
            Some(RecommendedMethod::RustRecommended)
        );
        assert!(pending.updated_at >= job.updated_at);
    }

    #[test]
    fn normalize_json_handles_none_and_some() {
        let with_none: Option<Value> = None;
        assert!(normalize_json(&with_none).is_none());

        let with_value = Some(serde_json::json!({"a": 1}));
        let normalized = normalize_json(&with_value);
        assert!(normalized.is_some());
    }

    #[test]
    fn parse_status_rejects_unknown_values() {
        assert!(parse_status("pending").is_ok());
        assert!(parse_status("processing").is_ok());
        assert!(parse_status("completed").is_ok());
        let err = parse_status("broken").unwrap_err();
        assert!(format!("{err}").contains("unknown status"));
    }
}
