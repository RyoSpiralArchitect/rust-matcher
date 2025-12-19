/// KO判定結果（唯一の正）
#[derive(Debug, Clone, PartialEq)]
pub enum KoDecision {
    /// 確実にKO（即時除外、スコアリング不要）
    /// 例: 利益5万円未満、商流制限違反、スキルマッチ率<30%
    HardKo { reason: String },

    /// 要手動確認（スコアリングは行うが manual_review=true）
    /// 例: required_skills が空、日本語スキル不明
    SoftKo { reason: String },

    /// 問題なし（次のチェックへ）
    Pass,
}

impl KoDecision {
    /// HardKo の場合のみ true
    pub fn is_hard_ko(&self) -> bool {
        matches!(self, KoDecision::HardKo { .. })
    }

    /// SoftKo の場合のみ true（manual_review が必要）
    pub fn is_soft_ko(&self) -> bool {
        matches!(self, KoDecision::SoftKo { .. })
    }

    /// manual_review が必要な場合 true（= is_soft_ko のエイリアス）
    pub fn needs_manual_review(&self) -> bool {
        self.is_soft_ko()
    }

    /// KO reason を取得（Pass の場合は None）
    pub fn reason(&self) -> Option<&str> {
        match self {
            KoDecision::HardKo { reason } => Some(reason),
            KoDecision::SoftKo { reason } => Some(reason),
            KoDecision::Pass => None,
        }
    }

    /// KO かどうか（HardKo または SoftKo）
    pub fn is_ko(&self) -> bool {
        !matches!(self, KoDecision::Pass)
    }
}

/// 複合KO判定結果（全チェック項目を集約）
pub struct KnockoutResultV2 {
    /// いずれかが HardKo なら true
    pub is_hard_knockout: bool,
    /// SoftKo が1つ以上あれば true
    pub needs_manual_review: bool,
    /// 全ての判定結果（チェック名, 判定）
    pub decisions: Vec<(&'static str, KoDecision)>,
}

impl KnockoutResultV2 {
    pub fn new(decisions: Vec<(&'static str, KoDecision)>) -> Self {
        let is_hard_knockout = decisions.iter().any(|(_, d)| d.is_hard_ko());
        let needs_manual_review = decisions.iter().any(|(_, d)| d.needs_manual_review());

        Self {
            is_hard_knockout,
            needs_manual_review,
            decisions,
        }
    }

    /// manual_review_reason を生成（SoftKo の理由を ; 区切りで連結）
    pub fn manual_review_reasons(&self) -> Option<String> {
        let soft_reasons: Vec<_> = self
            .decisions
            .iter()
            .filter_map(|(name, d)| {
                if let KoDecision::SoftKo { reason } = d {
                    Some(format!("{}: {}", name, reason))
                } else {
                    None
                }
            })
            .collect();

        if soft_reasons.is_empty() {
            None
        } else {
            Some(soft_reasons.join("; "))
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub tanka: f64,
    pub location: f64,
    pub skills: f64,
    pub experience: f64,
    pub contract: f64,
}

impl ScoreBreakdown {
    pub fn total(&self) -> f64 {
        self.tanka + self.location + self.skills + self.experience + self.contract
    }
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    /// マッチングスコア（0.0〜1.0）
    pub score: f64,

    /// 詳細スコア内訳
    pub score_breakdown: ScoreBreakdown,

    /// 自動マッチ可能か（HardKo/SoftKo があれば false）
    pub auto_match_eligible: bool,

    /// 手動レビューが必要か（SoftKo があれば true）
    pub manual_review_required: bool,

    /// KO理由リスト（HardKo/SoftKo の reason を集約）
    pub ko_reasons: Vec<String>,

    /// 詳細な判定結果（チェック名, 判定）
    pub ko_decisions: Vec<(&'static str, KoDecision)>,
}

impl MatchResult {
    /// KO判定結果からMatchResultを構築
    ///
    /// 【重要な挙動】
    /// - HardKo が1つでもあれば: auto_match_eligible = false, スコアは0.0に上書き
    /// - SoftKo が1つでもあれば: auto_match_eligible = false, manual_review_required = true
    /// - 全て Pass: auto_match_eligible = true
    pub fn from_ko_checks(
        ko_decisions: Vec<(&'static str, KoDecision)>,
        score: f64,
        score_breakdown: ScoreBreakdown,
    ) -> Self {
        let has_hard_ko = ko_decisions.iter().any(|(_, d)| d.is_hard_ko());
        let has_soft_ko = ko_decisions.iter().any(|(_, d)| d.needs_manual_review());

        let ko_reasons: Vec<String> = ko_decisions
            .iter()
            .filter_map(|(name, d)| d.reason().map(|r| format!("[{}] {}", name, r)))
            .collect();

        // HardKo があればスコアは意味がないので 0.0
        let final_score = if has_hard_ko { 0.0 } else { score };

        Self {
            score: final_score,
            score_breakdown,
            auto_match_eligible: !has_hard_ko && !has_soft_ko,
            manual_review_required: has_soft_ko,
            ko_reasons,
            ko_decisions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_result_converts_soft_and_hard_ko() {
        let ko = vec![
            (
                "location",
                KoDecision::SoftKo {
                    reason: "遠方".into(),
                },
            ),
            (
                "flow",
                KoDecision::HardKo {
                    reason: "制限超過".into(),
                },
            ),
        ];

        let result = MatchResult::from_ko_checks(
            ko.clone(),
            0.8,
            ScoreBreakdown {
                tanka: 0.1,
                location: 0.2,
                skills: 0.3,
                experience: 0.1,
                contract: 0.1,
            },
        );

        assert!(!result.auto_match_eligible);
        assert!(result.ko_reasons.iter().any(|r| r.contains("location")));
        assert!(result.ko_reasons.iter().any(|r| r.contains("flow")));
        assert_eq!(result.score, 0.0); // hard KO zeroes score
        assert!(result.manual_review_required);
        assert_eq!(result.ko_decisions.len(), ko.len());
    }
}
