use chrono::Utc;
use serde_json::json;
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobError, JobOutcome, RecommendedMethod,
};

fn main() {
    let mut queue = ExtractionQueue::default();

    let mut job = ExtractionJob::new(
        "llm-message-1",
        "llm subject",
        Utc::now(),
        "llm-subject-hash",
    );
    job.recommended_method = Some(RecommendedMethod::LlmRecommended);

    queue.enqueue(job);

    queue.process_next(|job| {
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
            })
        } else {
            Err(JobError::Permanent {
                message: "non-llm job routed to sr-llm-worker".into(),
            })
        }
    });

    for job in queue.jobs.iter() {
        println!(
            "job {} status={} recommended={:?} final={:?} manual_review={}",
            job.id,
            job.status.as_str(),
            job.recommended_method,
            job.final_method,
            job.requires_manual_review,
        );
    }
}
