use chrono::NaiveDate;

/// 日付の解像度（精度）を表現
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatePrecision {
    /// YYYY-MM-DD など日付まで確定
    Exact,
    /// YYYY-MM など月単位
    Month,
    /// 四半期単位 (Q1/Q2/Q3/Q4)
    Quarter,
    /// 月の上旬 (1日固定)
    Early,
    /// 月の中旬 (15日固定)
    Middle,
    /// 月の下旬 (25日固定)
    Late,
    /// ASAP/即日
    Asap,
    /// 応相談など日付未確定
    Negotiable,
    /// 解釈不能
    Unknown,
}

impl DatePrecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            DatePrecision::Exact => "exact",
            DatePrecision::Month => "month",
            DatePrecision::Quarter => "quarter",
            DatePrecision::Early => "early",
            DatePrecision::Middle => "middle",
            DatePrecision::Late => "late",
            DatePrecision::Asap => "asap",
            DatePrecision::Negotiable => "negotiable",
            DatePrecision::Unknown => "unknown",
        }
    }
}

/// 日付正規化結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedDate {
    pub date: Option<NaiveDate>,
    pub precision: DatePrecision,
    pub interpretation_note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_all_precisions_as_strings() {
        let cases = vec![
            DatePrecision::Exact,
            DatePrecision::Month,
            DatePrecision::Quarter,
            DatePrecision::Early,
            DatePrecision::Middle,
            DatePrecision::Late,
            DatePrecision::Asap,
            DatePrecision::Negotiable,
            DatePrecision::Unknown,
        ];

        let labels: Vec<_> = cases.iter().map(|p| p.as_str()).collect();
        assert!(labels.contains(&"quarter"));
        assert!(labels.contains(&"negotiable"));
        assert!(labels.contains(&"asap"));
    }
}
