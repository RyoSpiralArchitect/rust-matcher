use super::DatePrecision;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// 開始日の精度を表現
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartDatePrecision {
    /// 日付まで判明
    ExactDay,
    /// 月のみ（上旬/中旬/下旬を含む）
    ApproximateMonth,
    /// 即日・ASAP など「できるだけ早く」
    Asap,
    /// 四半期（年が省略された場合は基準年で補完）
    Quarter,
    /// 応相談など日時未確定
    Negotiable,
    /// 判別不能
    Unknown,
}

/// 正規化済み開始日
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedStartDate {
    pub date: Option<NaiveDate>,
    pub precision: StartDatePrecision,
    pub interpretation_note: Option<String>,
}

lazy_static! {
    static ref EXACT_DATE_RE: Regex = Regex::new(r"(\d{4})[/-](\d{1,2})[/-](\d{1,2})").unwrap();
    static ref MONTH_PART_RE: Regex = Regex::new(r"(\d{1,2})月(上旬|中旬|下旬)").unwrap();
    static ref MONTH_ONLY_RE: Regex = Regex::new(r"(\d{1,2})月").unwrap();
    static ref NEXT_MONTH_RE: Regex = Regex::new(r"来月").unwrap();
    static ref ASAP_RE: Regex = Regex::new(r"(?i)(即日|即時|ASAP)").unwrap();
    static ref QUARTER_RE: Regex =
        Regex::new(r"(?i)(?:(\d{4})\s*[-/]?\s*)?(?:q([1-4])|([1-4])q|第\s*([1-4])\s*四半期)")
            .unwrap();
    static ref NEGOTIABLE_RE: Regex =
        Regex::new(r"(?i)(応相談|要相談|調整(?:可|可能)|negotiable)").unwrap();
}

/// 開始日テキストを受領日時を基準に正規化する
///
/// - 即日/ASAP: 基準日そのまま
/// - YYYY/MM/DD: その日付を ExactDay として返す
/// - "1月上旬" などの部分指定: 5/15/25 日に丸め、過去月なら翌年に繰り上げ
/// - "12月" など月のみ: 1日始まりで ApproximateMonth
/// - "来月": 翌月1日開始（年跨ぎ対応）
/// - "2025Q2" / "第3四半期" など四半期指定: 四半期の初日を Quarter 精度で返す（年欠落時は基準年補完）
/// - "応相談" など未確定: Negotiable として date=None
pub fn normalize_start_date(
    raw: &str,
    base_received_at: DateTime<Utc>,
) -> Option<NormalizedStartDate> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let base_date = base_received_at.date_naive();

    if ASAP_RE.is_match(trimmed) {
        return Some(NormalizedStartDate {
            date: Some(base_date),
            precision: StartDatePrecision::Asap,
            interpretation_note: None,
        });
    }

    if let Some(caps) = EXACT_DATE_RE.captures(trimmed) {
        let year: i32 = caps.get(1)?.as_str().parse().ok()?;
        let month: u32 = caps.get(2)?.as_str().parse().ok()?;
        let day: u32 = caps.get(3)?.as_str().parse().ok()?;
        let date = NaiveDate::from_ymd_opt(year, month, day)?;

        return Some(NormalizedStartDate {
            date: Some(date),
            precision: StartDatePrecision::ExactDay,
            interpretation_note: None,
        });
    }

    if NEXT_MONTH_RE.is_match(trimmed) {
        let next_month = if base_date.month() == 12 {
            NaiveDate::from_ymd_opt(base_date.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(base_date.year(), base_date.month() + 1, 1)
        }?;

        return Some(NormalizedStartDate {
            date: Some(next_month),
            precision: StartDatePrecision::ApproximateMonth,
            interpretation_note: None,
        });
    }

    if let Some(caps) = QUARTER_RE.captures(trimmed) {
        let year = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or(base_date.year());

        let quarter_str = caps
            .get(2)
            .or_else(|| caps.get(3))
            .or_else(|| caps.get(4))
            .map(|m| m.as_str())?;
        let quarter: u32 = quarter_str.parse().ok()?;

        let month = match quarter {
            1 => 1,
            2 => 4,
            3 => 7,
            4 => 10,
            _ => return None,
        };

        let date = NaiveDate::from_ymd_opt(year, month, 1)?;
        let interpretation_note = caps
            .get(1)
            .is_none()
            .then(|| format!("year assumed from received_at {}", base_date));

        return Some(NormalizedStartDate {
            date: Some(date),
            precision: StartDatePrecision::Quarter,
            interpretation_note,
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
            date: Some(date),
            precision: StartDatePrecision::ApproximateMonth,
            interpretation_note: None,
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
            date: Some(date),
            precision: StartDatePrecision::ApproximateMonth,
            interpretation_note: None,
        });
    }

    if NEGOTIABLE_RE.is_match(trimmed) {
        return Some(NormalizedStartDate {
            date: None,
            precision: StartDatePrecision::Negotiable,
            interpretation_note: Some("start date negotiable/unspecified".into()),
        });
    }

    Some(NormalizedStartDate {
        date: None,
        precision: StartDatePrecision::Unknown,
        interpretation_note: Some("could not normalize start date".into()),
    })
}

impl From<StartDatePrecision> for DatePrecision {
    fn from(value: StartDatePrecision) -> Self {
        match value {
            StartDatePrecision::ExactDay => DatePrecision::Exact,
            StartDatePrecision::ApproximateMonth => DatePrecision::Month,
            StartDatePrecision::Asap => DatePrecision::Asap,
            StartDatePrecision::Quarter => DatePrecision::Quarter,
            StartDatePrecision::Negotiable => DatePrecision::Negotiable,
            StartDatePrecision::Unknown => DatePrecision::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone};

    fn base_year() -> i32 {
        Utc::now().year() + 1
    }

    fn base_datetime(month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(base_year(), month, day, 0, 0, 0)
            .unwrap()
    }

    #[test]
    fn normalizes_asap_and_exact_dates() {
        let year = base_year();
        let base = base_datetime(1, 10);

        let asap = normalize_start_date("即日", base).unwrap();
        assert_eq!(asap.date, Some(base.date_naive()));
        assert_eq!(asap.precision, StartDatePrecision::Asap);

        let exact = normalize_start_date(&format!("{year}/02/15"), base).unwrap();
        assert_eq!(
            exact.date,
            Some(NaiveDate::from_ymd_opt(year, 2, 15).unwrap())
        );
        assert_eq!(exact.precision, StartDatePrecision::ExactDay);
    }

    #[test]
    fn normalizes_next_month_and_month_parts() {
        let year = base_year();
        let base = base_datetime(1, 28);

        let next_month = normalize_start_date("来月", base).unwrap();
        assert_eq!(
            next_month.date,
            Some(NaiveDate::from_ymd_opt(year, 2, 1).unwrap())
        );
        assert_eq!(next_month.precision, StartDatePrecision::ApproximateMonth);

        let late = normalize_start_date("3月下旬", base).unwrap();
        assert_eq!(
            late.date,
            Some(NaiveDate::from_ymd_opt(year, 3, 25).unwrap())
        );
        assert_eq!(late.precision, StartDatePrecision::ApproximateMonth);
    }

    #[test]
    fn rolls_months_into_next_year_when_needed() {
        let year = base_year();
        let base = base_datetime(12, 15);

        let next_month = normalize_start_date("来月", base).unwrap();
        assert_eq!(
            next_month.date,
            Some(NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap())
        );

        let november = normalize_start_date("11月", base).unwrap();
        assert_eq!(
            november.date,
            Some(NaiveDate::from_ymd_opt(year + 1, 11, 1).unwrap())
        );
    }

    #[test]
    fn parses_quarter_and_negotiable_cases() {
        let year = base_year();
        let base = base_datetime(1, 10);

        let q2 = normalize_start_date(&format!("{}Q2", year + 1), base).unwrap();
        assert_eq!(
            q2.date,
            Some(NaiveDate::from_ymd_opt(year + 1, 4, 1).unwrap())
        );
        assert_eq!(q2.precision, StartDatePrecision::Quarter);
        assert!(q2.interpretation_note.is_none());

        let q3 = normalize_start_date("第3四半期", base).unwrap();
        assert_eq!(q3.date, Some(NaiveDate::from_ymd_opt(year, 7, 1).unwrap()));
        assert_eq!(q3.precision, StartDatePrecision::Quarter);
        assert!(q3
            .interpretation_note
            .as_ref()
            .map(|n| n.contains("received_at"))
            .unwrap_or(false));

        let negotiable = normalize_start_date("参画時期は応相談です", base).unwrap();
        assert_eq!(negotiable.date, None);
        assert_eq!(negotiable.precision, StartDatePrecision::Negotiable);
        assert!(negotiable
            .interpretation_note
            .as_ref()
            .map(|n| n.contains("negotiable"))
            .unwrap_or(false));
    }
}
