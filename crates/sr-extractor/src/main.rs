use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use serde_json::to_value;
use sr_common::db::{
    create_pool_from_url_checked, fetch_pending_emails, pending_copy, run_migrations,
    upsert_extraction_job, PendingEmail,
};
use sr_common::extraction::{
    calculate_priority, evaluate_quality, extract_all_fields, extract_flow_dept,
    extract_remote_onsite, extract_start_date_raw, extract_tanka, extract_work_todofuken,
    ExtractorOutput, PartialFields,
};
use sr_common::logging::{init_tracing_subscriber, install_tracing_panic_hook};
use sr_common::normalize::{calculate_content_hash, calculate_subject_hash, normalize_subject};
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobOutcome, RecommendedMethod,
};
use std::collections::HashSet;
use tokio::task::spawn_blocking;
use tracing::{debug, error, info};

#[derive(Debug, Parser)]
#[command(
    name = "sr-extractor",
    about = "Enqueue and pre-process extraction jobs"
)]
struct Cli {
    /// PostgreSQL connection string
    #[arg(long, env = "DATABASE_URL")]
    db_url: String,

    /// Skip DB writes and run the in-memory demonstration only
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

const RULE_VERSION: &str = "2025-01-15-r1";
const FETCH_LIMIT: i64 = 100;

fn dedup_emails_by_body(emails: Vec<PendingEmail>) -> (Vec<PendingEmail>, usize) {
    let mut seen_hashes = HashSet::new();
    let mut unique = Vec::new();
    let mut dropped = 0usize;

    for email in emails {
        let hash = calculate_content_hash(&email.body_text);
        if seen_hashes.insert(hash) {
            unique.push(email);
        } else {
            dropped += 1;
        }
    }

    (unique, dropped)
}

fn build_job_from_email(
    email_subject: &str,
    email_received_at: chrono::DateTime<Utc>,
    subject_hash: &str,
    extraction: &ExtractorOutput,
) -> ExtractionJob {
    let mut job = ExtractionJob::new("", email_subject, email_received_at, subject_hash);
    job.partial_fields = to_value(&extraction.partial).ok();
    job.priority = calculate_priority(&extraction.quality);
    job.recommended_method = Some(extraction.decision.recommended_method.clone());
    job.decision_reason = Some(extraction.decision.reason.clone());
    job.extractor_version = Some(env!("CARGO_PKG_VERSION").into());
    job.rule_version = Some(RULE_VERSION.into());
    job
}

pub fn run_sample_flow() -> ExtractionQueue {
    let mut queue = ExtractionQueue::default();

    let body_text = r#"
【案件概要】
月額70〜90万円、即日から参画できるフルリモート案件です（勤務地: 東京都）。
"#;

    let mut partial = PartialFields::default();
    if let Some((min, max)) = extract_tanka(body_text) {
        partial.monthly_tanka_min = Some(min);
        partial.monthly_tanka_max = Some(max);
    }
    partial.start_date_raw = extract_start_date_raw(body_text);
    partial.work_todofuken = extract_work_todofuken(body_text);
    partial.remote_onsite = extract_remote_onsite(body_text);
    partial.flow_dept = extract_flow_dept(body_text);
    partial.outcome_tag = Some("unknown".to_string());

    let (quality, decision) = evaluate_quality(&partial);
    let priority = calculate_priority(&quality);

    let subject = "stub subject";
    let mut job = ExtractionJob::new(
        "message-1",
        subject,
        Utc::now(),
        &calculate_subject_hash(subject),
    );
    job.email_subject = normalize_subject(subject);
    job.recommended_method = Some(decision.recommended_method);
    job.decision_reason = Some(decision.reason.clone());
    job.priority = priority;
    job.extractor_version = Some(env!("CARGO_PKG_VERSION").into());
    job.rule_version = Some(RULE_VERSION.into());

    queue.enqueue(job);

    queue.process_next_with_worker("sr-extractor", |job| {
        let partial = serde_json::to_value(&partial).ok();

        Ok(JobOutcome {
            final_method: match job.recommended_method {
                Some(RecommendedMethod::RustRecommended) => FinalMethod::RustCompleted,
                _ => FinalMethod::ManualReview,
            },
            partial_fields: partial,
            decision_reason: job.decision_reason.clone(),
            llm_latency_ms: None,
            requires_manual_review: matches!(
                job.recommended_method,
                Some(RecommendedMethod::LlmRecommended)
            ),
            manual_review_reason: job.decision_reason.clone(),
        })
    });

    queue
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    init_tracing_subscriber(env!("CARGO_PKG_NAME"));
    install_tracing_panic_hook(env!("CARGO_PKG_NAME"));

    let args = Cli::parse();
    let pool = create_pool_from_url_checked(&args.db_url).await?;

    run_migrations(&pool).await?;

    let status = pool.status();
    info!(
        size = status.size,
        available = status.available,
        "created postgres connection pool"
    );

    if args.dry_run {
        let queue = run_sample_flow();
        debug!(jobs = queue.jobs.len(), "ran in-memory extraction demo");
        info!("dry-run mode: skipping DB I/O and enqueue");
        return Ok(());
    }

    let (emails, deduped) = {
        let fetched = fetch_pending_emails(&pool, FETCH_LIMIT).await?;
        let (unique, dropped) = spawn_blocking(|| dedup_emails_by_body(fetched))
            .await
            .map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("deduplication task failed: {err}"),
                )
            })?;
        (unique, dropped)
    };
    info!(
        count = emails.len(),
        deduped, "fetched pending emails to enqueue"
    );

    for email in emails {
        let (normalized_subject, subject_hash, extraction) = spawn_blocking({
            let subject = email.subject.clone();
            let body_text = email.body_text.clone();
            move || {
                let normalized_subject = normalize_subject(&subject);
                let subject_hash = calculate_subject_hash(&subject);
                let extraction = extract_all_fields(&body_text, Some(&normalized_subject));
                (normalized_subject, subject_hash, extraction)
            }
        })
        .await
        .map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("extraction task failed: {err}"),
            )
        })?;
        let mut job = build_job_from_email(
            &normalized_subject,
            email.created_at,
            &subject_hash,
            &extraction,
        );
        job.message_id = email.message_id.clone();
        job.requires_manual_review =
            extraction.decision.recommended_method == RecommendedMethod::LlmRecommended;
        job.manual_review_reason = job.decision_reason.clone();

        let pending = pending_copy(&job, email.created_at);
        let rows = upsert_extraction_job(&pool, &pending).await?;
        info!(rows, message_id = %pending.message_id, "enqueued job into postgres");
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        error!(error = %err, "sr-extractor failed");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_flow_enqueues_and_processes_job() {
        let queue = run_sample_flow();

        assert_eq!(queue.jobs.len(), 1);
        let job = &queue.jobs[0];
        assert_eq!(job.status.as_str(), "completed");
        assert_eq!(job.final_method, Some(FinalMethod::RustCompleted));
        assert!(job.decision_reason.is_some());
        assert_eq!(
            job.recommended_method,
            Some(RecommendedMethod::RustRecommended)
        );
        assert!(job.email_subject.contains("stub"));
    }

    #[test]
    fn dedup_emails_by_body_removes_duplicate_payloads() {
        let now = Utc::now();
        let emails = vec![
            PendingEmail {
                message_id: "m1".into(),
                subject: "s1".into(),
                body_text: "duplicate body".into(),
                created_at: now,
            },
            PendingEmail {
                message_id: "m2".into(),
                subject: "s2".into(),
                body_text: "duplicate body".into(),
                created_at: now,
            },
            PendingEmail {
                message_id: "m3".into(),
                subject: "s3".into(),
                body_text: "unique body".into(),
                created_at: now,
            },
        ];

        let (unique, deduped) = dedup_emails_by_body(emails);
        assert_eq!(unique.len(), 2);
        assert_eq!(deduped, 1);
        assert!(unique.iter().any(|e| e.message_id == "m1"));
        assert!(unique.iter().any(|e| e.message_id == "m3"));
    }
}
