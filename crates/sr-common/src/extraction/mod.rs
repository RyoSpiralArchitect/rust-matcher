use chrono::NaiveDate;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::corrections::{
    flow_depth::correct_flow_dept,
    remote_onsite::correct_remote_onsite,
    todofuken::{correct_todofuken, TODOFUKEN_TO_AREA},
};
use crate::queue::RecommendedMethod;
use crate::skill_normalizer::normalize_skill_set;

/// sr-extractor がメール本文から拾う項目（MVP 範囲）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PartialFields {
    pub monthly_tanka_min: Option<u32>,
    pub monthly_tanka_max: Option<u32>,
    pub start_date_raw: Option<String>,
    pub work_todofuken: Option<String>,
    pub remote_onsite: Option<String>,
    pub flow_dept: Option<String>,
    pub required_skills_keywords: Option<Vec<String>>,
    pub project_name: Option<String>,
    pub outcome_tag: Option<String>,
    pub decline_reason_tag: Option<String>,
}

/// メール1通分の抽出結果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractorOutput {
    pub partial: PartialFields,
    pub quality: ExtractionQuality,
    pub decision: RecommendedDecision,
}

/// 抽出品質（Tier 充足度ベース）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExtractionQuality {
    pub tier1_extracted: usize,
    pub tier1_total: usize,
    pub tier2_extracted: usize,
    pub tier2_total: usize,
    pub llm_recommended: bool,
    pub reason: String,
}

/// 推奨メソッドの判定結果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendedDecision {
    pub recommended_method: RecommendedMethod,
    pub reason: String,
}

lazy_static! {
    // レンジ: "70〜90万円" / "70-90万円" / "70万円〜90万円"
    static ref RANGE_RE: Regex = Regex::new(r"(\d{1,3})\s*[〜～~-]\s*(\d{1,3})\s*万円").unwrap();
    // 下限のみ: "70万円〜" / "70万円以上"
    static ref MIN_ONLY_RE: Regex = Regex::new(r"(\d{1,3})\s*万円\s*(?:〜|以上)").unwrap();
    // 上限のみ: "〜90万円" / "90万円まで"
    static ref MAX_ONLY_RE: Regex = Regex::new(r"(?:〜|まで)\s*(\d{1,3})\s*万円").unwrap();
    // 単発: "80万円" / "80万円程度"
    static ref SINGLE_RE: Regex = Regex::new(r"(\d{1,3})\s*万円(?:程度|くらい|前後)?").unwrap();

    // 開始日: ASAP / 即日系
    static ref ASAP_RE: Regex = Regex::new(r"(?i)(即日|即時|ASAP)").unwrap();
    // 来月
    static ref NEXT_MONTH_RE: Regex = Regex::new(r"来月").unwrap();
    // 〇月上旬/中旬/下旬
    static ref MONTH_PART_RE: Regex = Regex::new(r"\d{1,2}月(?:上旬|中旬|下旬)").unwrap();
    // YYYY/MM/DD or YYYY-MM-DD
    static ref EXACT_DATE_RE: Regex = Regex::new(r"\d{4}[/-]\d{1,2}[/-]\d{1,2}").unwrap();
    // 〇月のみ（解像度は低いが Tier1 を埋める）
    static ref MONTH_ONLY_RE: Regex = Regex::new(r"\d{1,2}月").unwrap();

    // リモート/出社キーワード
    static ref FULL_REMOTE_RE: Regex =
        Regex::new(r"(?i)(フルリモート|完全在宅|常時リモート|full\s*remote)").unwrap();
    static ref HYBRID_REMOTE_RE: Regex = Regex::new(
        r"(?i)(週\s*[1-4１-４一二三四]\s*リモート|リモート可|リモート併用|ハイブリッド)"
    )
    .unwrap();
    static ref ONSITE_RE: Regex = Regex::new(r"(?i)(フル出社|常駐|客先|出社のみ|出社必須)").unwrap();

    // 商流キーワード
    static ref END_DIRECT_RE: Regex = Regex::new(r"(?i)(エンド直|直請け)").unwrap();
    static ref ONE_HOP_RE: Regex = Regex::new(r"(?i)(1次|一次|元請|プライム)").unwrap();
    static ref TWO_HOP_RE: Regex = Regex::new(r"(?i)(2次|二次)").unwrap();
    static ref THREE_HOP_RE: Regex = Regex::new(r"(?i)(3次|三次)").unwrap();
    static ref FOUR_PLUS_RE: Regex = Regex::new(r"(?i)(4次|四次|4次以上|四次以上)").unwrap();
}

/// 本文から Tier1/2 相当の項目をまとめて抽出
pub fn extract_partial_fields(body_text: &str) -> PartialFields {
    let mut partial = PartialFields::default();

    if let Some((min, max)) = extract_tanka(body_text) {
        partial.monthly_tanka_min = Some(min);
        partial.monthly_tanka_max = Some(max);
    }

    partial.start_date_raw = extract_start_date_raw(body_text);
    partial.work_todofuken = extract_work_todofuken(body_text);
    partial.remote_onsite = extract_remote_onsite(body_text);
    partial.flow_dept = extract_flow_dept(body_text);
    partial.outcome_tag = Some("unknown".to_string());

    partial
}

/// 月単価抽出（レンジ/下限のみ/上限のみ/単発に対応）
/// 単発値は min=max として返す（Tier1 を落とさない）
pub fn extract_tanka(body_text: &str) -> Option<(u32, u32)> {
    // 1. レンジ: "70〜90万円"
    if let Some(caps) = RANGE_RE.captures(body_text) {
        let min: u32 = caps.get(1)?.as_str().parse().ok()?;
        let max: u32 = caps.get(2)?.as_str().parse().ok()?;
        if (30..=200).contains(&min) && min <= max && max <= 200 {
            return Some((min, max));
        }
    }

    // 2. 下限のみ: "70万円〜"
    if let Some(caps) = MIN_ONLY_RE.captures(body_text) {
        let min: u32 = caps.get(1)?.as_str().parse().ok()?;
        if (30..=200).contains(&min) {
            let max = (min + 20).min(200);
            return Some((min, max));
        }
    }

    // 3. 上限のみ: "〜90万円"
    if let Some(caps) = MAX_ONLY_RE.captures(body_text) {
        let max: u32 = caps.get(1)?.as_str().parse().ok()?;
        if (30..=200).contains(&max) {
            let min = if max > 20 { max - 20 } else { 30 };
            return Some((min, max));
        }
    }

    // 4. 単発: "80万円程度" → min=max
    if let Some(caps) = SINGLE_RE.captures(body_text) {
        let tanka: u32 = caps.get(1)?.as_str().parse().ok()?;
        if (30..=200).contains(&tanka) {
            return Some((tanka, tanka));
        }
    }

    None
}

/// 開始日の生テキストを保守的に抽出
pub fn extract_start_date_raw(body_text: &str) -> Option<String> {
    if body_text.trim().is_empty() {
        return None;
    }

    if let Some(mat) = ASAP_RE.find(body_text) {
        return Some(mat.as_str().trim().to_string());
    }

    if let Some(mat) = NEXT_MONTH_RE.find(body_text) {
        return Some(mat.as_str().trim().to_string());
    }

    if let Some(mat) = MONTH_PART_RE.find(body_text) {
        return Some(mat.as_str().trim().to_string());
    }

    if let Some(mat) = EXACT_DATE_RE.find(body_text) {
        let raw = mat.as_str().trim();
        let normalized = raw.replace('/', "-");
        match NaiveDate::parse_from_str(&normalized, "%Y-%m-%d") {
            Ok(_) => return Some(raw.to_string()),
            Err(err) => {
                warn!(raw_start_date = raw, error = %err, "failed to parse exact start date");
            }
        }
    }

    if let Some(mat) = MONTH_ONLY_RE.find(body_text) {
        return Some(mat.as_str().trim().to_string());
    }

    None
}

/// メール本文から都道府県を最初に見つかったものだけ抽出
pub fn extract_work_todofuken(body_text: &str) -> Option<String> {
    if body_text.trim().is_empty() {
        return None;
    }

    TODOFUKEN_TO_AREA
        .keys()
        .filter(|pref| body_text.contains(*pref))
        .max_by_key(|pref| pref.len())
        .and_then(|pref| correct_todofuken(pref))
}

/// メールから Tier1/Tier2 を抽出し、品質判定まで含めた結果を返す
pub fn extract_all_fields(body_text: &str, subject: Option<&str>) -> ExtractorOutput {
    extract_all_fields_with_skills(body_text, subject, None)
}

/// メールから Tier1/Tier2 を抽出し、スキルキーワードも正規化した結果を返す
pub fn extract_all_fields_with_skills(
    body_text: &str,
    subject: Option<&str>,
    required_skills_keywords: Option<Vec<String>>,
) -> ExtractorOutput {
    let mut partial = extract_partial_fields(body_text);
    partial.required_skills_keywords = required_skills_keywords;

    if let Some(subj) = subject {
        let trimmed = subj.trim();
        if !trimmed.is_empty() {
            partial.project_name = Some(trimmed.to_string());
        }
    }

    normalize_required_skills(&mut partial);

    let (quality, decision) = evaluate_quality(&partial);

    ExtractorOutput {
        partial,
        quality,
        decision,
    }
}

/// メール本文からリモート/出社形態を抽出して ENUM 補正
pub fn extract_remote_onsite(body_text: &str) -> Option<String> {
    if FULL_REMOTE_RE.is_match(body_text) {
        return Some("フルリモート".to_string());
    }

    if HYBRID_REMOTE_RE.is_match(body_text) {
        return Some("リモート併用".to_string());
    }

    if ONSITE_RE.is_match(body_text) {
        return correct_remote_onsite("フル出社");
    }

    correct_remote_onsite(body_text)
}

/// メール本文から商流を抽出して ENUM 補正
pub fn extract_flow_dept(body_text: &str) -> Option<String> {
    if END_DIRECT_RE.is_match(body_text) {
        return Some("エンド直".to_string());
    }
    if ONE_HOP_RE.is_match(body_text) {
        return Some("1次請け".to_string());
    }
    if TWO_HOP_RE.is_match(body_text) {
        return Some("2次請け".to_string());
    }
    if THREE_HOP_RE.is_match(body_text) {
        return Some("3次請け".to_string());
    }
    if FOUR_PLUS_RE.is_match(body_text) {
        return Some("4次請け以上".to_string());
    }

    let corrected = correct_flow_dept(body_text);
    if corrected == "不明" {
        None
    } else {
        Some(corrected)
    }
}

fn normalize_required_skills(partial: &mut PartialFields) {
    if let Some(raw) = partial.required_skills_keywords.take() {
        let mut normalized: Vec<_> = normalize_skill_set(&raw).into_iter().collect();
        if !normalized.is_empty() {
            normalized.sort();
            partial.required_skills_keywords = Some(normalized);
        }
    }
}

/// Tier 充足度をカウント
pub fn calculate_quality(partial: &PartialFields) -> ExtractionQuality {
    let tier1_extracted = [
        partial.monthly_tanka_min.is_some(),
        partial.monthly_tanka_max.is_some(),
        partial.start_date_raw.is_some(),
        partial.work_todofuken.is_some(),
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    let tier2_extracted = [partial.remote_onsite.is_some(), partial.flow_dept.is_some()]
        .iter()
        .filter(|&&x| x)
        .count();

    ExtractionQuality {
        tier1_extracted,
        tier1_total: 4,
        tier2_extracted,
        tier2_total: 2,
        llm_recommended: false,
        reason: String::new(),
    }
}

/// Tier 不足ベースで推奨メソッドを決定
pub fn decide_recommended_method(quality: &ExtractionQuality) -> RecommendedDecision {
    if quality.tier1_extracted < quality.tier1_total {
        return RecommendedDecision {
            recommended_method: RecommendedMethod::LlmRecommended,
            reason: format!(
                "LLM recommended: Tier1 incomplete {}/{}",
                quality.tier1_extracted, quality.tier1_total
            ),
        };
    }

    if quality.tier2_extracted < 1 {
        return RecommendedDecision {
            recommended_method: RecommendedMethod::LlmRecommended,
            reason: format!(
                "LLM recommended: Tier2 incomplete {}/{}",
                quality.tier2_extracted, quality.tier2_total
            ),
        };
    }

    RecommendedDecision {
        recommended_method: RecommendedMethod::RustRecommended,
        reason: "Rust recommended: Tier1/Tier2 satisfied".to_string(),
    }
}

/// priority は「処理順序」であり品質スコアとは独立
pub fn calculate_priority(quality: &ExtractionQuality) -> i32 {
    if quality.tier1_extracted < quality.tier1_total {
        100
    } else if quality.tier2_extracted == 0 {
        50
    } else if quality.tier2_extracted == 1 {
        20
    } else {
        10
    }
}

/// quality と decision をまとめて生成
pub fn evaluate_quality(partial: &PartialFields) -> (ExtractionQuality, RecommendedDecision) {
    let mut quality = calculate_quality(partial);
    let decision = decide_recommended_method(&quality);
    quality.llm_recommended = decision.recommended_method == RecommendedMethod::LlmRecommended;
    quality.reason = decision.reason.clone();
    (quality, decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn tanka_extracts_range_min_max_and_single() {
        assert_eq!(extract_tanka("月額70〜90万円の案件です"), Some((70, 90)));
        assert_eq!(extract_tanka("70万円〜で検討"), Some((70, 90)));
        assert_eq!(extract_tanka("〜90万円まで"), Some((70, 90)));
        assert_eq!(extract_tanka("80万円程度"), Some((80, 80)));
        assert_eq!(extract_tanka("25万円"), None);
    }

    #[test]
    fn start_date_raw_matches_common_patterns() {
        let year = chrono::Utc::now().year() + 1;
        assert_eq!(
            extract_start_date_raw("即日で参画可能です"),
            Some("即日".to_string())
        );
        assert_eq!(
            extract_start_date_raw("来月開始想定"),
            Some("来月".to_string())
        );
        assert_eq!(
            extract_start_date_raw("1月上旬スタート"),
            Some("1月上旬".to_string())
        );
        assert_eq!(
            extract_start_date_raw(&format!("開始日は{year}/02/15を予定")),
            Some(format!("{year}/02/15"))
        );
        assert_eq!(
            extract_start_date_raw("12月開始を希望しています"),
            Some("12月".to_string())
        );
        assert_eq!(extract_start_date_raw("未定です"), None);
        assert_eq!(
            extract_start_date_raw(&format!("Start date is {year}/03/10 for the project")),
            Some(format!("{year}/03/10"))
        );
    }

    #[test]
    fn start_date_handles_invalid_and_edge_cases() {
        let year = chrono::Utc::now().year();
        // Invalid calendar dates should be rejected with logging
        assert_eq!(
            extract_start_date_raw(&format!("{year}/02/30")),
            None,
            "invalid day should not be accepted"
        );
        assert_eq!(
            extract_start_date_raw(&format!("{year}-13-01")),
            None,
            "invalid month should not be accepted"
        );

        // Edge expressions should still be surfaced verbatim
        assert_eq!(
            extract_start_date_raw("来月上旬に開始希望"),
            Some("来月上旬".to_string())
        );
        assert_eq!(
            extract_start_date_raw("12月下旬スタートを想定しています"),
            Some("12月下旬".to_string())
        );
    }

    #[test]
    fn quality_and_recommendation_follow_tier_rules() {
        let mut partial = PartialFields::default();
        partial.monthly_tanka_min = Some(70);
        partial.monthly_tanka_max = Some(90);
        partial.start_date_raw = Some("即日".to_string());
        partial.work_todofuken = Some("東京都".to_string());

        // Tier1=4/4, Tier2=0 -> LLM 推奨
        let (quality, decision) = evaluate_quality(&partial);
        assert_eq!(quality.tier1_extracted, 4);
        assert_eq!(quality.tier2_extracted, 0);
        assert_eq!(
            decision.recommended_method,
            RecommendedMethod::LlmRecommended
        );
        assert!(decision.reason.contains("Tier2 incomplete"));

        // Tier2 も埋めれば Rust 推奨
        partial.remote_onsite = Some("リモート併用".to_string());
        partial.flow_dept = Some("自社".to_string());
        let (quality2, decision2) = evaluate_quality(&partial);
        assert_eq!(quality2.tier2_extracted, 2);
        assert_eq!(
            decision2.recommended_method,
            RecommendedMethod::RustRecommended
        );
    }

    #[test]
    fn extracts_prefecture_remote_and_flow_patterns() {
        let text = "勤務地：東京都千代田区\n週3リモート、元請案件です";
        assert_eq!(extract_work_todofuken(text), Some("東京都".to_string()));
        assert_eq!(
            extract_remote_onsite(text),
            Some("リモート併用".to_string())
        );
        assert_eq!(extract_flow_dept(text), Some("1次請け".to_string()));

        let onsite = "勤務地: 大阪府\nフル出社（客先常駐）";
        assert_eq!(extract_remote_onsite(onsite), Some("フル出社".to_string()));

        let deep = "4次以上の商流です";
        assert_eq!(extract_flow_dept(deep), Some("4次請け以上".to_string()));
    }

    #[test]
    fn extract_all_fields_sets_tiers_and_project_name() {
        let body = "月額80万円、勤務地: 大阪府。週2リモート可、一次請け案件。即日参画。";
        let output = extract_all_fields(body, Some("【案件】データエンジニア"));

        assert_eq!(output.partial.monthly_tanka_min, Some(80));
        assert_eq!(output.partial.monthly_tanka_max, Some(80));
        assert_eq!(output.partial.work_todofuken, Some("大阪府".to_string()));
        assert_eq!(
            output.partial.remote_onsite,
            Some("リモート併用".to_string())
        );
        assert_eq!(output.partial.flow_dept, Some("1次請け".to_string()));
        assert_eq!(
            output.partial.project_name,
            Some("【案件】データエンジニア".to_string())
        );

        assert_eq!(output.quality.tier1_extracted, 4);
        assert_eq!(output.quality.tier2_extracted, 2);
        assert_eq!(
            output.decision.recommended_method,
            RecommendedMethod::RustRecommended
        );
    }

    #[test]
    fn normalizes_required_skills_after_extraction() {
        let output = extract_all_fields_with_skills(
            "月額80万円、勤務地: 大阪府。",
            Some("データエンジニア"),
            Some(vec![" JavaScript ".into(), "js".into(), "Rust".into()]),
        );

        assert_eq!(
            output.partial.required_skills_keywords.unwrap(),
            vec!["javascript", "rust"]
        );
    }
}
