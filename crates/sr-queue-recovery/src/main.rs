use chrono::{DateTime, Duration, Utc};
use sr_common::queue::{ExtractionJob, ExtractionQueue, QueueStatus};

fn recover(queue: &mut ExtractionQueue, now: DateTime<Utc>) {
    for job in queue.jobs.iter_mut() {
        if job.status == QueueStatus::Processing {
            job.status = QueueStatus::Pending;
            job.locked_by = None;
            job.next_retry_at = Some(now);
            job.updated_at = now;
        }
    }
}

fn main() {
    let mut queue = ExtractionQueue::default();

    let mut stuck = ExtractionJob::new(
        "stuck-message-1",
        "stuck subject",
        Utc::now() - Duration::minutes(20),
        "stuck-subject-hash",
    );
    stuck.status = QueueStatus::Processing;
    stuck.locked_by = Some("worker_stub".into());
    stuck.processing_started_at = Some(Utc::now() - Duration::minutes(15));
    queue.enqueue(stuck);

    recover(&mut queue, Utc::now());

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

    fn stuck_job() -> ExtractionJob {
        let mut job = ExtractionJob::new(
            "stuck-message-1",
            "stuck subject",
            Utc::now() - Duration::minutes(20),
            "stuck-subject-hash",
        );
        job.status = QueueStatus::Processing;
        job.locked_by = Some("worker_stub".into());
        job.processing_started_at = Some(Utc::now() - Duration::minutes(15));
        job.updated_at = Utc::now() - Duration::minutes(10);
        job
    }

    #[test]
    fn processing_jobs_return_to_pending_and_unlock() {
        let mut queue = ExtractionQueue::default();
        queue.enqueue(stuck_job());

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        recover(&mut queue, now);

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
        recover(&mut queue, now);

        let job = queue.jobs.first().unwrap();
        assert_eq!(job.status, QueueStatus::Pending);
        assert_eq!(job.next_retry_at, None);
        assert_eq!(job.locked_by, None);
        assert!(job.updated_at < now);
    }
}
