use chrono::{NaiveDate, NaiveDateTime};

/// received_at を解決する
///
/// extraction_queue から取得した email_received_at を使用。
/// None の場合は抽出を中断（相対日付の解釈が不可能）
pub fn resolve_received_at(
    email_received_at: Option<NaiveDateTime>,
) -> Result<NaiveDate, ReceivedAtError> {
    email_received_at
        .map(|dt| dt.date())
        .ok_or(ReceivedAtError::MissingReceivedAt)
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
        let resolved = resolve_received_at(Some(dt)).unwrap();
        assert_eq!(resolved, NaiveDate::from_ymd_opt(2025, 1, 15).unwrap());
    }

    #[test]
    fn missing_received_at_errors() {
        let err = resolve_received_at(None).unwrap_err();
        assert_eq!(err, ReceivedAtError::MissingReceivedAt);
    }
}
