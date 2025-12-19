use chrono::{NaiveDate, NaiveDateTime};

/// received_at を解決する
///
/// extraction_queue から取得した email_received_at を使用。
/// None の場合は抽出を中断（相対日付の解釈が不可能）
pub fn resolve_received_at(
    email_received_at: Option<NaiveDateTime>,
    fallback_received_at: Option<NaiveDateTime>,
) -> Result<NaiveDate, ReceivedAtError> {
    if let Some(dt) = email_received_at {
        return Ok(dt.date());
    }

    if let Some(dt) = fallback_received_at {
        return Ok(dt.date());
    }

    Err(ReceivedAtError::MissingReceivedAt)
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ReceivedAtError {
    #[error("email_received_at is missing - cannot interpret relative dates")]
    MissingReceivedAt,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_received_at_when_present() {
        let dt = NaiveDate::from_ymd_opt(2025, 1, 15)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let resolved = resolve_received_at(Some(dt), None).unwrap();
        assert_eq!(resolved, NaiveDate::from_ymd_opt(2025, 1, 15).unwrap());
    }

    #[test]
    fn missing_received_at_errors() {
        let err = resolve_received_at(None, None).unwrap_err();
        assert_eq!(err, ReceivedAtError::MissingReceivedAt);
    }

    #[test]
    fn fallback_received_at_is_used_when_email_missing() {
        let fallback = NaiveDate::from_ymd_opt(2024, 12, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();

        let resolved = resolve_received_at(None, Some(fallback)).unwrap();
        assert_eq!(resolved, NaiveDate::from_ymd_opt(2024, 12, 1).unwrap());
    }

    #[test]
    fn email_received_at_has_priority_over_fallback() {
        let email = NaiveDate::from_ymd_opt(2025, 2, 10)
            .unwrap()
            .and_hms_opt(15, 0, 0)
            .unwrap();
        let fallback = NaiveDate::from_ymd_opt(2024, 12, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();

        let resolved = resolve_received_at(Some(email), Some(fallback)).unwrap();
        assert_eq!(resolved, NaiveDate::from_ymd_opt(2025, 2, 10).unwrap());
    }
}
