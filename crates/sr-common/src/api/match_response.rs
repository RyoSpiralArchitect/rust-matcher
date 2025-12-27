use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::matching::ko_unified::{KoDecision, MatchResult, ScoreBreakdown as CoreScoreBreakdown};

/// GUI向けマッチング結果レスポンス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResponse {
    /// タレントID
    pub talent_id: i64,
    /// 案件ID
    pub project_id: i64,
    /// インタラクションID（feedback紐付け用）
    pub interaction_id: i64,

    // === 判定結果 ===
    /// 自動マッチ推奨（HardKoなし & score > threshold）
    pub auto_match_eligible: bool,
    /// 手動レビュー必要（SoftKo or 閾値ギリギリ）
    pub manual_review_required: bool,

    // === スコア ===
    /// 最終スコア（0.0〜1.0）
    pub score: f64,
    /// スコア内訳
    pub score_breakdown: ScoreBreakdown,
    /// Two-Tower スコア（有効時のみ）
    pub two_tower_score: Option<f64>,

    // === KO判定 ===
    /// KO判定の詳細（チェック名 → KoDecision）
    pub ko_decisions: HashMap<String, KoDecisionDto>,
    /// 表示用KO理由（整形済み）
    pub ko_reasons: Vec<String>,

    // === 説明 ===
    /// 各項目の詳細説明
    pub details: MatchDetails,

    // === メタデータ ===
    pub engine_version: String,
    pub rule_version: String,
    pub matched_at: DateTime<Utc>,
}

impl MatchResponse {
    /// KO判定からauto_match_eligibleを判定
    pub fn is_auto_match_eligible(&self, config: &MatchConfig) -> bool {
        !self.ko_decisions.values().any(|d| d.ko_type == "hard_ko")
            && self.score >= config.auto_match_threshold
            && !self.manual_review_required
    }

    /// 閾値ギリギリかどうか（手動レビュー推奨）
    pub fn is_near_threshold(&self, config: &MatchConfig) -> bool {
        let lower = config.auto_match_threshold - config.manual_review_margin;
        let upper = config.auto_match_threshold + config.manual_review_margin;
        self.score >= lower && self.score <= upper
    }

    /// 既存の`MatchResult`からGUI用レスポンスを構築する
    pub fn from_match_result(
        interaction_id: i64,
        talent_id: i64,
        project_id: i64,
        result: &MatchResult,
        matched_at: DateTime<Utc>,
        config: &MatchConfig,
    ) -> Self {
        let mut response = Self {
            talent_id,
            project_id,
            interaction_id,
            auto_match_eligible: result.auto_match_eligible,
            manual_review_required: result.manual_review_required,
            score: result.score,
            score_breakdown: ScoreBreakdown::from(&result.score_breakdown),
            two_tower_score: None,
            ko_decisions: result
                .ko_decisions
                .iter()
                .map(|(name, decision)| (name.to_string(), KoDecisionDto::from(decision)))
                .collect(),
            ko_reasons: result.ko_reasons.clone(),
            details: MatchDetails::default(),
            engine_version: "unknown".to_string(),
            rule_version: config
                .rule_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            matched_at,
        };

        if response.is_near_threshold(config) {
            response.manual_review_required = true;
        }

        response.auto_match_eligible = response.is_auto_match_eligible(config);
        response
    }
}

/// スコア内訳
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoreBreakdown {
    /// 単価スコア（0.0〜1.0）
    pub tanka: f64,
    /// ロケーションスコア（0.0〜1.0）
    pub location: f64,
    /// スキルスコア（0.0〜1.0）
    pub skills: f64,
    /// 経験年数スコア（0.0〜1.0）
    pub experience: f64,
    /// 契約形態スコア（0.0〜1.0）
    pub contract: f64,
    /// ビジネスルール総合（prefilter用）
    pub business_total: f64,
}

impl From<&CoreScoreBreakdown> for ScoreBreakdown {
    fn from(value: &CoreScoreBreakdown) -> Self {
        Self {
            tanka: value.tanka,
            location: value.location,
            skills: value.skills,
            experience: value.experience,
            contract: value.contract,
            business_total: value.total(),
        }
    }
}

/// KO判定DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KoDecisionDto {
    /// KOタイプ: "hard_ko" / "soft_ko" / "pass"
    pub ko_type: String,
    /// KO理由（nullならPass）
    pub reason: Option<String>,
    /// 詳細説明
    pub details: Option<String>,
}

impl From<&KoDecision> for KoDecisionDto {
    fn from(value: &KoDecision) -> Self {
        match value {
            KoDecision::HardKo { reason } => Self {
                ko_type: "hard_ko".into(),
                reason: Some(reason.clone()),
                details: None,
            },
            KoDecision::SoftKo { reason } => Self {
                ko_type: "soft_ko".into(),
                reason: Some(reason.clone()),
                details: None,
            },
            KoDecision::Pass => Self {
                ko_type: "pass".into(),
                reason: None,
                details: None,
            },
        }
    }
}

/// 詳細説明
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatchDetails {
    /// ロケーション詳細（例: "東京都 → 東京都（一致）"）
    pub location: Option<String>,
    /// スキルマッチ詳細（例: "3/5 必須スキル一致"）
    pub skills: Option<String>,
    /// 単価詳細（例: "希望60万 vs 案件50-70万（範囲内）"）
    pub tanka: Option<String>,
    /// 経験詳細（例: "5年 >= 3年（要件満たす）"）
    pub experience: Option<String>,
    /// 契約詳細（例: "業務委託 ⊂ {業務委託,派遣}（OK）"）
    pub contract: Option<String>,
    /// フロー詳細（例: "2次請け <= 3次請けまで（OK）"）
    pub flow: Option<String>,
    /// 日本語詳細
    pub japanese: Option<String>,
    /// 英語詳細
    pub english: Option<String>,
    /// 年齢詳細（例: "35歳 ∈ [25, 45]（OK）"）
    pub age: Option<String>,
    /// 国籍詳細
    pub nationality: Option<String>,
    /// 稼働開始詳細
    pub availability: Option<String>,
}

/// マッチング設定（環境変数から読み込み）
#[derive(Debug, Clone)]
pub struct MatchConfig {
    /// 自動マッチ推奨の閾値（デフォルト: 0.7）
    pub auto_match_threshold: f64,
    /// 手動レビュー推奨のマージン（閾値±margin で manual_review_required = true）
    pub manual_review_margin: f64,
    /// 期待する rule_version（None の場合はフィルタしない）
    pub rule_version: Option<String>,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            auto_match_threshold: 0.7,
            manual_review_margin: 0.1,
            rule_version: None,
        }
    }
}

impl MatchConfig {
    /// 環境変数から設定を読み込み
    pub fn from_env() -> Self {
        Self::from_env_checked().unwrap_or_default()
    }

    pub fn from_env_checked() -> Result<Self, String> {
        fn parse_env_f64(key: &str, default: f64) -> Result<f64, String> {
            match std::env::var(key) {
                Ok(raw) => raw.parse::<f64>().map_err(|_| {
                    format!("{key} must be a number between 0.0 and 1.0 (got `{raw}`)")
                }),
                Err(std::env::VarError::NotPresent) => Ok(default),
                Err(err) => Err(format!("failed to read {key}: {err}")),
            }
        }

        let auto_match_threshold = parse_env_f64("AUTO_MATCH_THRESHOLD", 0.7)?;
        let manual_review_margin = parse_env_f64("MANUAL_REVIEW_MARGIN", 0.1)?;
        let rule_version = std::env::var("MATCH_RULE_VERSION").ok();

        if !(0.0..=1.0).contains(&auto_match_threshold) {
            return Err(format!(
                "AUTO_MATCH_THRESHOLD must be between 0.0 and 1.0 (got {auto_match_threshold})"
            ));
        }
        if !(0.0..=1.0).contains(&manual_review_margin) {
            return Err(format!(
                "MANUAL_REVIEW_MARGIN must be between 0.0 and 1.0 (got {manual_review_margin})"
            ));
        }

        Ok(Self {
            auto_match_threshold,
            manual_review_margin,
            rule_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::ko_unified::{
        KoDecision, MatchResult, ScoreBreakdown as CoreScoreBreakdown,
    };
    use serial_test::serial;
    use std::sync::Mutex;

    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn with_env(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        let _guard = ENV_GUARD.lock().unwrap();

        let previous: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(key, value)| {
                let old = std::env::var(key).ok();
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
                (key.to_string(), old)
            })
            .collect();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

        for (key, value) in previous {
            if let Some(v) = value {
                std::env::set_var(&key, v);
            } else {
                std::env::remove_var(&key);
            }
        }

        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    }

    #[test]
    #[serial]
    fn from_env_checked_validates_numbers_and_ranges() {
        with_env(
            &[
                ("AUTO_MATCH_THRESHOLD", Some("not-a-number")),
                ("MANUAL_REVIEW_MARGIN", Some("0.2")),
            ],
            || {
                let err = MatchConfig::from_env_checked().unwrap_err();
                assert!(err.contains("AUTO_MATCH_THRESHOLD"));
            },
        );

        with_env(
            &[
                ("AUTO_MATCH_THRESHOLD", Some("0.8")),
                ("MANUAL_REVIEW_MARGIN", Some("2.5")),
            ],
            || {
                let err = MatchConfig::from_env_checked().unwrap_err();
                assert!(err.contains("MANUAL_REVIEW_MARGIN"));
            },
        );

        with_env(
            &[
                ("AUTO_MATCH_THRESHOLD", Some("0.6")),
                ("MANUAL_REVIEW_MARGIN", Some("0.05")),
                ("MATCH_RULE_VERSION", Some("v2025-01")),
            ],
            || {
                let cfg = MatchConfig::from_env_checked().expect("config should parse");
                assert_eq!(cfg.auto_match_threshold, 0.6);
                assert_eq!(cfg.manual_review_margin, 0.05);
                assert_eq!(cfg.rule_version.as_deref(), Some("v2025-01"));
            },
        );
    }

    #[test]
    fn builds_response_from_match_result() {
        let ko_decisions = vec![
            (
                "location",
                KoDecision::SoftKo {
                    reason: "要確認".into(),
                },
            ),
            ("skills", KoDecision::Pass),
        ];

        let result = MatchResult::from_ko_checks(
            ko_decisions.clone(),
            0.82,
            CoreScoreBreakdown {
                tanka: 0.2,
                location: 0.3,
                skills: 0.1,
                experience: 0.15,
                contract: 0.05,
            },
        );

        let matched_at = Utc::now();
        let config = MatchConfig::default();
        let response = MatchResponse::from_match_result(100, 10, 20, &result, matched_at, &config);

        assert_eq!(response.talent_id, 10);
        assert_eq!(response.project_id, 20);
        assert_eq!(response.interaction_id, 100);
        assert!(response.manual_review_required);
        assert_eq!(response.score, 0.82);
        assert_eq!(response.two_tower_score, None);
        assert_eq!(response.ko_decisions.len(), ko_decisions.len());
        assert_eq!(response.matched_at, matched_at);
        assert_eq!(response.score_breakdown.business_total, 0.8);
    }

    #[test]
    fn auto_match_helpers_respect_config() {
        let mut response = MatchResponse {
            interaction_id: 1,
            talent_id: 1,
            project_id: 2,
            auto_match_eligible: true,
            manual_review_required: false,
            score: 0.72,
            score_breakdown: ScoreBreakdown::default(),
            two_tower_score: None,
            ko_decisions: HashMap::new(),
            ko_reasons: vec![],
            details: MatchDetails::default(),
            engine_version: "v1".into(),
            rule_version: "v1".into(),
            matched_at: Utc::now(),
        };

        let config = MatchConfig::default();
        assert!(response.is_auto_match_eligible(&config));

        response.score = 0.65;
        assert!(response.is_near_threshold(&config));
    }

    #[test]
    fn near_threshold_results_require_manual_review() {
        let result = MatchResult::from_ko_checks(
            vec![("skills", KoDecision::Pass)],
            0.69,
            CoreScoreBreakdown {
                tanka: 0.2,
                location: 0.2,
                skills: 0.2,
                experience: 0.1,
                contract: 0.1,
            },
        );

        let config = MatchConfig::default();
        let response = MatchResponse::from_match_result(10, 1, 2, &result, Utc::now(), &config);

        assert!(response.manual_review_required);
        assert!(!response.auto_match_eligible);
    }
}
