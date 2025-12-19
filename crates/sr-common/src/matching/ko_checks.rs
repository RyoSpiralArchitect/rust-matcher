use super::{
    ko_unified::{KoDecision, KnockoutResultV2},
    location::evaluate_location,
    skills::check_required_skills,
};
use crate::{
    corrections::{
        english_skill::is_english_ko,
        japanese_skill::is_japanese_ko,
    },
    Project, Talent,
};

/// 全KO判定をまとめて実行する
pub fn run_all_ko_checks(project: &Project, talent: &Talent) -> KnockoutResultV2 {
    let decisions = vec![
        ("tanka", check_tanka_ko(project, talent)),
        (
            "required_skills",
            check_skill_ko(&project.required_skills_keywords, &talent.possessed_skills_keywords),
        ),
        ("location", check_location_ko(project, talent)),
        ("language", check_language_ko(project, talent)),
        ("contract", check_contract_type_ko(project, talent)),
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

    if is_english_ko(project.english_skill.as_deref(), talent.english_skill.as_deref()) {
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
        assert_eq!(result.decisions.len(), 5);
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
        assert!(result
            .decisions
            .iter()
            .any(|(_, d)| matches!(d, KoDecision::HardKo { reason } if reason.contains("tanka_ko"))));
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
}
