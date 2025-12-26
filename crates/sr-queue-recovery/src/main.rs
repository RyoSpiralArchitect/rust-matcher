use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use dotenvy::dotenv;
use sr_common::db::{create_pool_from_url_checked, recover_stuck_jobs, run_migrations};
use sr_common::logging::install_tracing_panic_hook;
use sr_common::queue::{ExtractionJob, ExtractionQueue, QueueStatus};
use tracing::{debug, error, info};

const DEFAULT_STUCK_THRESHOLD_MINUTES: i64 = 10;

#[derive(Debug, Parser)]
#[command(
    name = "sr-queue-recovery",
    about = "Recover stuck extraction queue jobs"
)]
struct Cli {
    /// PostgreSQL connection string
    #[arg(long, env = "DATABASE_URL")]
    db_url: String,

    /// Threshold in minutes for considering a processing job stale
    #[arg(long, default_value_t = DEFAULT_STUCK_THRESHOLD_MINUTES)]
    stale_minutes: i64,
}

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

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    install_tracing_panic_hook(env!("CARGO_PKG_NAME"));

    let args = Cli::parse();
    let pool = create_pool_from_url_checked(&args.db_url).await?;
    run_migrations(&pool).await?;
    let status = pool.status();
    info!(
        size = status.size,
        available = status.available,
        stale_minutes = args.stale_minutes,
        "created postgres connection pool for queue recovery",
    );

    let mut queue = ExtractionQueue::default();
    let stuck_started_at = Utc::now() - Duration::minutes(args.stale_minutes + 1);
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
        Duration::minutes(args.stale_minutes),
    );

    debug!(jobs = queue.jobs.len(), "ran in-memory recovery demo");
    let updated =
        recover_stuck_jobs(&pool, Utc::now(), Duration::minutes(args.stale_minutes)).await?;
    info!(rows = updated, "attempted DB recovery for stuck jobs");

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        error!(error = %err, "sr-queue-recovery failed");
        std::process::exit(1);
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
