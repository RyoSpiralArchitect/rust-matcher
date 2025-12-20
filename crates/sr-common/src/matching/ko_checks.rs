use super::{
    ko_unified::{KnockoutResultV2, KoDecision},
    location::evaluate_location,
    skills::check_required_skills,
};
use crate::{
    Project, Talent,
    corrections::{
        contract_type::correct_contract_type,
        english_skill::is_english_ko,
        flow_depth::{
            check_flow_ko as check_flow_depth_ko, parse_flow_limit, parse_talent_flow_depth,
        },
        japanese_skill::is_japanese_ko,
        nationality::is_japanese_nationality,
    },
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
        ("foreigner", check_foreigner_ko(project, talent)),
        ("contract", check_contract_type_ko(project, talent)),
        ("flow", check_flow_limit_ko(project, talent)),
        (
            "ng_keyword",
            check_ng_keyword_ko(
                talent.ng_keywords.as_deref(),
                project.project_keywords.as_deref(),
            ),
        ),
        ("age", check_age_ko(project, talent)),
        ("availability", check_availability_ko(project, talent)),
    ];

    KnockoutResultV2::new(decisions)
}

/// 単価KO判定（利益<5万円でHardKo、情報不足はSoftKo）
fn check_tanka_ko(project: &Project, talent: &Talent) -> KoDecision {
    let Some(talent_min) = talent.desired_price_min else {
        return KoDecision::SoftKo {
            reason: "tanka_unknown: 単価情報不足".into(),
        };
    };

    if let Some(max) = project.monthly_tanka_max {
        let profit = max as i32 - talent_min as i32;
        if profit < 5 {
            return KoDecision::HardKo {
                reason: format!(
                    "tanka_ko: 利益 {}万 < 閾値5万 (案件上限{}万 - 人材下限{}万)",
                    profit, max, talent_min
                ),
            };
        }

        return KoDecision::Pass;
    }

    if let Some(min) = project.monthly_tanka_min {
        if talent_min <= min {
            KoDecision::Pass
        } else {
            KoDecision::SoftKo {
                reason: format!(
                    "tanka_unknown: 案件下限{}万に対し人材下限{}万、上限情報不足のため要確認",
                    min, talent_min
                ),
            }
        }
    } else {
        KoDecision::SoftKo {
            reason: "tanka_unknown: 単価情報不足".into(),
        }
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

/// 外国籍可否KO判定
/// - foreigner_allowed==false かつ 国籍が日本以外の場合: HardKo
/// - 国籍不明の場合: SoftKo
/// - 上記以外: Pass
fn check_foreigner_ko(project: &Project, talent: &Talent) -> KoDecision {
    if !matches!(project.foreigner_allowed, Some(false)) {
        return KoDecision::Pass;
    }

    match talent.nationality.as_deref() {
        Some(nat) if is_japanese_nationality(nat) => KoDecision::Pass,
        Some(nat) => KoDecision::HardKo {
            reason: format!("foreigner_not_allowed: {}", nat),
        },
        None => KoDecision::SoftKo {
            reason: "foreigner_unknown: 国籍情報不足".into(),
        },
    }
}

/// 契約形態KO（現状は情報不足をSoftKo、矛盾はHardKo）
fn check_contract_type_ko(project: &Project, talent: &Talent) -> KoDecision {
    let is_kojin_ok = project.is_kojin_ok.unwrap_or(true);

    let project_contract = project
        .contract_type
        .as_deref()
        .map(|c| {
            let trimmed = c.trim();
            let corrected = correct_contract_type(trimmed);

            if corrected == "準委任契約" && trimmed != corrected && !trimmed.contains("派遣")
            {
                trimmed.to_string()
            } else {
                corrected
            }
        })
        .filter(|c| !c.is_empty());
    let primary = talent
        .primary_contract_type
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string());
    let secondary = talent
        .secondary_contract_type
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string());

    match (
        project_contract.as_deref(),
        primary.as_deref(),
        secondary.as_deref(),
    ) {
        (None, _, _) => KoDecision::Pass,
        (Some(req), _, Some(secondary)) if req == secondary => KoDecision::Pass,
        (Some(req), Some(primary), _) if req == primary => KoDecision::Pass,
        (Some(_), Some(primary), secondary)
            if is_kojin_ok && (primary == "直個人" || secondary == Some("直個人")) =>
        {
            KoDecision::Pass
        }
        (Some(_), None, _) => KoDecision::SoftKo {
            reason: "contract_unknown: 人材契約形態が未設定".into(),
        },
        (Some(req), Some(primary), secondary) => KoDecision::HardKo {
            reason: format!(
                "contract_mismatch: required={}, talent={} / {:?}",
                req, primary, secondary
            ),
        },
    }
}

/// 商流制限KO判定
fn check_flow_limit_ko(project: &Project, talent: &Talent) -> KoDecision {
    let talent_depth = talent
        .flow_depth
        .as_deref()
        .and_then(parse_talent_flow_depth);
    let project_limit = project
        .jinzai_flow_limit
        .as_deref()
        .and_then(parse_flow_limit);

    check_flow_depth_ko(talent_depth, project_limit)
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

/// 開始日と参画可能日の衝突を検知
/// - 両者が分かり、タレントの参画可能日が案件開始日より遅い場合は HardKo
/// - いずれか不明や日付未確定は SoftKo（要確認）
fn check_availability_ko(project: &Project, talent: &Talent) -> KoDecision {
    match (&project.start_date, &talent.availability_date) {
        (Some(project_start), Some(talent_available)) => {
            match (project_start.date, talent_available.date) {
                (Some(p), Some(t)) => {
                    if t > p {
                        KoDecision::HardKo {
                            reason: format!(
                                "availability_conflict: talent {} starts after project {}",
                                t, p
                            ),
                        }
                    } else {
                        KoDecision::Pass
                    }
                }
                _ => KoDecision::SoftKo {
                    reason: "availability_unknown: 精度不足のため確認要".into(),
                },
            }
        }
        _ => KoDecision::SoftKo {
            reason: "availability_unknown: 開始日/参画可能日が不足".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::{NormalizedStartDate, StartDatePrecision};
    use chrono::NaiveDate;

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
            start_date: Some(start_date(1)),
            jinzai_flow_limit: Some("SPONTO一社先まで".into()),
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
            availability_date: Some(start_date(1)),
            flow_depth: Some("1社先".into()),
            ..Talent::default()
        }
    }

    fn start_date(day: u32) -> NormalizedStartDate {
        NormalizedStartDate {
            date: NaiveDate::from_ymd_opt(2025, 1, day),
            precision: StartDatePrecision::ExactDay,
            interpretation_note: None,
        }
    }

    #[test]
    fn aggregates_location_and_skill_results() {
        let project = base_project();
        let talent = base_talent();

        let result = run_all_ko_checks(&project, &talent);
        assert!(!result.is_hard_knockout);
        assert!(!result.needs_manual_review);
        assert_eq!(result.decisions.len(), 10);
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

    #[test]
    fn detects_flow_limit_mismatches() {
        let mut project = base_project();
        project.jinzai_flow_limit = Some("SPONTO一社先まで".into());

        let mut talent = base_talent();
        talent.flow_depth = Some("2社先".into());

        let decision = check_flow_limit_ko(&project, &talent);
        assert!(
            matches!(decision, KoDecision::HardKo { reason } if reason.contains("flow_exceeded"))
        );

        talent.flow_depth = None;
        let soft = check_flow_limit_ko(&project, &talent);
        assert!(matches!(soft, KoDecision::SoftKo { .. }));
    }

    #[test]
    fn checks_foreigner_allowance() {
        let mut project = base_project();
        project.foreigner_allowed = Some(false);

        let mut talent = base_talent();
        talent.nationality = Some("アメリカ".into());
        let hard = check_foreigner_ko(&project, &talent);
        assert!(
            matches!(hard, KoDecision::HardKo { reason } if reason.contains("foreigner_not_allowed"))
        );

        talent.nationality = None;
        let soft = check_foreigner_ko(&project, &talent);
        assert!(matches!(soft, KoDecision::SoftKo { .. }));

        talent.nationality = Some(" JAPAN ".into());
        let pass = check_foreigner_ko(&project, &talent);
        assert!(matches!(pass, KoDecision::Pass));
    }

    #[test]
    fn allows_secondary_and_kojin_ok_contracts() {
        let mut project = base_project();
        project.contract_type = Some("業務委託".into());
        project.is_kojin_ok = Some(true);

        let mut talent = base_talent();
        talent.primary_contract_type = Some("派遣".into());
        talent.secondary_contract_type = Some("業務委託".into());

        let secondary_match = check_contract_type_ko(&project, &talent);
        assert!(matches!(secondary_match, KoDecision::Pass));

        talent.secondary_contract_type = None;
        talent.primary_contract_type = Some("直個人".into());
        let kojin_ok = check_contract_type_ko(&project, &talent);
        assert!(matches!(kojin_ok, KoDecision::Pass));
    }

    #[test]
    fn normalizes_contract_types_before_comparison() {
        let mut project = base_project();
        project.contract_type = Some(" 派遣契約 ".into());

        let mut talent = base_talent();
        talent.primary_contract_type = Some(" 派遣 ".into());

        let decision = check_contract_type_ko(&project, &talent);
        assert!(matches!(decision, KoDecision::Pass));
    }

    #[test]
    fn detects_availability_conflicts() {
        let mut project = base_project();
        project.start_date = Some(start_date(1));

        let mut talent = base_talent();
        talent.availability_date = Some(start_date(15));

        let result = check_availability_ko(&project, &talent);
        assert!(matches!(result, KoDecision::HardKo { .. }));

        talent.availability_date = Some(start_date(1));
        let pass = check_availability_ko(&project, &talent);
        assert!(matches!(pass, KoDecision::Pass));
    }
}
