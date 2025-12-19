use chrono::{Duration, Utc};
use sr_common::queue::{ExtractionJob, ExtractionQueue, QueueStatus};

fn recover(queue: &mut ExtractionQueue) {
    let now = Utc::now();
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

    recover(&mut queue);

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
