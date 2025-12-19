use chrono::{Datelike, NaiveDate, Utc, DateTime};
use lazy_static::lazy_static;
use regex::Regex;

/// 開始日の精度を表現
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartDatePrecision {
    /// 日付まで判明
    ExactDay,
    /// 月のみ（上旬/中旬/下旬を含む）
    ApproximateMonth,
    /// 即日・ASAP など「できるだけ早く」
    Asap,
}

/// 正規化済み開始日
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedStartDate {
    pub date: NaiveDate,
    pub precision: StartDatePrecision,
}

lazy_static! {
    static ref EXACT_DATE_RE: Regex = Regex::new(r"(\d{4})[/-](\d{1,2})[/-](\d{1,2})").unwrap();
    static ref MONTH_PART_RE: Regex = Regex::new(r"(\d{1,2})月(上旬|中旬|下旬)").unwrap();
    static ref MONTH_ONLY_RE: Regex = Regex::new(r"(\d{1,2})月").unwrap();
    static ref NEXT_MONTH_RE: Regex = Regex::new(r"来月").unwrap();
    static ref ASAP_RE: Regex = Regex::new(r"(?i)(即日|即時|ASAP)").unwrap();
}

/// 開始日テキストを受領日時を基準に正規化する
///
/// - 即日/ASAP: 基準日そのまま
/// - YYYY/MM/DD: その日付を ExactDay として返す
/// - "1月上旬" などの部分指定: 5/15/25 日に丸め、過去月なら翌年に繰り上げ
/// - "12月" など月のみ: 1日始まりで ApproximateMonth
/// - "来月": 翌月1日開始（年跨ぎ対応）
pub fn normalize_start_date(raw: &str, base_received_at: DateTime<Utc>) -> Option<NormalizedStartDate> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let base_date = base_received_at.date_naive();

    if ASAP_RE.is_match(trimmed) {
        return Some(NormalizedStartDate {
            date: base_date,
            precision: StartDatePrecision::Asap,
        });
    }

    if let Some(caps) = EXACT_DATE_RE.captures(trimmed) {
        let year: i32 = caps.get(1)?.as_str().parse().ok()?;
        let month: u32 = caps.get(2)?.as_str().parse().ok()?;
        let day: u32 = caps.get(3)?.as_str().parse().ok()?;
        let date = NaiveDate::from_ymd_opt(year, month, day)?;

        return Some(NormalizedStartDate {
            date,
            precision: StartDatePrecision::ExactDay,
        });
    }

    if NEXT_MONTH_RE.is_match(trimmed) {
        let next_month = if base_date.month() == 12 {
            NaiveDate::from_ymd_opt(base_date.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(base_date.year(), base_date.month() + 1, 1)
        }?;

        return Some(NormalizedStartDate {
            date: next_month,
            precision: StartDatePrecision::ApproximateMonth,
        });
    }

    if let Some(caps) = MONTH_PART_RE.captures(trimmed) {
        let month: u32 = caps.get(1)?.as_str().parse().ok()?;
        let bucket = caps.get(2)?.as_str();
        let day = match bucket {
            "上旬" => 5,
            "中旬" => 15,
            "下旬" => 25,
            _ => 1,
        };

        let mut year = base_date.year();
        if month < base_date.month() {
            year += 1;
        }

        let date = NaiveDate::from_ymd_opt(year, month, day)?;

        return Some(NormalizedStartDate {
            date,
            precision: StartDatePrecision::ApproximateMonth,
        });
    }

    if let Some(caps) = MONTH_ONLY_RE.captures(trimmed) {
        let month: u32 = caps.get(1)?.as_str().parse().ok()?;
        let mut year = base_date.year();
        if month < base_date.month() {
            year += 1;
        }

        let date = NaiveDate::from_ymd_opt(year, month, 1)?;

        return Some(NormalizedStartDate {
            date,
            precision: StartDatePrecision::ApproximateMonth,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn normalizes_asap_and_exact_dates() {
        let base = Utc.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap();

        let asap = normalize_start_date("即日", base).unwrap();
        assert_eq!(asap.date, base.date_naive());
        assert_eq!(asap.precision, StartDatePrecision::Asap);

        let exact = normalize_start_date("2025/02/15", base).unwrap();
        assert_eq!(exact.date, NaiveDate::from_ymd_opt(2025, 2, 15).unwrap());
        assert_eq!(exact.precision, StartDatePrecision::ExactDay);
    }

    #[test]
    fn normalizes_next_month_and_month_parts() {
        let base = Utc.with_ymd_and_hms(2025, 1, 28, 0, 0, 0).unwrap();

        let next_month = normalize_start_date("来月", base).unwrap();
        assert_eq!(next_month.date, NaiveDate::from_ymd_opt(2025, 2, 1).unwrap());
        assert_eq!(next_month.precision, StartDatePrecision::ApproximateMonth);

        let late = normalize_start_date("3月下旬", base).unwrap();
        assert_eq!(late.date, NaiveDate::from_ymd_opt(2025, 3, 25).unwrap());
        assert_eq!(late.precision, StartDatePrecision::ApproximateMonth);
    }

    #[test]
    fn rolls_months_into_next_year_when_needed() {
        let base = Utc.with_ymd_and_hms(2025, 12, 15, 0, 0, 0).unwrap();

        let next_month = normalize_start_date("来月", base).unwrap();
        assert_eq!(next_month.date, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());

        let november = normalize_start_date("11月", base).unwrap();
        assert_eq!(november.date, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
    }
}
