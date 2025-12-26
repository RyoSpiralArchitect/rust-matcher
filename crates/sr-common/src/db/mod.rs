pub mod anken_emails;
pub mod candidates;
pub mod conversion;
pub mod extraction_queue;
pub mod feedback;
pub mod feedback_history;
pub mod interaction_events;
pub mod interaction_logs;
pub mod match_results;
pub mod migrations;
pub mod pool;
pub mod queue_dashboard;

// Keep re-exports unique so downstream crates see a single symbol per helper.
pub use anken_emails::{fetch_email_body, fetch_pending_emails, PendingEmail, PendingEmailError};
pub use candidates::{fetch_candidates_for_project, fetch_match_by_id, MatchFetchError};
pub use conversion::{insert_conversion_event, ConversionStorageError};
pub use extraction_queue::{
    get_job_by_id, get_job_detail_with_includes, list_jobs, lock_next_pending_job, pending_copy,
    recover_stuck_jobs, retry_job, upsert_extraction_job, QueueStorageError,
};
pub use feedback::{insert_feedback_event, FeedbackStorageError};
pub use feedback_history::{fetch_feedback_history, FeedbackHistoryError};
pub use interaction_events::{insert_interaction_event, InteractionEventStorageError};
pub use interaction_logs::{
    insert_interaction_log, InteractionLogInsert, InteractionLogStorageError,
};
pub use match_results::{insert_match_result, MatchResultInsert, MatchResultStorageError};
pub use migrations::{run_migrations, MigrationError};
pub use pool::{create_pool_from_url, create_pool_from_url_checked, DbPoolError, PgPool};
pub use queue_dashboard::{fetch_dashboard, QueueDashboardError};
