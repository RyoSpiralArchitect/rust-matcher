use crate::matching::ko_unified::KoDecision;

/// 商流depth（0=エンド直, 1=1次, 2=2次, ...）
pub type FlowDepth = u8;

/// 案件商流ENUM補正
pub fn correct_flow_dept(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "不明".to_string();
    }

    let valid = [
        "エンド直",
        "1次請け",
        "2次請け",
        "3次請け",
        "4次請け以上",
        "不明",
    ];
    if valid.contains(&trimmed) {
        return trimmed.to_string();
    }

    if trimmed.contains("エンド直") {
        return "エンド直".to_string();
    }
    if trimmed.contains("1次") || trimmed.contains("元請") {
        return "1次請け".to_string();
    }
    if trimmed.contains("2次") {
        return "2次請け".to_string();
    }
    if trimmed.contains("3次") {
        return "3次請け".to_string();
    }
    if trimmed.contains("4次") {
        return "4次請け以上".to_string();
    }

    "不明".to_string()
}

/// 人材商流ENUM補正
pub fn correct_talent_flow_depth(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let valid = ["1社先", "2社先", "3社先以上"];
    if valid.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    if trimmed.contains('3') || trimmed.contains('３') || trimmed.contains("以上") {
        return Some("3社先以上".to_string());
    }
    if trimmed.contains('2') || trimmed.contains('２') {
        return Some("2社先".to_string());
    }

    None
}

/// 商流制限ENUM補正
pub fn correct_jinzai_flow_limit(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let valid = ["SPONTO直人材", "SPONTO一社先まで", "商流制限なし"];
    if valid.contains(&trimmed) {
        return Some(trimmed.to_string());
    }

    let lowered = trimmed.to_lowercase();
    if lowered.contains("sponto") && lowered.contains("直") {
        return Some("SPONTO直人材".to_string());
    }
    if lowered.contains("sponto") && (lowered.contains("1社") || lowered.contains("一社")) {
        return Some("SPONTO一社先まで".to_string());
    }
    if trimmed.contains("貴社まで")
        || trimmed.contains("御社まで")
        || trimmed.contains("直人材")
        || trimmed.contains("貴社社員")
    {
        return Some("SPONTO直人材".to_string());
    }
    if trimmed.contains("制限なし") || trimmed.contains("不問") {
        return Some("商流制限なし".to_string());
    }

    None
}

/// 案件側: flow_dept 文字列 → depth
pub fn parse_project_flow_depth(flow_dept: &str) -> Option<FlowDepth> {
    match correct_flow_dept(flow_dept).as_str() {
        "エンド直" => Some(0),
        "1次請け" => Some(1),
        "2次請け" => Some(2),
        "3次請け" => Some(3),
        "4次請け以上" => Some(4),
        _ => None,
    }
}

/// 人材側: 商流位置 → depth（1社先=1次の位置）
pub fn parse_talent_flow_depth(flow_depth: &str) -> Option<FlowDepth> {
    if flow_depth
        .contains("直")
        || flow_depth.contains("自社")
        || flow_depth.contains("貴社直")
    {
        return Some(0);
    }

    correct_talent_flow_depth(flow_depth).and_then(|depth| match depth.as_str() {
        "1社先" => Some(1),
        "2社先" => Some(2),
        "3社先以上" => Some(3),
        _ => None,
    })
}

/// 案件側: 人材商流制限 → 許容最大depth
/// ⚠️ DDL ses.jinzai_flow_limit_enum は3値: SPONTO直人材, SPONTO一社先まで, 商流制限なし
/// #4修正: 2025-12-18 存在しない「SPONTO二社先まで」を削除
pub fn parse_flow_limit(jinzai_flow_limit: &str) -> Option<FlowDepth> {
    match correct_jinzai_flow_limit(jinzai_flow_limit)?.as_str() {
        "SPONTO直人材" => Some(0),       // 直人材のみ
        "SPONTO一社先まで" => Some(1),   // 1次まで
        "商流制限なし" => Some(u8::MAX), // 制限なし
        _ => None,
    }
}

/// 商流KO判定（depth ベース）
pub fn check_flow_ko(
    talent_depth: Option<FlowDepth>,
    project_limit: Option<FlowDepth>,
) -> KoDecision {
    match (talent_depth, project_limit) {
        (Some(t), Some(p)) if t > p => KoDecision::HardKo {
            reason: format!("flow_exceeded: 人材depth={} > 制限depth={}", t, p),
        },
        (Some(_), Some(_)) => KoDecision::Pass,
        _ => KoDecision::SoftKo {
            reason: "flow_unknown: 商流情報不足".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_depths_and_checks() {
        assert_eq!(parse_project_flow_depth("1次請け"), Some(1));
        assert_eq!(parse_talent_flow_depth("1社先"), Some(1));
        assert_eq!(parse_talent_flow_depth("貴社直"), Some(0));
        let decision = check_flow_ko(Some(2), Some(1));
        assert!(decision.is_hard_ko());
    }

    #[test]
    fn corrects_flow_enums() {
        assert_eq!(correct_flow_dept("元請案件"), "1次請け");
        assert_eq!(
            correct_talent_flow_depth("３社先"),
            Some("3社先以上".to_string())
        );
        assert_eq!(correct_jinzai_flow_limit("制限なし"), Some("商流制限なし".into()));
    }

    #[test]
    fn parse_flow_limit_includes_corrections() {
        assert_eq!(parse_flow_limit(" 制限なし"), Some(u8::MAX));
        assert_eq!(parse_flow_limit("sponto一社先まで"), Some(1));
        assert_eq!(parse_flow_limit("unknown"), None);
    }
}
