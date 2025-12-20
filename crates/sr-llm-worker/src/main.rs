use chrono::Utc;
use serde_json::json;
use sr_common::queue::{
    ExtractionJob, ExtractionQueue, FinalMethod, JobError, JobOutcome, RecommendedMethod,
};

pub fn run_sample_flow() -> ExtractionQueue {
    let mut queue = ExtractionQueue::default();

    let mut job = ExtractionJob::new(
        "llm-message-1",
        "llm subject",
        Utc::now(),
        "llm-subject-hash",
    );
    job.recommended_method = Some(RecommendedMethod::LlmRecommended);

    queue.enqueue(job);

    queue.process_next_with_worker("sr-llm-worker", |job| {
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

fn main() {
    let queue = run_sample_flow();

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
