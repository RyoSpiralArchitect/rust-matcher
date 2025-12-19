use crate::matching::ko_unified::KoDecision;

/// 商流depth（0=エンド直, 1=1次, 2=2次, ...）
pub type FlowDepth = u8;

/// 案件側: flow_dept 文字列 → depth
pub fn parse_project_flow_depth(flow_dept: &str) -> Option<FlowDepth> {
    match flow_dept.trim() {
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
    match flow_depth.trim() {
        "SPONTO直人材" => Some(0),
        "1社先" => Some(1),
        "2社先" => Some(2),
        "3社先以上" => Some(3),
        _ => None,
    }
}

/// 案件側: 人材商流制限 → 許容最大depth
/// ⚠️ DDL ses.jinzai_flow_limit_enum は3値: SPONTO直人材, SPONTO一社先まで, 商流制限なし
/// #4修正: 2025-12-18 存在しない「SPONTO二社先まで」を削除
pub fn parse_flow_limit(jinzai_flow_limit: &str) -> Option<FlowDepth> {
    match jinzai_flow_limit.trim() {
        "SPONTO直人材" => Some(0),         // 直人材のみ
        "SPONTO一社先まで" => Some(1),      // 1次まで
        "商流制限なし" => Some(u8::MAX),    // 制限なし
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
        let decision = check_flow_ko(Some(2), Some(1));
        assert!(decision.is_hard_ko());
    }
}
