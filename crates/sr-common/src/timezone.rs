/// Canonical timezone used when deriving run_date/event_date columns.
///
/// Keeping this in a single constant avoids scattering string literals across
/// SQL definitions and application queries.
pub const RUN_DATE_TIMEZONE: &str = "Asia/Tokyo";
