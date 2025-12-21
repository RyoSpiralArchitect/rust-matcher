use std::collections::HashSet;

use crate::skill_normalizer::normalize_skill_set;

fn get_skill_match_threshold() -> f64 {
    std::env::var("SR_SKILL_MATCH_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.3)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillMatchResult {
    pub is_knockout: bool,
    pub match_percentage: f64,
    pub matched_skills: Vec<String>,
    pub reason: String,
    pub requires_manual_review: bool,
}

/// 必須スキルのマッチング判定（strategy.py の _check_required_skills 相当）
pub fn check_required_skills(
    project_skills: &[String],
    talent_skills: &[String],
) -> SkillMatchResult {
    check_required_skills_with_threshold(project_skills, talent_skills, None)
}

/// 歓迎スキルのマッチング判定（KOなし・単純一致率のみ）
pub fn check_preferred_skills(
    project_skills: &[String],
    talent_skills: &[String],
) -> SkillMatchResult {
    let pref_skill_set = normalize_skill_set(project_skills);
    if pref_skill_set.is_empty() {
        return SkillMatchResult {
            is_knockout: false,
            match_percentage: 1.0,
            matched_skills: vec![],
            reason: "歓迎スキル要件なし".into(),
            requires_manual_review: true,
        };
    }

    let talent_skill_set = normalize_skill_set(talent_skills);
    let matched: HashSet<_> = pref_skill_set
        .intersection(&talent_skill_set)
        .cloned()
        .collect();
    let matched_len = matched.len();
    let match_percentage = matched_len as f64 / pref_skill_set.len() as f64;
    let matched_skills: Vec<_> = matched.into_iter().collect();

    SkillMatchResult {
        is_knockout: false,
        match_percentage,
        matched_skills,
        reason: format!(
            "歓迎スキル{}件中{}件({:.0}%)に合致",
            pref_skill_set.len(),
            matched_len,
            match_percentage * 100.0
        ),
        requires_manual_review: false,
    }
}

fn check_required_skills_with_threshold(
    project_skills: &[String],
    talent_skills: &[String],
    threshold_override: Option<f64>,
) -> SkillMatchResult {
    let req_skill_set = normalize_skill_set(project_skills);

    // 必須スキル要件がなければ合格
    if req_skill_set.is_empty() {
        return SkillMatchResult {
            is_knockout: false,
            match_percentage: 1.0,
            matched_skills: vec![],
            reason: "必須スキル要件なし".to_string(),
            requires_manual_review: true,
        };
    }

    let talent_skill_set = normalize_skill_set(talent_skills);
    if talent_skill_set.is_empty() {
        return SkillMatchResult {
            is_knockout: true,
            match_percentage: 0.0,
            matched_skills: vec![],
            reason: "人材スキル情報が不足しているため必須スキルを確認できません".to_string(),
            requires_manual_review: true,
        };
    }
    let matched: HashSet<_> = req_skill_set
        .intersection(&talent_skill_set)
        .cloned()
        .collect();
    let match_percentage = matched.len() as f64 / req_skill_set.len() as f64;

    let threshold = threshold_override.unwrap_or_else(get_skill_match_threshold);
    let is_knockout = match_percentage < threshold;

    let matched_len = matched.len();
    let mut matched_skills: Vec<_> = matched.into_iter().collect();
    matched_skills.sort();
    let matched_for_reason = matched_skills.clone();

    let mut missing_skills: Vec<_> = req_skill_set
        .difference(&talent_skill_set)
        .cloned()
        .collect();
    missing_skills.sort();
    let missing_for_reason = missing_skills.clone();

    SkillMatchResult {
        is_knockout,
        match_percentage,
        matched_skills,
        reason: if is_knockout {
            format!(
                "必須スキルとのマッチ率が{:.0}%であり、基準の{:.0}%に達していません (不足: {})",
                match_percentage * 100.0,
                threshold * 100.0,
                missing_skills.join(", ")
            )
        } else {
            format!(
                "必須スキル{}件中{}件({:.0}%)に合致 (一致: {} / 不足: {})",
                req_skill_set.len(),
                matched_len,
                match_percentage * 100.0,
                if matched_len == 0 {
                    "なし".to_string()
                } else {
                    matched_for_reason.join(", ")
                },
                if missing_for_reason.is_empty() {
                    "なし".to_string()
                } else {
                    missing_for_reason.join(", ")
                }
            )
        },
        requires_manual_review: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_requirements_pass() {
        let result = check_required_skills(&[], &[]);
        assert!(!result.is_knockout);
        assert_eq!(result.match_percentage, 1.0);
        assert!(result.requires_manual_review);
    }

    #[test]
    fn threshold_controls_knockout() {
        let result = check_required_skills_with_threshold(
            &["React.js".to_string(), "K8s".to_string()],
            &["react".to_string()],
            Some(0.6),
        );

        assert!((result.match_percentage - 0.5).abs() < f64::EPSILON);
        assert!(result.is_knockout);
        assert!(result.reason.contains("60%"));
        assert!(result.reason.contains("不足"));
        assert!(!result.requires_manual_review);
    }

    #[test]
    fn alias_normalization_allows_match() {
        let result = check_required_skills_with_threshold(
            &["JavaScript".to_string(), "Kubernetes".to_string()],
            &["js".to_string(), "k8s".to_string()],
            Some(0.3),
        );

        assert!(!result.is_knockout);
        assert!((result.match_percentage - 1.0).abs() < f64::EPSILON);
        assert!(!result.requires_manual_review);
    }

    #[test]
    fn empty_talent_skills_require_manual_review() {
        let result = check_required_skills(&["rust".to_string()], &[]);

        assert!(result.is_knockout);
        assert!(result.requires_manual_review);
        assert_eq!(result.match_percentage, 0.0);
        assert!(result.reason.contains("不足"));
    }

    #[test]
    fn reason_lists_missing_and_matched_skills() {
        let result = check_required_skills_with_threshold(
            &["rust".to_string(), "k8s".to_string(), "react".to_string()],
            &["Rust".to_string(), "react".to_string()],
            Some(0.3),
        );

        assert!(!result.is_knockout);
        assert!(result.reason.contains("一致: react, rust") || result.reason.contains("一致: rust, react"));
        assert!(result.reason.contains("不足: kubernetes"));
    }
}
