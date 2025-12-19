use chrono::Utc;
use sr_common::extraction::{
    calculate_priority, evaluate_quality, extract_flow_dept, extract_remote_onsite,
    extract_start_date_raw, extract_tanka, extract_work_todofuken, PartialFields,
};
use sr_common::queue::{ExtractionJob, ExtractionQueue, FinalMethod, JobOutcome, RecommendedMethod};

const RULE_VERSION: &str = "2025-01-15-r1";

fn main() {
    let mut queue = ExtractionQueue::default();

    let body_text = r#"
【案件概要】
月額70〜90万円、即日から参画できるフルリモート案件です。
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

    let mut job = ExtractionJob::new(
        "message-1",
        "stub subject",
        Utc::now(),
        "subject-hash-stub",
    );
    job.recommended_method = Some(decision.recommended_method);
    job.decision_reason = Some(decision.reason.clone());
    job.priority = priority;
    job.extractor_version = Some(env!("CARGO_PKG_VERSION").into());
    job.rule_version = Some(RULE_VERSION.into());

    queue.enqueue(job);

    queue.process_next(|job| {
        let partial = serde_json::to_value(&partial).ok();

        Ok(JobOutcome {
            final_method: match job.recommended_method {
                Some(RecommendedMethod::RustRecommended) => FinalMethod::RustCompleted,
                _ => FinalMethod::ManualReview,
            },
            partial_fields: partial,
            decision_reason: job.decision_reason.clone(),
            llm_latency_ms: None,
            requires_manual_review: matches!(job.recommended_method, Some(RecommendedMethod::LlmRecommended)),
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
