use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use serde_json::json;
use sr_common::db::{
    MatchResultInsert, create_pool_from_url, insert_match_result, upsert_extraction_job,
};
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobError, JobOutcome, RecommendedMethod,
};
use tracing::{debug, info};

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

    let queue = run_sample_flow_with_worker(&args.worker_id);
    debug!(jobs = queue.jobs.len(), "ran in-memory llm worker demo");

    info!(
        worker_id = %args.worker_id,
        "stub: talents_enum upsert pending; demo inserts match_results snapshot alongside queue completion",
    );

    if let Some(job) = queue.jobs.first() {
        let rows = upsert_extraction_job(&pool, job).await?;
        info!(rows, message_id = %job.message_id, "persisted completed job");

        // Stubbed persistence of a match result snapshot for the processed job.
        let result = MatchResultInsert {
            talent_id: 1,
            project_id: 1,
            is_knockout: false,
            needs_manual_review: job.requires_manual_review,
            score_total: Some(0.75),
            score_breakdown: Some(json!({
                "tanka": 0.8,
                "skills": 0.7,
            })),
            engine_version: job.extractor_version.clone(),
            rule_version: job.rule_version.clone(),
            ..Default::default()
        };

        let inserted = insert_match_result(&pool, &result).await?;
        info!(rows = inserted, "stub: inserted match_results snapshot");
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
}
