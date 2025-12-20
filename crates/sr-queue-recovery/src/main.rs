use chrono::{DateTime, Duration, Utc};
use sr_common::queue::{ExtractionJob, ExtractionQueue, QueueStatus};

const DEFAULT_STUCK_THRESHOLD_MINUTES: i64 = 10;

fn recover(queue: &mut ExtractionQueue, now: DateTime<Utc>, max_processing: Duration) {
    for job in queue.jobs.iter_mut() {
        if job.status != QueueStatus::Processing {
            continue;
        }

        let started_at = job.processing_started_at.unwrap_or(job.updated_at);
        if now.signed_duration_since(started_at) < max_processing {
            continue;
        }

        job.status = QueueStatus::Pending;
        job.locked_by = None;
        job.next_retry_at = Some(now);
        job.updated_at = now;
    }
}

fn main() {
    let mut queue = ExtractionQueue::default();

    let stuck_started_at = Utc::now() - Duration::minutes(DEFAULT_STUCK_THRESHOLD_MINUTES + 5);
    let mut stuck = ExtractionJob::new(
        "stuck-message-1",
        "stuck subject",
        stuck_started_at,
        "stuck-subject-hash",
    );
    stuck.status = QueueStatus::Processing;
    stuck.locked_by = Some("worker_stub".into());
    stuck.processing_started_at = Some(stuck_started_at);
    queue.enqueue(stuck);

    recover(
        &mut queue,
        Utc::now(),
        Duration::minutes(DEFAULT_STUCK_THRESHOLD_MINUTES),
    );

    for job in queue.jobs.iter() {
        println!(
            "job {} status={} locked_by={:?} next_retry_at={:?}",
            job.id,
            job.status.as_str(),
            job.locked_by,
            job.next_retry_at,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn stuck_job(started_at: DateTime<Utc>) -> ExtractionJob {
        let mut job = ExtractionJob::new(
            "stuck-message-1",
            "stuck subject",
            started_at,
            "stuck-subject-hash",
        );
        job.status = QueueStatus::Processing;
        job.locked_by = Some("worker_stub".into());
        job.processing_started_at = Some(started_at);
        job.updated_at = started_at;
        job
    }

    #[test]
    fn processing_jobs_return_to_pending_and_unlock() {
        let mut queue = ExtractionQueue::default();
        let started_at = Utc.with_ymd_and_hms(2023, 12, 31, 23, 40, 0).unwrap();
        queue.enqueue(stuck_job(started_at));

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        recover(
            &mut queue,
            now,
            Duration::minutes(DEFAULT_STUCK_THRESHOLD_MINUTES),
        );

        let job = queue.jobs.first().unwrap();
        assert_eq!(job.status, QueueStatus::Pending);
        assert_eq!(job.locked_by, None);
        assert_eq!(job.next_retry_at, Some(now));
        assert_eq!(job.updated_at, now);
        assert!(job.processing_started_at.is_some());
    }

    #[test]
    fn non_processing_jobs_are_left_untouched() {
        let mut queue = ExtractionQueue::default();
        let mut pending = ExtractionJob::new(
            "pending-message-1",
            "pending subject",
            Utc::now(),
            "pending-subject-hash",
        );
        pending.updated_at = Utc.with_ymd_and_hms(2023, 12, 31, 0, 0, 0).unwrap();
        queue.enqueue(pending);

        let now = Utc.with_ymd_and_hms(2024, 1, 2, 12, 0, 0).unwrap();
        recover(
            &mut queue,
            now,
            Duration::minutes(DEFAULT_STUCK_THRESHOLD_MINUTES),
        );

        let job = queue.jobs.first().unwrap();
        assert_eq!(job.status, QueueStatus::Pending);
        assert_eq!(job.next_retry_at, None);
        assert_eq!(job.locked_by, None);
        assert!(job.updated_at < now);
    }

    #[test]
    fn keeps_recent_processing_jobs_locked() {
        let mut queue = ExtractionQueue::default();
        let started_at = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        queue.enqueue(stuck_job(started_at));

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 5, 0).unwrap();
        recover(
            &mut queue,
            now,
            Duration::minutes(DEFAULT_STUCK_THRESHOLD_MINUTES),
        );

        let job = queue.jobs.first().unwrap();
        assert_eq!(job.status, QueueStatus::Processing);
        assert!(job.locked_by.is_some());
        assert_eq!(job.next_retry_at, None);
    }
}
