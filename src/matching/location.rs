use super::ko_unified::KoDecision;
use crate::{
    Project, Talent,
    corrections::{
        remote_onsite::{correct_remote_onsite, normalize_remote_onsite},
        todofuken::{correct_todofuken, correct_work_area},
    },
};

#[derive(Debug, Clone)]
pub struct LocationEvaluation {
    pub ko_decision: KoDecision,
    pub score: f64, // 0.0〜1.0
    pub details: String,
}

/// 正規化済みの Project/Talent を返す（勤務地/エリア/リモート形態のみ）
pub fn normalize_for_matching(project: &Project, talent: &Talent) -> (Project, Talent) {
    let normalized_project = {
        let work_todofuken = project
            .work_todofuken
            .as_deref()
            .and_then(correct_todofuken);
        let work_area = project
            .work_area
            .as_deref()
            .and_then(correct_work_area)
            .or_else(|| work_todofuken.as_deref().and_then(correct_work_area));

        let normalized_remote = normalize_remote_onsite(project.remote_onsite.as_deref().unwrap_or(""));
        let remote_onsite = Some(
            correct_remote_onsite(&normalized_remote).unwrap_or(normalized_remote),
        );

        Project {
            work_todofuken,
            work_area,
            remote_onsite,
        }
    };

    let normalized_talent = {
        let residential_todofuken = talent
            .residential_todofuken
            .as_deref()
            .and_then(correct_todofuken);
        let residential_area = talent
            .residential_area
            .as_deref()
            .and_then(correct_work_area)
            .or_else(|| residential_todofuken.as_deref().and_then(correct_work_area));

        Talent {
            residential_todofuken,
            residential_area,
        }
    };

    (normalized_project, normalized_talent)
}

/// 【唯一の勤務地判定関数】
/// KO判定・prefilter・スコアリング全てがこの関数を呼ぶこと。
pub fn evaluate_location(project: &Project, talent: &Talent) -> LocationEvaluation {
    let (project, talent) = normalize_for_matching(project, talent);
    let remote_mode = project.remote_onsite.as_deref();

    // 1. フルリモート → 勤務地KOなし、スコア1.0
    if remote_mode == Some("フルリモート") {
        return LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: 1.0,
            details: "フルリモート案件 - 勤務地制約なし".into(),
        };
    }

    // 2. 都道府県が双方ある → 都道府県ロジック
    if let (Some(p_pref), Some(t_pref)) = (
        project.work_todofuken.as_deref(),
        talent.residential_todofuken.as_deref(),
    ) {
        return evaluate_by_todofuken(p_pref, t_pref, remote_mode);
    }

    // 3. 片方でも都道府県がない → エリアで粗く判定
    if let (Some(p_area), Some(t_area)) = (
        project.work_area.as_deref(),
        talent.residential_area.as_deref(),
    ) {
        return evaluate_by_area(p_area, t_area, remote_mode);
    }

    // 4. どっちも取れない → SoftKo（手動レビューへ）
    LocationEvaluation {
        ko_decision: KoDecision::SoftKo {
            reason: "location_unknown: 勤務地情報不足のため要手動確認".into(),
        },
        score: 0.5, // 中立
        details: format!(
            "勤務地情報なし - 手動確認必要 (remote_onsite={:?})",
            remote_mode
        ),
    }
}

fn evaluate_by_todofuken(
    project_pref: &str,
    talent_pref: &str,
    remote_mode: Option<&str>,
) -> LocationEvaluation {
    if project_pref == talent_pref {
        // 同一都道府県
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: if remote_mode == Some("リモート併用") {
                0.95
            } else {
                1.0
            },
            details: format!(
                "都道府県一致: {} (remote_onsite={:?})",
                project_pref, remote_mode
            ),
        }
    } else if is_adjacent_prefecture(project_pref, talent_pref) {
        // 隣接都道府県（通勤圏内）
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: match remote_mode {
                Some("フル出社") => 0.6,
                Some("リモート併用") => 0.75,
                _ => 0.7,
            },
            details: format!(
                "隣接都道府県: {} ↔ {} (remote_onsite={:?})",
                talent_pref, project_pref, remote_mode
            ),
        }
    } else {
        // 遠隔（HardKoではなくSoftKo: リモート併用なら通えるかも）
        LocationEvaluation {
            ko_decision: if remote_mode == Some("フル出社") {
                KoDecision::HardKo {
                    reason: format!(
                        "location_mismatch: {} → {} はフル出社案件で通勤困難",
                        talent_pref, project_pref
                    ),
                }
            } else {
                KoDecision::SoftKo {
                    reason: format!(
                        "location_distant: {} → {} は通勤困難の可能性",
                        talent_pref, project_pref
                    ),
                }
            },
            score: if remote_mode == Some("フル出社") {
                0.0
            } else {
                0.2
            },
            details: format!(
                "都道府県不一致: {} ≠ {} (remote_onsite={:?})",
                talent_pref, project_pref, remote_mode
            ),
        }
    }
}

fn evaluate_by_area(
    project_area: &str,
    talent_area: &str,
    remote_mode: Option<&str>,
) -> LocationEvaluation {
    if project_area == talent_area {
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: if remote_mode == Some("リモート併用") {
                0.85
            } else {
                0.8
            },
            details: format!(
                "エリア一致: {} (remote_onsite={:?})",
                project_area, remote_mode
            ),
        }
    } else {
        LocationEvaluation {
            ko_decision: if remote_mode == Some("フル出社") {
                KoDecision::HardKo {
                    reason: format!(
                        "area_mismatch: {} ≠ {} (フル出社案件)",
                        talent_area, project_area
                    ),
                }
            } else {
                KoDecision::SoftKo {
                    reason: format!("area_mismatch: {} ≠ {}", talent_area, project_area),
                }
            },
            score: if remote_mode == Some("フル出社") {
                0.0
            } else {
                0.3
            },
            details: format!(
                "エリア不一致: {} ≠ {} (remote_onsite={:?})",
                talent_area, project_area, remote_mode
            ),
        }
    }
}

/// 隣接都道府県テーブル（例: 東京↔神奈川, 大阪↔京都）
fn is_adjacent_prefecture(a: &str, b: &str) -> bool {
    const ADJACENT_PAIRS: &[(&str, &str)] = &[
        ("東京都", "神奈川県"),
        ("東京都", "埼玉県"),
        ("東京都", "千葉県"),
        ("神奈川県", "埼玉県"),
        ("神奈川県", "千葉県"),
        ("大阪府", "京都府"),
        ("大阪府", "兵庫県"),
        ("大阪府", "奈良県"),
        ("愛知県", "岐阜県"),
        ("愛知県", "三重県"),
    ];
    ADJACENT_PAIRS
        .iter()
        .any(|(x, y)| (a == *x && b == *y) || (a == *y && b == *x))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(pref: Option<&str>, area: Option<&str>, remote: Option<&str>) -> Project {
        Project {
            work_todofuken: pref.map(|s| s.to_string()),
            work_area: area.map(|s| s.to_string()),
            remote_onsite: remote.map(|s| s.to_string()),
        }
    }

    fn talent(pref: Option<&str>, area: Option<&str>) -> Talent {
        Talent {
            residential_todofuken: pref.map(|s| s.to_string()),
            residential_area: area.map(|s| s.to_string()),
        }
    }

    #[test]
    fn full_remote_passes() {
        let result = evaluate_location(
            &project(Some("東京都"), None, Some("フルリモート")),
            &talent(Some("大阪府"), None),
        );
        assert!(matches!(result.ko_decision, KoDecision::Pass));
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn adjacent_prefecture_soft_score() {
        let result = evaluate_location(
            &project(Some("東京都"), None, None),
            &talent(Some("神奈川県"), None),
        );
        assert!(matches!(result.ko_decision, KoDecision::Pass));
        assert!(result.score > 0.6);
    }

    #[test]
    fn area_only_fallback() {
        let result = evaluate_location(
            &project(None, Some("関東"), None),
            &talent(None, Some("関西")),
        );
        assert!(matches!(result.ko_decision, KoDecision::SoftKo { .. }));
        assert!(result.score < 0.5);
    }

    #[test]
    fn derives_area_from_prefecture_when_missing() {
        let result = evaluate_location(
            &project(Some("東京都"), None, None),
            &talent(None, Some("関東")),
        );

        assert!(matches!(result.ko_decision, KoDecision::Pass));
        assert!(result.score > 0.7);
    }

    #[test]
    fn defaults_remote_to_hybrid_when_absent() {
        let result = evaluate_location(
            &project(Some("東京都"), None, None),
            &talent(Some("神奈川県"), None),
        );

        assert!(matches!(result.ko_decision, KoDecision::Pass));
        assert!(result.details.contains("リモート併用"));
    }
}
