use chrono::Utc;
use serde_json::json;
use sr_common::queue::{ExtractionJob, ExtractionQueue, FinalMethod, JobOutcome, RecommendedMethod};

fn main() {
    let mut queue = ExtractionQueue::default();

    let mut job = ExtractionJob::new(
        "message-1",
        "stub subject",
        Utc::now(),
        "subject-hash-stub",
    );
    job.recommended_method = Some(RecommendedMethod::RustRecommended);
    job.extractor_version = Some("sr-extractor-stub".into());

    queue.enqueue(job);

    queue.process_next(|job| {
        let partial = json!({
            "message_id": job.message_id,
            "email_subject": job.email_subject,
        });

        Ok(JobOutcome {
            final_method: FinalMethod::RustCompleted,
            partial_fields: Some(partial),
            decision_reason: Some("extracted via sr-extractor stub".into()),
            llm_latency_ms: None,
            requires_manual_review: false,
        })
    });

    for job in queue.jobs.iter() {
        println!(
            "job {} status={} recommended={:?} final={:?}",
            job.id,
            job.status.as_str(),
            job.recommended_method,
            job.final_method
        );
    }
}
