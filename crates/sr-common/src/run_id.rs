//! Process-level run ID for tracking execution instances.
//!
//! Each process gets a unique ULID at startup. All operations within
//! the same process share this ID, enabling:
//! - Idempotent inserts within a single run
//! - Separate records for different runs (even on the same day)
//! - Traceability of which run produced each record
//!
//! # Example
//! ```
//! use sr_common::run_id;
//!
//! // Get the process-level run ID (same value for entire process lifetime)
//! let id = run_id::get();
//! println!("Current run: {}", id);
//!
//! // Generate a fresh ULID for sub-tasks if needed
//! let sub_id = run_id::generate();
//! ```

use once_cell::sync::Lazy;
use ulid::Ulid;

/// Process-level run ID, generated once at first access.
static RUN_ID: Lazy<String> = Lazy::new(|| Ulid::new().to_string());

/// Returns the process-level run ID.
///
/// This ID is:
/// - Generated once per process (at first call)
/// - Time-ordered (ULIDs sort lexicographically by creation time)
/// - 26 characters, URL-safe
///
/// Use this for `match_run_id` in `match_results` and `interaction_logs`.
#[inline]
pub fn get() -> &'static str {
    &RUN_ID
}

/// Generates a fresh ULID.
///
/// Use this when you need a new unique ID for sub-operations
/// (e.g., request IDs, batch sub-runs).
#[inline]
pub fn generate() -> String {
    Ulid::new().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_same_value() {
        let first = get();
        let second = get();
        assert_eq!(first, second);
        assert_eq!(first.len(), 26); // ULID is 26 chars
    }

    #[test]
    fn generate_returns_unique_values() {
        let a = generate();
        let b = generate();
        assert_ne!(a, b);
        assert_eq!(a.len(), 26);
        assert_eq!(b.len(), 26);
    }

    #[test]
    fn ulid_is_lexicographically_sortable() {
        let older = generate();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let newer = generate();
        assert!(older < newer, "ULIDs should be time-ordered");
    }
}
