use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// キュー状態（3状態のみ: failed は廃止）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueStatus {
    Pending,
    Processing,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedMethod {
    RustRecommended,
    LlmRecommended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalMethod {
    RustCompleted,
    LlmCompleted,
    ManualReview,
}

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueueStatus::Pending => "pending",
            QueueStatus::Processing => "processing",
            QueueStatus::Completed => "completed",
        }
    }
}

impl RecommendedMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecommendedMethod::RustRecommended => "rust_recommended",
            RecommendedMethod::LlmRecommended => "llm_recommended",
        }
    }
}

impl FinalMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            FinalMethod::RustCompleted => "rust_completed",
            FinalMethod::LlmCompleted => "llm_completed",
            FinalMethod::ManualReview => "manual_review",
        }
    }
}

/// DDL-1 `ses.extraction_queue` に対応する簡易モデル
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionJob {
    pub id: u64,
    pub message_id: String,
    pub email_subject: String,
    pub email_received_at: DateTime<Utc>,
    pub subject_hash: String,
    pub status: QueueStatus,
    pub priority: i32,
    pub locked_by: Option<String>,
    pub retry_count: u32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub partial_fields: Option<Value>,
    pub decision_reason: Option<String>,
    pub recommended_method: Option<RecommendedMethod>,
    pub final_method: Option<FinalMethod>,
    pub extractor_version: Option<String>,
    pub rule_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub llm_latency_ms: Option<i32>,
    pub requires_manual_review: bool,
    pub canary_target: bool,
}

impl ExtractionJob {
    pub fn new(
        message_id: &str,
        email_subject: &str,
        email_received_at: DateTime<Utc>,
        subject_hash: &str,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            message_id: message_id.to_string(),
            email_subject: email_subject.to_string(),
            email_received_at,
            subject_hash: subject_hash.to_string(),
            status: QueueStatus::Pending,
            priority: 50,
            locked_by: None,
            retry_count: 0,
            next_retry_at: None,
            last_error: None,
            partial_fields: None,
            decision_reason: None,
            recommended_method: None,
            final_method: None,
            extractor_version: None,
            rule_version: None,
            created_at: now,
            processing_started_at: None,
            completed_at: None,
            updated_at: now,
            llm_latency_ms: None,
            requires_manual_review: false,
            canary_target: false,
        }
    }
}

pub enum JobError {
    Retryable {
        message: String,
        retry_after: Option<Duration>,
    },
    Permanent {
        message: String,
    },
}

pub struct JobOutcome {
    pub final_method: FinalMethod,
    pub partial_fields: Option<Value>,
    pub decision_reason: Option<String>,
    pub llm_latency_ms: Option<i32>,
    pub requires_manual_review: bool,
}

/// シンプルなインメモリ extraction_queue worker
#[derive(Default)]
pub struct ExtractionQueue {
    pub jobs: Vec<ExtractionJob>,
    next_id: u64,
}

impl ExtractionQueue {
    pub fn enqueue(&mut self, mut job: ExtractionJob) {
        if self
            .jobs
            .iter()
            .any(|existing| existing.message_id == job.message_id)
        {
            return;
        }
        self.next_id += 1;
        job.id = self.next_id;
        self.jobs.push(job);
    }

    fn poll_next(&mut self, now: DateTime<Utc>) -> Option<usize> {
        self.jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| {
                job.status == QueueStatus::Pending
                    && job.next_retry_at.map(|ts| ts <= now).unwrap_or(true)
            })
            .max_by(|(_, a), (_, b)| {
                a.priority
                    .cmp(&b.priority)
                    .then(std::cmp::Reverse(a.id).cmp(&std::cmp::Reverse(b.id)))
            })
            .map(|(idx, _)| idx)
    }

    pub fn process_next<F>(&mut self, handler: F) -> Option<QueueStatus>
    where
        F: Fn(&ExtractionJob) -> Result<JobOutcome, JobError>,
    {
        let now = Utc::now();
        let idx = self.poll_next(now)?;
        let mut job = self.jobs[idx].clone();
        job.status = QueueStatus::Processing;
        job.processing_started_at = Some(now);
        job.updated_at = now;

        match handler(&job) {
            Ok(outcome) => {
                job.status = QueueStatus::Completed;
                job.final_method = Some(outcome.final_method);
                job.partial_fields = outcome.partial_fields;
                job.decision_reason = outcome.decision_reason;
                job.llm_latency_ms = outcome.llm_latency_ms;
                job.completed_at = Some(now);
                job.updated_at = now;
                job.requires_manual_review = outcome.requires_manual_review;
            }
            Err(JobError::Permanent { message }) => {
                job.status = QueueStatus::Completed;
                job.final_method = Some(FinalMethod::ManualReview);
                job.last_error = Some(message.clone());
                job.decision_reason = Some(message);
                job.completed_at = Some(now);
                job.updated_at = now;
                job.requires_manual_review = true;
            }
            Err(JobError::Retryable {
                message,
                retry_after,
            }) => {
                job.status = QueueStatus::Pending;
                job.retry_count += 1;
                job.next_retry_at = Some(now + retry_after.unwrap_or_else(|| Duration::minutes(5)));
                job.last_error = Some(message);
                job.updated_at = now;
            }
        }

        self.jobs[idx] = job;
        Some(self.jobs[idx].status.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_job() -> ExtractionJob {
        ExtractionJob::new("msg-1", "subject", Utc::now(), "deadbeef")
    }

    #[test]
    fn transitions_pending_processing_completed() {
        let mut queue = ExtractionQueue::default();
        queue.enqueue(sample_job());

        let status = queue.process_next(|_| {
            Ok(JobOutcome {
                final_method: FinalMethod::RustCompleted,
                partial_fields: None,
                decision_reason: Some("ok".into()),
                llm_latency_ms: Some(1200),
                requires_manual_review: false,
            })
        });

        assert_eq!(status, Some(QueueStatus::Completed));
        let job = queue.jobs.first().unwrap();
        assert_eq!(job.status, QueueStatus::Completed);
        assert_eq!(job.retry_count, 0);
        assert_eq!(job.final_method, Some(FinalMethod::RustCompleted));
    }

    #[test]
    fn retryable_error_returns_to_pending() {
        let mut queue = ExtractionQueue::default();
        queue.enqueue(sample_job());

        let status = queue.process_next(|_| {
            Err(JobError::Retryable {
                message: "temp".into(),
                retry_after: Some(Duration::minutes(1)),
            })
        });

        assert_eq!(status, Some(QueueStatus::Pending));
        let job = queue.jobs.first().unwrap();
        assert_eq!(job.status, QueueStatus::Pending);
        assert_eq!(job.retry_count, 1);
        assert!(job.next_retry_at.is_some());
    }

    #[test]
    fn permanent_error_marks_completed_manual_review() {
        let mut queue = ExtractionQueue::default();
        queue.enqueue(sample_job());

        let status = queue.process_next(|_| {
            Err(JobError::Permanent {
                message: "bad request".into(),
            })
        });

        assert_eq!(status, Some(QueueStatus::Completed));
        let job = queue.jobs.first().unwrap();
        assert_eq!(job.status, QueueStatus::Completed);
        assert_eq!(job.final_method, Some(FinalMethod::ManualReview));
        assert!(job.requires_manual_review);
        assert!(job.decision_reason.is_some());
    }
}
