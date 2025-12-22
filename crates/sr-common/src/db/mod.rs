pub mod anken_emails;
pub mod candidates;
pub mod extraction_queue;
pub mod feedback;
pub mod match_results;
pub mod pool;
pub mod queue_dashboard;

// Keep re-exports unique so downstream crates see a single symbol per helper.
pub use anken_emails::{PendingEmail, PendingEmailError, fetch_email_body, fetch_pending_emails};
pub use candidates::{CandidateFetchError, fetch_candidates_for_project};
pub use extraction_queue::{
    QueueStorageError, get_job_by_id, get_job_detail_with_includes, list_jobs,
    lock_next_pending_job, pending_copy, recover_stuck_jobs, retry_job, upsert_extraction_job,
};
pub use feedback::{FeedbackStorageError, insert_feedback_event};
pub use match_results::{MatchResultInsert, MatchResultStorageError, insert_match_result};
pub use pool::{DbPoolError, PgPool, create_pool_from_url};
pub use queue_dashboard::{QueueDashboardError, fetch_dashboard};
