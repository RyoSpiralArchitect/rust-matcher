use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use sr_common::db::{create_pool_from_url, pending_copy, upsert_extraction_job};
use sr_common::extraction::{
    PartialFields, calculate_priority, evaluate_quality, extract_flow_dept, extract_remote_onsite,
    extract_start_date_raw, extract_tanka, extract_work_todofuken,
};
use sr_common::normalize::{calculate_subject_hash, normalize_subject};
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobOutcome, RecommendedMethod,
};
use tracing::{debug, info};

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
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    let pool = create_pool_from_url(&args.db_url)?;

    let status = pool.status();
    info!(
        size = status.size,
        available = status.available,
        "created postgres connection pool"
    );

    let queue = run_sample_flow();
    debug!(jobs = queue.jobs.len(), "ran in-memory extraction demo");

    if args.dry_run {
        info!("dry-run mode: skipping DB I/O and enqueue");
        return Ok(());
    }

    if let Some(job) = queue.jobs.first() {
        let pending = pending_copy(job, job.email_received_at);
        let rows = upsert_extraction_job(&pool, &pending).await?;
        info!(rows, message_id = %pending.message_id, "enqueued job into postgres");
    } else {
        info!("no jobs were created by extractor flow");
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("sr-extractor failed: {err}");
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
}
