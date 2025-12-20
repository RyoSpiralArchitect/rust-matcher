use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use serde_json::json;
use sr_common::db::{
    MatchResultInsert, create_pool_from_url, insert_match_result, lock_next_pending_job,
    upsert_extraction_job,
};
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobError, JobOutcome, QueueStatus,
    RecommendedMethod,
};
use tracing::info;

#[derive(Debug, Parser)]
#[command(
    name = "sr-llm-worker",
    about = "Process LLM recommended extraction jobs"
)]
struct Cli {
    /// PostgreSQL connection string
    #[arg(long, env = "DATABASE_URL")]
    db_url: String,

    /// Worker id recorded into the queue
    #[arg(long, default_value = "sr-llm-worker")]
    worker_id: String,

    /// Optional cap on how many jobs to process in one run (default: until queue is empty)
    #[arg(long)]
    max_jobs: Option<usize>,
}

pub fn run_sample_flow_with_worker(worker_id: &str) -> ExtractionQueue {
    let mut queue = ExtractionQueue::default();

    let mut job = ExtractionJob::new(
        "llm-message-1",
        "llm subject",
        Utc::now(),
        "llm-subject-hash",
    );
    job.recommended_method = Some(RecommendedMethod::LlmRecommended);

    queue.enqueue(job);

    queue.process_next_with_worker(worker_id, |job| {
        if job.recommended_method == Some(RecommendedMethod::LlmRecommended) {
            let partial = json!({
                "message_id": job.message_id,
                "llm": "stubbed",
            });
            Ok(JobOutcome {
                final_method: FinalMethod::LlmCompleted,
                partial_fields: Some(partial),
                decision_reason: Some("processed by sr-llm-worker".into()),
                llm_latency_ms: Some(1500),
                requires_manual_review: false,
                manual_review_reason: None,
            })
        } else {
            Err(JobError::Permanent {
                message: "non-llm job routed to sr-llm-worker".into(),
            })
        }
    });

    queue
}

pub fn run_sample_flow() -> ExtractionQueue {
    run_sample_flow_with_worker("sr-llm-worker")
}

fn handle_llm_job(job: &ExtractionJob) -> Result<JobOutcome, JobError> {
    if job.recommended_method == Some(RecommendedMethod::LlmRecommended) {
        let partial = json!({
            "message_id": job.message_id,
            "llm": "stubbed",
        });
        Ok(JobOutcome {
            final_method: FinalMethod::LlmCompleted,
            partial_fields: Some(partial),
            decision_reason: Some("processed by sr-llm-worker".into()),
            llm_latency_ms: Some(1500),
            requires_manual_review: false,
            manual_review_reason: None,
        })
    } else {
        Err(JobError::Permanent {
            message: "non-llm job routed to sr-llm-worker".into(),
        })
    }
}

fn apply_outcome(
    mut job: ExtractionJob,
    outcome: Result<JobOutcome, JobError>,
) -> (ExtractionJob, QueueStatus) {
    match outcome {
        Ok(outcome) => {
            let finished_at = Utc::now();
            job.status = QueueStatus::Completed;
            job.final_method = Some(outcome.final_method);
            job.partial_fields = outcome.partial_fields;
            job.decision_reason = outcome.decision_reason;
            job.manual_review_reason = outcome.manual_review_reason;
            job.llm_latency_ms = outcome.llm_latency_ms;
            job.completed_at = Some(finished_at);
            job.updated_at = finished_at;
            job.requires_manual_review = outcome.requires_manual_review;
            job.locked_by = None;
        }
        Err(JobError::Permanent { message }) => {
            let finished_at = Utc::now();
            job.status = QueueStatus::Completed;
            job.final_method = Some(FinalMethod::ManualReview);
            job.last_error = Some(message.clone());
            job.decision_reason = Some(message.clone());
            job.manual_review_reason = Some(message);
            job.completed_at = Some(finished_at);
            job.updated_at = finished_at;
            job.requires_manual_review = true;
            job.locked_by = None;
        }
        Err(JobError::Retryable {
            message,
            retry_after,
        }) => {
            let finished_at = Utc::now();
            job.status = QueueStatus::Pending;
            job.retry_count += 1;
            job.next_retry_at =
                Some(finished_at + retry_after.unwrap_or_else(|| chrono::Duration::minutes(5)));
            job.last_error = Some(message);
            job.updated_at = finished_at;
            job.locked_by = None;
        }
    }

    let status = job.status.clone();
    (job, status)
}

async fn process_locked_job(
    pool: &sr_common::db::PgPool,
    worker_id: &str,
    locked: ExtractionJob,
) -> Result<(), Box<dyn std::error::Error>> {
    let (processed, _status) = apply_outcome(locked.clone(), handle_llm_job(&locked));
    let rows = upsert_extraction_job(pool, &processed).await?;
    info!(rows, message_id = %processed.message_id, "persisted processed job");

    // Stubbed persistence of a match result snapshot for the processed job.
    let result = MatchResultInsert {
        talent_id: 1,
        project_id: 1,
        is_knockout: false,
        needs_manual_review: processed.requires_manual_review,
        score_total: Some(0.75),
        score_breakdown: Some(json!({
            "tanka": 0.8,
            "skills": 0.7,
        })),
        engine_version: processed.extractor_version.clone(),
        rule_version: processed.rule_version.clone(),
        ..Default::default()
    };

    let inserted = insert_match_result(pool, &result).await?;
    info!(
        rows = inserted,
        worker_id = %worker_id,
        message_id = %processed.message_id,
        "stub: inserted match_results snapshot"
    );

    Ok(())
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
        worker_id = %args.worker_id,
        "created postgres connection pool for llm worker",
    );

    let mut processed_jobs = 0usize;
    let max_jobs = args.max_jobs.unwrap_or(usize::MAX);

    while processed_jobs < max_jobs {
        let maybe_job = lock_next_pending_job(&pool, &args.worker_id, Utc::now()).await?;

        let Some(job) = maybe_job else {
            if processed_jobs == 0 {
                info!("no pending jobs found; exiting");
            }
            break;
        };

        process_locked_job(&pool, &args.worker_id, job).await?;
        processed_jobs += 1;
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("sr-llm-worker failed: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_job_is_marked_completed() {
        let queue = run_sample_flow();

        assert_eq!(queue.jobs.len(), 1);
        let job = &queue.jobs[0];
        assert_eq!(job.final_method, Some(FinalMethod::LlmCompleted));
        assert_eq!(job.status.as_str(), "completed");
        assert!(!job.requires_manual_review);
        assert!(
            job.decision_reason
                .as_ref()
                .map(|r| r.contains("sr-llm-worker"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn non_llm_jobs_are_sent_to_manual_review() {
        let mut queue = ExtractionQueue::default();
        let mut job = ExtractionJob::new("m2", "subject", Utc::now(), "hash");
        job.recommended_method = Some(RecommendedMethod::RustRecommended);

        queue.enqueue(job);

        queue.process_next_with_worker("sr-llm-worker", |j| handle_llm_job(j));

        let job = &queue.jobs[0];
        assert_eq!(job.status, QueueStatus::Completed);
        assert_eq!(job.final_method, Some(FinalMethod::ManualReview));
        assert!(job.requires_manual_review);
        assert!(job.manual_review_reason.is_some());
        assert!(job.locked_by.is_none());
    }
}
