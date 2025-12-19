use super::ko_unified::KoDecision;
use crate::{Project, Talent};

#[derive(Debug, Clone)]
pub struct LocationEvaluation {
    pub ko_decision: KoDecision,
    pub score: f64, // 0.0〜1.0
    pub details: String,
}

/// 【唯一の勤務地判定関数】
/// KO判定・prefilter・スコアリング全てがこの関数を呼ぶこと。
pub fn evaluate_location(project: &Project, talent: &Talent) -> LocationEvaluation {
    // 1. フルリモート → 勤務地KOなし、スコア1.0
    if project.remote_onsite.as_deref() == Some("フルリモート") {
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
        return evaluate_by_todofuken(p_pref, t_pref);
    }

    // 3. 片方でも都道府県がない → エリアで粗く判定
    if let (Some(p_area), Some(t_area)) = (
        project.work_area.as_deref(),
        talent.residential_area.as_deref(),
    ) {
        return evaluate_by_area(p_area, t_area);
    }

    // 4. どっちも取れない → SoftKo（手動レビューへ）
    LocationEvaluation {
        ko_decision: KoDecision::SoftKo {
            reason: "location_unknown: 勤務地情報不足のため要手動確認".into(),
        },
        score: 0.5, // 中立
        details: "勤務地情報なし - 手動確認必要".into(),
    }
}

fn evaluate_by_todofuken(project_pref: &str, talent_pref: &str) -> LocationEvaluation {
    if project_pref == talent_pref {
        // 同一都道府県
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: 1.0,
            details: format!("都道府県一致: {}", project_pref),
        }
    } else if is_adjacent_prefecture(project_pref, talent_pref) {
        // 隣接都道府県（通勤圏内）
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: 0.7,
            details: format!("隣接都道府県: {} ↔ {}", talent_pref, project_pref),
        }
    } else {
        // 遠隔（HardKoではなくSoftKo: リモート併用なら通えるかも）
        LocationEvaluation {
            ko_decision: KoDecision::SoftKo {
                reason: format!(
                    "location_distant: {} → {} は通勤困難の可能性",
                    talent_pref, project_pref
                ),
            },
            score: 0.2,
            details: format!("都道府県不一致: {} ≠ {}", talent_pref, project_pref),
        }
    }
}

fn evaluate_by_area(project_area: &str, talent_area: &str) -> LocationEvaluation {
    if project_area == talent_area {
        LocationEvaluation {
            ko_decision: KoDecision::Pass,
            score: 0.8, // 都道府県より粗いので0.8
            details: format!("エリア一致: {}", project_area),
        }
    } else {
        LocationEvaluation {
            ko_decision: KoDecision::SoftKo {
                reason: format!("area_mismatch: {} ≠ {}", talent_area, project_area),
            },
            score: 0.3,
            details: format!("エリア不一致: {} ≠ {}", talent_area, project_area),
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
}
