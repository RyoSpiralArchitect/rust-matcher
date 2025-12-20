use super::{
    ko_unified::{KnockoutResultV2, KoDecision},
    location::evaluate_location,
    skills::check_required_skills,
};
use crate::{
    Project, Talent,
    corrections::{english_skill::is_english_ko, japanese_skill::is_japanese_ko},
};
use chrono::Datelike;
use std::collections::HashSet;

/// 全KO判定をまとめて実行する
pub fn run_all_ko_checks(project: &Project, talent: &Talent) -> KnockoutResultV2 {
    let decisions = vec![
        ("tanka", check_tanka_ko(project, talent)),
        (
            "required_skills",
            check_skill_ko(
                &project.required_skills_keywords,
                &talent.possessed_skills_keywords,
            ),
        ),
        ("location", check_location_ko(project, talent)),
        ("language", check_language_ko(project, talent)),
        ("contract", check_contract_type_ko(project, talent)),
        (
            "ng_keyword",
            check_ng_keyword_ko(
                talent.ng_keywords.as_deref(),
                project.project_keywords.as_deref(),
            ),
        ),
        ("age", check_age_ko(project, talent)),
    ];

    KnockoutResultV2::new(decisions)
}

/// 単価KO判定（利益<5万円でHardKo、情報不足はSoftKo）
fn check_tanka_ko(project: &Project, talent: &Talent) -> KoDecision {
    match (project.monthly_tanka_max, talent.desired_price_min) {
        (Some(max), Some(min)) => {
            let profit = max as i32 - min as i32;
            if profit < 5 {
                KoDecision::HardKo {
                    reason: format!(
                        "tanka_ko: 利益 {}万 < 閾値5万 (案件上限{}万 - 人材下限{}万)",
                        profit, max, min
                    ),
                }
            } else {
                KoDecision::Pass
            }
        }
        _ => KoDecision::SoftKo {
            reason: "tanka_unknown: 単価情報不足".into(),
        },
    }
}

/// 必須スキルKO判定（既存のスキルマッチ判定を利用）
fn check_skill_ko(project_skills: &[String], talent_skills: &[String]) -> KoDecision {
    let result = check_required_skills(project_skills, talent_skills);

    if result.requires_manual_review {
        return KoDecision::SoftKo {
            reason: "required_skills_missing: 必須スキル要件が空".into(),
        };
    }

    if result.is_knockout {
        KoDecision::HardKo {
            reason: result.reason,
        }
    } else {
        KoDecision::Pass
    }
}

/// 勤務地KO判定（既存の evaluate_location を委譲）
fn check_location_ko(project: &Project, talent: &Talent) -> KoDecision {
    evaluate_location(project, talent).ko_decision
}

/// 日本語/英語スキルのKO判定
fn check_language_ko(project: &Project, talent: &Talent) -> KoDecision {
    let japanese = is_japanese_ko(
        project.japanese_skill.as_deref(),
        talent.japanese_skill.as_deref(),
    );

    if let Some(true) = japanese {
        return KoDecision::HardKo {
            reason: "japanese_skill_insufficient: 日本語レベル不足".into(),
        };
    }

    if japanese.is_none() {
        return KoDecision::SoftKo {
            reason: "japanese_skill_unknown: 日本語スキル情報不足".into(),
        };
    }

    if is_english_ko(
        project.english_skill.as_deref(),
        talent.english_skill.as_deref(),
    ) {
        return KoDecision::HardKo {
            reason: "english_skill_insufficient: 英語レベル不足".into(),
        };
    }

    KoDecision::Pass
}

/// 契約形態KO（現状は情報不足をSoftKo、矛盾はHardKo）
fn check_contract_type_ko(project: &Project, talent: &Talent) -> KoDecision {
    match (
        project.contract_type.as_deref(),
        talent.primary_contract_type.as_deref(),
    ) {
        (Some(req), Some(tal)) => {
            if req == tal {
                KoDecision::Pass
            } else {
                KoDecision::HardKo {
                    reason: format!("contract_mismatch: required={}, talent={}", req, tal),
                }
            }
        }
        (Some(_), None) => KoDecision::SoftKo {
            reason: "contract_unknown: 人材契約形態が未設定".into(),
        },
        _ => KoDecision::Pass,
    }
}

/// NGキーワードKO判定
/// - 重複あり: HardKo
/// - どちらかNone: SoftKo
/// - 重複なし: Pass
fn check_ng_keyword_ko(
    talent_ng_keywords: Option<&[String]>,
    project_keywords: Option<&[String]>,
) -> KoDecision {
    match (talent_ng_keywords, project_keywords) {
        (Some(ng), Some(project)) => {
            let ng_set: HashSet<_> = ng.iter().collect();
            let project_set: HashSet<_> = project.iter().collect();
            let overlap: Vec<_> = ng_set.intersection(&project_set).collect();

            if overlap.is_empty() {
                KoDecision::Pass
            } else {
                KoDecision::HardKo {
                    reason: format!(
                        "ng_keyword_overlap: {:?} が重複",
                        overlap.iter().take(3).collect::<Vec<_>>()
                    ),
                }
            }
        }
        _ => KoDecision::SoftKo {
            reason: "ng_keyword_unknown: キーワード情報不足のため要確認".into(),
        },
    }
}

/// 年齢KO判定: 下限/上限を満たさない場合は HardKo。情報不足は SoftKo。
fn check_age_ko(project: &Project, talent: &Talent) -> KoDecision {
    let birth_year = match talent.birth_year {
        Some(year) => year,
        None => {
            return KoDecision::SoftKo {
                reason: "age_unknown: 生年情報不足".into(),
            };
        }
    };

    let limits = (project.age_limit_lower, project.age_limit_upper);
    if limits == (None, None) {
        return KoDecision::Pass;
    }

    let current_year = chrono::Utc::now().year();
    let age = current_year - birth_year;

    if let Some(lower) = project.age_limit_lower {
        if age < lower {
            return KoDecision::HardKo {
                reason: format!("age_below_limit: {} < {}", age, lower),
            };
        }
    }

    if let Some(upper) = project.age_limit_upper {
        if age > upper {
            return KoDecision::HardKo {
                reason: format!("age_above_limit: {} > {}", age, upper),
            };
        }
    }

    KoDecision::Pass
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_project() -> Project {
        Project {
            monthly_tanka_max: Some(80),
            required_skills_keywords: vec!["rust".into()],
            work_todofuken: Some("東京都".into()),
            work_area: Some("関東".into()),
            remote_onsite: Some("リモート併用".into()),
            japanese_skill: Some("N2".into()),
            english_skill: Some("ビジネス".into()),
            project_keywords: Some(vec!["Rust".into()]),
            ..Project::default()
        }
    }

    fn base_talent() -> Talent {
        Talent {
            desired_price_min: Some(70),
            possessed_skills_keywords: vec!["Rust".into()],
            residential_todofuken: Some("神奈川県".into()),
            residential_area: Some("関東".into()),
            japanese_skill: Some("N1".into()),
            english_skill: Some("ネイティブ".into()),
            ng_keywords: Some(vec!["金融".into()]),
            birth_year: Some(chrono::Utc::now().year() - 30),
            ..Talent::default()
        }
    }

    #[test]
    fn aggregates_location_and_skill_results() {
        let project = base_project();
        let talent = base_talent();

        let result = run_all_ko_checks(&project, &talent);
        assert!(!result.is_hard_knockout);
        assert!(!result.needs_manual_review);
        assert_eq!(result.decisions.len(), 7);
    }

    #[test]
    fn flags_profit_and_language_issues() {
        let mut project = base_project();
        project.monthly_tanka_max = Some(60);
        project.japanese_skill = Some("N1".into());

        let mut talent = base_talent();
        talent.desired_price_min = Some(58);
        talent.japanese_skill = Some("N2".into());
        talent.english_skill = Some("会話".into());

        let result = run_all_ko_checks(&project, &talent);
        assert!(result.is_hard_knockout);
        assert!(result.decisions.iter().any(
            |(_, d)| matches!(d, KoDecision::HardKo { reason } if reason.contains("tanka_ko"))
        ));
        assert!(result
            .decisions
            .iter()
            .any(|(_, d)| matches!(d, KoDecision::HardKo { reason } if reason.contains("language") || reason.contains("英語") || reason.contains("japanese"))));
    }

    #[test]
    fn marks_unknown_contract_as_softko() {
        let mut project = base_project();
        project.contract_type = Some("業務委託".into());

        let talent = base_talent();
        let result = run_all_ko_checks(&project, &talent);

        assert!(result.needs_manual_review);
        assert!(result
            .decisions
            .iter()
            .any(|(_, d)| matches!(d, KoDecision::SoftKo { reason } if reason.contains("contract_unknown"))));
    }

    #[test]
    fn detects_ng_keyword_overlap_and_unknowns() {
        let hard = check_ng_keyword_ko(
            Some(&["保険".into(), "金融".into()]),
            Some(&["Java".into(), "金融".into()]),
        );
        assert!(matches!(hard, KoDecision::HardKo { .. }));

        let soft = check_ng_keyword_ko(None, Some(&["Java".into()]));
        assert!(matches!(soft, KoDecision::SoftKo { .. }));
    }

    #[test]
    fn checks_age_limits_when_available() {
        let mut project = base_project();
        project.age_limit_lower = Some(25);
        project.age_limit_upper = Some(35);

        let mut talent = base_talent();
        let current_year = chrono::Utc::now().year();
        talent.birth_year = Some(current_year - 40);

        let result = check_age_ko(&project, &talent);
        assert!(matches!(result, KoDecision::HardKo { reason } if reason.contains(">")));

        talent.birth_year = Some(current_year - 30);
        let result = check_age_ko(&project, &talent);
        assert!(matches!(result, KoDecision::Pass));
    }
}
