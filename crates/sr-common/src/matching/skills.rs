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
    let matched: HashSet<_> = req_skill_set
        .intersection(&talent_skill_set)
        .cloned()
        .collect();
    let match_percentage = matched.len() as f64 / req_skill_set.len() as f64;

    let threshold = threshold_override.unwrap_or_else(get_skill_match_threshold);
    let is_knockout = match_percentage < threshold;

    let matched_len = matched.len();

    SkillMatchResult {
        is_knockout,
        match_percentage,
        matched_skills: matched.into_iter().collect(),
        reason: if is_knockout {
            format!(
                "必須スキルとのマッチ率が{:.0}%であり、基準の{:.0}%に達していません",
                match_percentage * 100.0,
                threshold * 100.0
            )
        } else {
            format!(
                "必須スキル{}件中{}件({:.0}%)に合致",
                req_skill_set.len(),
                matched_len,
                match_percentage * 100.0
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
}
