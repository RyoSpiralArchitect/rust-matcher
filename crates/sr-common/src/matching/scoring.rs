use super::{
    location::evaluate_location,
    skills::{check_preferred_skills, check_required_skills},
    weights::Weights,
};
use crate::{Project, Talent};
use chrono::{Datelike, Utc};

use super::weights::{DETAILED_WEIGHTS, PREFILTER_WEIGHTS};

#[derive(Debug, Clone)]
pub struct MatchingConfig {
    pub weights: Weights,
    pub tanka_profit_minimum: f64,
    pub tanka_profit_optimal: f64,
    pub skill_match_minimum: f64,
    pub experience_buffer_years: f64,
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            weights: DETAILED_WEIGHTS,
            tanka_profit_minimum: 5.0,
            tanka_profit_optimal: 0.25,
            skill_match_minimum: env_skill_threshold(),
            experience_buffer_years: 0.5,
        }
    }
}

impl MatchingConfig {
    pub fn detailed() -> Self {
        Self::default()
    }

    pub fn prefilter() -> Self {
        Self {
            weights: PREFILTER_WEIGHTS,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScoringResult {
    pub score: f64,
    pub max_score: f64,
    pub status: &'static str,
    pub details: String,
}

#[derive(Debug, Clone)]
pub struct MatchScore {
    pub total: f64,
    pub tanka: ScoringResult,
    pub location: ScoringResult,
    pub skills: ScoringResult,
    pub experience: ScoringResult,
    pub contract: ScoringResult,
    pub other: ScoringResult,
}

/// Prefilter 用のスコア計算（粗選別）
pub fn calculate_prefilter_score(project: &Project, talent: &Talent) -> MatchScore {
    let engine = BusinessRulesEngine::new(MatchingConfig::prefilter());
    engine.calculate_match_score(project, talent)
}

/// 詳細スコア計算（ランキング用）
pub fn calculate_detailed_score(project: &Project, talent: &Talent) -> MatchScore {
    let engine = BusinessRulesEngine::new(MatchingConfig::detailed());
    engine.calculate_match_score(project, talent)
}

pub struct BusinessRulesEngine {
    config: MatchingConfig,
}

impl BusinessRulesEngine {
    pub fn new(config: MatchingConfig) -> Self {
        Self { config }
    }

    /// 総合スコア計算（Phase2 詳細スコアリング）
    pub fn calculate_match_score(&self, project: &Project, talent: &Talent) -> MatchScore {
        let tanka = self.score_tanka(project, talent);
        let location = self.score_location(project, talent);
        let skills = self.score_skills(project, talent);
        let experience = self.score_experience(project, talent);
        let contract = self.score_contract(project, talent);
        let other = self.score_other_factors(project, talent);

        let weights = self.config.weights;
        let total = tanka.score * weights.tanka
            + location.score * weights.location
            + skills.score * weights.skills
            + experience.score * weights.experience
            + contract.score * weights.contract
            + other.score * weights.other;

        MatchScore {
            total,
            tanka,
            location,
            skills,
            experience,
            contract,
            other,
        }
    }

    fn score_tanka(&self, project: &Project, talent: &Talent) -> ScoringResult {
        let talent_tanka = match talent.desired_price_min {
            Some(t) => t as f64,
            None => {
                return ScoringResult {
                    score: 0.5,
                    max_score: 1.0,
                    status: "UNKNOWN",
                    details: "人材希望単価が不明のため中立スコア".into(),
                };
            }
        };

        let project_tanka = match project.monthly_tanka_max {
            Some(t) => t as f64,
            None => {
                return ScoringResult {
                    score: 0.5,
                    max_score: 1.0,
                    status: "UNKNOWN",
                    details: "案件上限単価が不明のため中立スコア".into(),
                };
            }
        };

        let profit = project_tanka - talent_tanka;
        let min_profit = self.config.tanka_profit_minimum;
        let optimal_profit = project_tanka * self.config.tanka_profit_optimal;

        if profit < min_profit {
            return ScoringResult {
                score: 0.0,
                max_score: 1.0,
                status: "MISS",
                details: format!("利益不足: {:.1}万 < {:.1}万", profit, min_profit),
            };
        }

        let (score, status, details) = if profit >= optimal_profit {
            (
                1.0,
                "PERFECT_MATCH",
                format!("十分な利益: {:.1}万 ≥ {:.1}万", profit, optimal_profit),
            )
        } else if profit >= min_profit * 3.0 {
            (
                0.9,
                "MATCH",
                format!("良好な利益: {:.1}万 ≥ {:.1}万", profit, min_profit * 3.0),
            )
        } else if profit >= min_profit * 2.0 {
            (
                0.7,
                "MATCH",
                format!("許容利益: {:.1}万 ≥ {:.1}万", profit, min_profit * 2.0),
            )
        } else {
            (
                0.4,
                "PARTIAL_MATCH",
                format!("最低限利益: {:.1}万 ≥ {:.1}万", profit, min_profit),
            )
        };

        ScoringResult {
            score,
            max_score: 1.0,
            status,
            details,
        }
    }

    fn score_location(&self, project: &Project, talent: &Talent) -> ScoringResult {
        let evaluation = evaluate_location(project, talent);
        let unknown = matches!(
            evaluation.ko_decision,
            crate::matching::ko_unified::KoDecision::SoftKo { reason }
                if reason.contains("location_unknown")
        );

        let status = status_from_score(evaluation.score, unknown);

        ScoringResult {
            score: evaluation.score,
            max_score: 1.0,
            status,
            details: evaluation.details,
        }
    }

    fn score_skills(&self, project: &Project, talent: &Talent) -> ScoringResult {
        let required = check_required_skills(
            &project.required_skills_keywords,
            &talent.possessed_skills_keywords,
        );

        if required.requires_manual_review {
            return ScoringResult {
                score: 0.5,
                max_score: 1.0,
                status: "UNKNOWN",
                details: "必須スキル要件が未設定のため中立スコア".into(),
            };
        }

        if required.is_knockout {
            return ScoringResult {
                score: 0.0,
                max_score: 1.0,
                status: "MISS",
                details: required.reason,
            };
        }

        let preferred = check_preferred_skills(
            &project.preferred_skills_keywords,
            &talent.possessed_skills_keywords,
        );

        let score = required.match_percentage * 0.75 + preferred.match_percentage * 0.25;
        let status = if score >= 0.9 {
            "PERFECT_MATCH"
        } else if score >= self.config.skill_match_minimum.max(0.6) {
            "MATCH"
        } else if score >= self.config.skill_match_minimum {
            "PARTIAL_MATCH"
        } else {
            "MISS"
        };

        ScoringResult {
            score,
            max_score: 1.0,
            status,
            details: format!(
                "必須:{:.0}% ({}) / 歓迎:{:.0}% ({})",
                required.match_percentage * 100.0,
                required.reason,
                preferred.match_percentage * 100.0,
                preferred.reason
            ),
        }
    }

    fn score_experience(&self, project: &Project, talent: &Talent) -> ScoringResult {
        let required = match project.min_experience_years {
            Some(v) => v as f64,
            None => {
                return ScoringResult {
                    score: 1.0,
                    max_score: 1.0,
                    status: "PERFECT_MATCH",
                    details: "案件に経験年数要件なし".into(),
                };
            }
        };

        let actual = match talent.min_experience_years {
            Some(v) => v as f64,
            None => {
                return ScoringResult {
                    score: 0.5,
                    max_score: 1.0,
                    status: "UNKNOWN",
                    details: "人材の経験年数が不明のため中立スコア".into(),
                };
            }
        };

        let buffer = self.config.experience_buffer_years;
        let (score, status, details) = if actual >= required + buffer * 4.0 {
            (
                1.0,
                "PERFECT_MATCH",
                format!(
                    "経験大幅超過: {:.1}年 ≥ {:.1}年",
                    actual,
                    required + buffer * 4.0
                ),
            )
        } else if actual >= required + buffer * 2.0 {
            (
                0.9,
                "MATCH",
                format!(
                    "経験十分: {:.1}年 ≥ {:.1}年",
                    actual,
                    required + buffer * 2.0
                ),
            )
        } else if actual >= required + buffer {
            (
                0.8,
                "MATCH",
                format!("経験超過: {:.1}年 ≥ {:.1}年", actual, required + buffer),
            )
        } else if actual >= required {
            (
                0.7,
                "MATCH",
                format!("要件達成: {:.1}年 ≥ {:.1}年", actual, required),
            )
        } else if actual + buffer >= required {
            (
                0.4,
                "PARTIAL_MATCH",
                format!("要件近接: {:.1}年 ≈ {:.1}年", actual, required),
            )
        } else {
            (
                0.0,
                "MISS",
                format!("経験不足: {:.1}年 < {:.1}年", actual, required),
            )
        };

        ScoringResult {
            score,
            max_score: 1.0,
            status,
            details,
        }
    }

    fn score_contract(&self, project: &Project, talent: &Talent) -> ScoringResult {
        let is_kojin_ok = project.is_kojin_ok.unwrap_or(true);

        match (
            project.contract_type.as_deref(),
            talent.primary_contract_type.as_deref(),
            talent.secondary_contract_type.as_deref(),
        ) {
            (None, _, _) => ScoringResult {
                score: 1.0,
                max_score: 1.0,
                status: "PERFECT_MATCH",
                details: "案件側に契約形態の制約なし".into(),
            },
            (Some(req), Some(primary), _secondary) if req == primary => ScoringResult {
                score: 1.0,
                max_score: 1.0,
                status: "PERFECT_MATCH",
                details: format!("契約形態一致: {}", primary),
            },
            (Some(req), Some(primary), Some(secondary)) if req == secondary => ScoringResult {
                score: 0.7,
                max_score: 1.0,
                status: "PARTIAL_MATCH",
                details: format!(
                    "副次契約形態で合致: primary={}, secondary={}",
                    primary, secondary
                ),
            },
            (Some(req), Some(primary), secondary)
                if is_kojin_ok && (primary == "直個人" || secondary == Some("直個人")) =>
            {
                ScoringResult {
                    score: 0.8,
                    max_score: 1.0,
                    status: "MATCH",
                    details: format!(
                        "個人事業主許容のため直個人を許容: 要件={} vs 人材={} / {:?}",
                        req, primary, secondary
                    ),
                }
            }
            (Some(req), None, _) => ScoringResult {
                score: 0.5,
                max_score: 1.0,
                status: "UNKNOWN",
                details: format!("契約形態不明: 要件={}", req),
            },
            (Some(req), Some(primary), _) => ScoringResult {
                score: 0.0,
                max_score: 1.0,
                status: "MISS",
                details: format!("契約形態不一致: 要件={} vs 人材={}", req, primary),
            },
        }
    }

    fn score_other_factors(&self, project: &Project, talent: &Talent) -> ScoringResult {
        let mut details: Vec<String> = Vec::new();
        let mut score: f64 = 1.0;
        let mut status = "PERFECT_MATCH";

        // 年齢スコア
        if project.age_limit_lower.is_some() || project.age_limit_upper.is_some() {
            match talent.birth_year {
                Some(year) => {
                    let current_year = Utc::now().year();
                    let age = current_year - year;

                    if let Some(lower) = project.age_limit_lower {
                        if age < lower {
                            score = 0.0;
                            status = "MISS";
                            details.push(format!("年齢下限未達: {} < {}", age, lower));
                        }
                    }
                    if let Some(upper) = project.age_limit_upper {
                        if age > upper {
                            score = 0.0;
                            status = "MISS";
                            details.push(format!("年齢上限超過: {} > {}", age, upper));
                        }
                    }
                    if details.is_empty() {
                        details.push("年齢要件クリア".into());
                    }
                }
                None => {
                    score = score.min(0.5);
                    status = "UNKNOWN";
                    details.push("年齢情報不足".into());
                }
            }
        }

        // 外国籍可否（project.foreigner_allowed==Some(false) の場合のみチェック）
        if matches!(project.foreigner_allowed, Some(false)) {
            match talent.nationality.as_deref() {
                Some(nat) if nat.contains("日本") => {
                    details.push("国籍要件クリア".into());
                }
                Some(nat) => {
                    score = 0.0;
                    status = "MISS";
                    details.push(format!("外国籍不可: {}", nat));
                }
                None => {
                    score = score.min(0.5);
                    if status != "MISS" {
                        status = "UNKNOWN";
                    }
                    details.push("国籍情報不足".into());
                }
            }
        }

        if details.is_empty() {
            details.push("追加要素なし".into());
        }

        ScoringResult {
            score,
            max_score: 1.0,
            status,
            details: details.join(" / "),
        }
    }
}

fn env_skill_threshold() -> f64 {
    std::env::var("SR_SKILL_MATCH_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.3)
}

fn status_from_score(score: f64, unknown: bool) -> &'static str {
    if unknown {
        "UNKNOWN"
    } else if score >= 0.9 {
        "PERFECT_MATCH"
    } else if score >= 0.7 {
        "MATCH"
    } else if score >= 0.4 {
        "PARTIAL_MATCH"
    } else {
        "MISS"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_project() -> Project {
        Project {
            monthly_tanka_max: Some(120),
            work_todofuken: Some("東京都".into()),
            work_area: Some("関東".into()),
            remote_onsite: Some("リモート併用".into()),
            required_skills_keywords: vec!["Rust".into(), "AWS".into()],
            min_experience_years: Some(5),
            contract_type: Some("業務委託".into()),
            age_limit_upper: Some(60),
            ..Project::default()
        }
    }

    fn full_talent() -> Talent {
        Talent {
            desired_price_min: Some(80),
            residential_todofuken: Some("東京都".into()),
            residential_area: Some("関東".into()),
            possessed_skills_keywords: vec!["rust".into(), "aws".into()],
            min_experience_years: Some(6),
            primary_contract_type: Some("業務委託".into()),
            birth_year: Some(Utc::now().year() - 35),
            ..Talent::default()
        }
    }

    #[test]
    fn calculates_weighted_scores() {
        let engine = BusinessRulesEngine::new(MatchingConfig::default());
        let score = engine.calculate_match_score(&full_project(), &full_talent());

        assert!(score.total > 0.9);
        assert_eq!(score.skills.status, "PERFECT_MATCH");
        assert_eq!(score.experience.status, "MATCH");
        assert_eq!(score.other.status, "PERFECT_MATCH");
    }

    #[test]
    fn handles_unknown_fields_neutrally() {
        let engine = BusinessRulesEngine::new(MatchingConfig::default());
        let mut project = full_project();
        project.monthly_tanka_max = None;
        let mut talent = full_talent();
        talent.desired_price_min = None;

        let score = engine.calculate_match_score(&project, &talent);
        assert_eq!(score.tanka.status, "UNKNOWN");
        assert_eq!(score.tanka.score, 0.5);
    }

    #[test]
    fn applies_profit_based_tanka_scoring() {
        let engine = BusinessRulesEngine::new(MatchingConfig::default());
        let mut project = full_project();
        project.monthly_tanka_max = Some(85);
        let mut talent = full_talent();
        talent.desired_price_min = Some(82);

        let tanka = engine.score_tanka(&project, &talent);
        assert_eq!(tanka.status, "MISS");
        assert_eq!(tanka.score, 0.0);
    }

    #[test]
    fn unknown_experience_scores_neutrally() {
        let engine = BusinessRulesEngine::new(MatchingConfig::default());
        let mut talent = full_talent();
        talent.min_experience_years = None;

        let exp = engine.score_experience(&full_project(), &talent);
        assert_eq!(exp.status, "UNKNOWN");
        assert_eq!(exp.score, 0.5);
    }

    #[test]
    fn preferred_skills_affect_score() {
        let engine = BusinessRulesEngine::new(MatchingConfig::default());
        let mut project = full_project();
        project.required_skills_keywords = vec!["Rust".into()];
        project.preferred_skills_keywords = vec!["GraphQL".into()];
        let mut talent = full_talent();
        talent.possessed_skills_keywords = vec!["rust".into(), "graphql".into()];

        let skills = engine.score_skills(&project, &talent);
        assert_eq!(skills.status, "PERFECT_MATCH");
        assert!(skills.score > 0.9);
        assert!(skills.details.contains("歓迎"));
    }

    #[test]
    fn kojin_ok_projects_accept_individuals() {
        let engine = BusinessRulesEngine::new(MatchingConfig::default());
        let mut project = full_project();
        project.contract_type = Some("業務委託".into());
        project.is_kojin_ok = Some(true);
        let mut talent = full_talent();
        talent.primary_contract_type = Some("直個人".into());
        talent.secondary_contract_type = None;

        let contract = engine.score_contract(&project, &talent);
        assert_eq!(contract.status, "MATCH");
        assert!(contract.score >= 0.8);
    }

    #[test]
    fn prefilter_weights_downplay_experience_penalty() {
        let mut project = full_project();
        project.min_experience_years = Some(10);
        project.contract_type = Some("業務委託".into());

        let mut talent = full_talent();
        talent.min_experience_years = Some(1);
        talent.primary_contract_type = Some("業務委託".into());
        talent.birth_year = Some(Utc::now().year() - 25);

        let pre = calculate_prefilter_score(&project, &talent);
        let detailed = calculate_detailed_score(&project, &talent);

        assert!(pre.total > detailed.total);
        assert!(pre.experience.score < 0.5);
    }

    #[test]
    fn uses_location_evaluation_score() {
        let engine = BusinessRulesEngine::new(MatchingConfig::default());
        let mut talent = full_talent();
        talent.residential_todofuken = Some("大阪府".into());
        talent.residential_area = Some("関西".into());

        let score = engine.calculate_match_score(&full_project(), &talent);
        assert!(score.location.score < 0.6);
        assert!(score.location.details.contains("remote_onsite"));
    }

    #[test]
    fn experience_buffer_allows_partial_credit() {
        let mut config = MatchingConfig::default();
        config.experience_buffer_years = 1.0;
        let engine = BusinessRulesEngine::new(config);
        let mut talent = full_talent();
        talent.min_experience_years = Some(4);

        let exp = engine.score_experience(&full_project(), &talent);
        assert_eq!(exp.status, "PARTIAL_MATCH");
        assert!(exp.score <= 0.4);
    }

    #[test]
    fn scores_other_factors_and_weights_prefilter() {
        let mut project = full_project();
        project.foreigner_allowed = Some(false);
        project.age_limit_lower = Some(30);
        project.age_limit_upper = Some(40);

        let mut talent = full_talent();
        talent.birth_year = Some(Utc::now().year() - 45);
        talent.nationality = Some("France".into());

        let pre = calculate_prefilter_score(&project, &talent);
        assert_eq!(pre.other.status, "MISS");
        let pre_total = pre.total;

        talent.birth_year = Some(Utc::now().year() - 35);
        talent.nationality = Some("日本".into());
        let detailed = calculate_detailed_score(&project, &talent);
        assert_eq!(detailed.other.status, "PERFECT_MATCH");
        assert!(detailed.total > pre_total);
    }
}
