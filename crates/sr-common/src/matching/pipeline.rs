use std::{cmp::Ordering, collections::HashMap};

use super::{
    ko_checks::run_all_ko_checks,
    ko_unified::KnockoutResultV2,
    prefilter::{EnhancedPreFilter, PreFilterConfig, PrefilterCandidate},
    scoring::{
        calculate_detailed_score, calculate_total_score_with_two_tower, MatchScore, MatchingConfig,
    },
};
use crate::{
    db::{insert_interaction_log, InteractionLogInsert, InteractionLogStorageError, PgPool},
    run_id,
    two_tower::{create_embedder, load_config_from_env, TwoTowerConfig, TwoTowerEmbedder},
    Project, Talent,
};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct RankedMatch {
    pub project: Project,
    pub ko: KnockoutResultV2,
    pub prefilter_score: MatchScore,
    pub detailed_score: MatchScore,
}

#[derive(Debug, Clone)]
pub struct RankedTalentMatch {
    pub talent: Talent,
    pub project: Project,
    pub ko: KnockoutResultV2,
    pub prefilter_score: MatchScore,
    pub detailed_score: MatchScore,
    pub total_score: f64,
    pub two_tower_score: Option<f64>,
    pub two_tower_embedder: Option<String>,
    pub two_tower_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MatchingEngineConfig {
    pub prefilter: PreFilterConfig,
}

impl Default for MatchingEngineConfig {
    fn default() -> Self {
        Self {
            prefilter: PreFilterConfig::default(),
        }
    }
}

pub struct MatchingEngine {
    prefilter: EnhancedPreFilter,
}

impl MatchingEngine {
    pub fn new(config: MatchingEngineConfig) -> Self {
        Self {
            prefilter: EnhancedPreFilter::new(config.prefilter),
        }
    }

    pub fn default() -> Self {
        Self::new(MatchingEngineConfig::default())
    }

    /// Prefilter で粗選別した上で詳細スコアを再計算し、総合スコア順に並べる
    pub fn rank_projects(&self, talent: &Talent, projects: &[Project]) -> Vec<RankedMatch> {
        let candidates = self.prefilter.filter_candidates(talent, projects);

        let mut ranked: Vec<_> = candidates
            .into_iter()
            .map(|candidate| self.build_ranked_match(talent, candidate))
            .collect();

        ranked.sort_by(|a, b| {
            match b
                .detailed_score
                .total
                .partial_cmp(&a.detailed_score.total)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Equal => Ordering::Equal,
                other => other,
            }
        });

        ranked
    }

    fn build_ranked_match(&self, talent: &Talent, candidate: PrefilterCandidate) -> RankedMatch {
        let detailed_score = calculate_detailed_score(&candidate.project, talent);

        RankedMatch {
            project: candidate.project,
            ko: candidate.ko,
            prefilter_score: candidate.match_score,
            detailed_score,
        }
    }

    /// KO 判定のみを先に確認したい場合のユーティリティ
    pub fn evaluate_ko(&self, project: &Project, talent: &Talent) -> KnockoutResultV2 {
        run_all_ko_checks(project, talent)
    }

    /// Prefilter + detailed score に加えて Two-Tower 類似度を組み合わせ、案件に対する人材をランキングする
    pub fn rank_talents_for_project(
        &self,
        project: &Project,
        talents: &[Talent],
        two_tower: Option<&dyn TwoTowerEmbedder>,
        two_tower_config: &TwoTowerConfig,
    ) -> Vec<RankedTalentMatch> {
        let mut two_tower_scores: HashMap<i64, f64> = HashMap::new();
        let mut embedder_meta: Option<(&'static str, String)> = None;

        if let Some(embedder) = two_tower {
            embedder_meta = Some((embedder.name(), embedder.version().to_string()));
            for (talent_id, score) in embedder.rank_talents(project, talents) {
                two_tower_scores.insert(talent_id, score as f64);
            }
        }

        let total_weights = MatchingConfig::detailed().total_score_weights;
        let mut ranked = Vec::new();

        for talent in talents {
            let Some(candidate) = self.prefilter.evaluate_candidate(talent, project) else {
                continue;
            };

            let detailed_score = calculate_detailed_score(project, talent);
            let business_score = detailed_score.total;
            let tt_score = talent.id.and_then(|id| two_tower_scores.get(&id).copied());
            let normalized_tt_score = tt_score.map(|s| two_tower_config.normalize_score(s));

            let total_score = calculate_total_score_with_two_tower(
                business_score,
                0.0,
                0.0,
                normalized_tt_score,
                &total_weights,
                two_tower_config,
            );

            ranked.push(RankedTalentMatch {
                talent: talent.clone(),
                project: candidate.project,
                ko: candidate.ko,
                prefilter_score: candidate.match_score,
                detailed_score,
                total_score,
                two_tower_score: normalized_tt_score,
                two_tower_embedder: embedder_meta.as_ref().map(|m| m.0.to_string()),
                two_tower_version: embedder_meta.as_ref().map(|m| m.1.clone()),
            });
        }

        ranked.sort_by(|a, b| {
            match b
                .total_score
                .partial_cmp(&a.total_score)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Equal => Ordering::Equal,
                other => other,
            }
        });

        ranked
    }
}

impl RankedTalentMatch {
    /// interaction_logs へ INSERT するためのレコードを組み立てる
    pub fn to_interaction_log(
        &self,
        match_run_id: impl Into<String>,
        match_result_id: Option<i64>,
        engine_version: Option<String>,
        config_version: Option<String>,
        variant: Option<String>,
    ) -> Option<InteractionLogInsert> {
        let talent_id = self.talent.id?;
        let project_id = self.project.id?;

        Some(InteractionLogInsert {
            match_result_id,
            talent_id,
            project_id,
            match_run_id: match_run_id.into(),
            engine_version,
            config_version,
            two_tower_score: self.two_tower_score,
            two_tower_embedder: self.two_tower_embedder.clone(),
            two_tower_version: self.two_tower_version.clone(),
            business_score: Some(self.detailed_score.total),
            outcome: None,
            feedback_at: None,
            variant,
            created_at: None,
        })
    }
}

/// 環境変数から Two-Tower を初期化するヘルパー。
/// enabled=false の場合は embedder を生成せず None を返す。
pub fn init_two_tower_from_env() -> (TwoTowerConfig, Option<Box<dyn TwoTowerEmbedder>>) {
    let config = load_config_from_env();
    if !config.enabled {
        return (config, None);
    }

    let embedder_name = std::env::var("TWO_TOWER_EMBEDDER").unwrap_or_else(|_| "hash".into());
    let embedder = create_embedder(&embedder_name, config.clone());
    (config, Some(embedder))
}

/// マッチングを実行し、Two-Tower スコアと business_score を interaction_logs へ保存するランナー
pub struct MatchRunner {
    engine: MatchingEngine,
    two_tower_config: TwoTowerConfig,
    two_tower: Option<Box<dyn TwoTowerEmbedder>>,
    engine_version: Option<String>,
    config_version: Option<String>,
    variant: Option<String>,
    match_run_id: String,
}

impl MatchRunner {
    /// 環境変数を読み込んで初期化する。
    /// - match_run_id: sr_common::run_id の生成 ULID（runごとに一意）
    /// - Two-Tower: TWO_TOWER_ENABLED/TWO_TOWER_DIMENSION/TWO_TOWER_WEIGHT/TWO_TOWER_EMBEDDER で制御
    pub fn from_env() -> Self {
        let (two_tower_config, two_tower) = init_two_tower_from_env();

        Self {
            engine: MatchingEngine::default(),
            two_tower_config,
            two_tower,
            engine_version: None,
            config_version: None,
            variant: None,
            match_run_id: run_id::generate(),
        }
    }

    pub fn with_engine_version(mut self, value: impl Into<String>) -> Self {
        self.engine_version = Some(value.into());
        self
    }

    pub fn with_config_version(mut self, value: impl Into<String>) -> Self {
        self.config_version = Some(value.into());
        self
    }

    pub fn with_variant(mut self, value: impl Into<String>) -> Self {
        self.variant = Some(value.into());
        self
    }

    pub fn with_match_run_id(mut self, value: impl Into<String>) -> Self {
        self.match_run_id = value.into();
        self
    }

    /// ランキング結果を返す（永続化しない）
    pub fn rank_talents(&self, project: &Project, talents: &[Talent]) -> Vec<RankedTalentMatch> {
        self.engine.rank_talents_for_project(
            project,
            talents,
            self.two_tower.as_deref(),
            &self.two_tower_config,
        )
    }

    /// interaction_logs へ INSERT するためのレコードを構築する
    pub fn build_interaction_logs(
        &self,
        project: &Project,
        talents: &[Talent],
        match_result_ids: Option<&HashMap<i64, i64>>,
    ) -> Vec<InteractionLogInsert> {
        let ranked = self.rank_talents(project, talents);

        ranked
            .into_iter()
            .filter_map(|r| {
                let match_result_id = r
                    .talent
                    .id
                    .and_then(|id| match_result_ids.and_then(|m| m.get(&id).copied()));

                r.to_interaction_log(
                    self.match_run_id.clone(),
                    match_result_id,
                    self.engine_version.clone(),
                    self.config_version.clone(),
                    self.variant.clone(),
                )
            })
            .collect()
    }

    /// match_results 用の INSERT レコードを構築する（永続化は行わない）
    /// 戻り値: (talent_id, MatchResultInsert)
    pub fn build_match_result_inserts(
        &self,
        project: &Project,
        talents: &[Talent],
        match_result_ids: Option<&HashMap<i64, i64>>,
    ) -> Vec<(i64, crate::db::match_results::MatchResultInsert)> {
        let ranked = self.rank_talents(project, talents);

        ranked
            .into_iter()
            .filter_map(|r| {
                let talent_id = r.talent.id?;
                let project_id = r.project.id?;
                let _match_result_id = r
                    .talent
                    .id
                    .and_then(|id| match_result_ids.and_then(|m| m.get(&id).copied()));

                let ko_reasons = r.ko.prioritized_reasons();

                let score_breakdown =
                    build_score_breakdown_json(&r.detailed_score, r.total_score, r.two_tower_score);

                Some((
                    talent_id,
                    crate::db::match_results::MatchResultInsert {
                        talent_id,
                        project_id,
                        is_knockout: r.ko.is_hard_knockout,
                        ko_reasons: if ko_reasons.is_empty() {
                            None
                        } else {
                            Some(json!(ko_reasons))
                        },
                        needs_manual_review: r.ko.needs_manual_review,
                        score_total: Some(r.total_score),
                        score_breakdown: Some(score_breakdown),
                        engine_version: self.engine_version.clone(),
                        rule_version: self.config_version.clone(),
                        match_run_id: Some(self.match_run_id.clone()),
                        created_at: None,
                    },
                ))
            })
            .collect()
    }

    /// Two-Tower スコア・business_score を含む interaction_logs を保存する
    pub async fn insert_interaction_logs(
        &self,
        pool: &PgPool,
        project: &Project,
        talents: &[Talent],
        match_result_ids: Option<&HashMap<i64, i64>>,
    ) -> Result<Vec<InteractionLogInsert>, InteractionLogStorageError> {
        let logs = self.build_interaction_logs(project, talents, match_result_ids);
        for log in &logs {
            insert_interaction_log(pool, log).await?;
        }
        Ok(logs)
    }
}

fn build_score_breakdown_json(
    score: &MatchScore,
    total_score: f64,
    two_tower_score: Option<f64>,
) -> serde_json::Value {
    json!({
        "tanka": score.tanka.score,
        "location": score.location.score,
        "skills": score.skills.score,
        "experience": score.experience.score,
        "contract": score.contract.score,
        "other": score.other.score,
        "business_total": score.total,
        "semantic_score": score.semantic_score,
        "historical_score": score.historical_score,
        "two_tower_score": two_tower_score,
        "total": total_score,
        "score_version": score.score_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Mutex;

    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn with_env(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        let _guard = ENV_GUARD.lock().unwrap();

        let previous: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(var, value)| {
                let old = std::env::var(var).ok();
                match value {
                    Some(v) => std::env::set_var(var, v),
                    None => std::env::remove_var(var),
                }
                (*var, old)
            })
            .collect();

        f();

        for (var, previous_value) in previous {
            match previous_value {
                Some(v) => std::env::set_var(var, v),
                None => std::env::remove_var(var),
            }
        }
    }

    fn base_project() -> Project {
        Project {
            monthly_tanka_max: Some(120),
            work_todofuken: Some("東京都".into()),
            work_area: Some("関東".into()),
            remote_onsite: Some("リモート併用".into()),
            required_skills_keywords: vec!["Rust".into()],
            preferred_skills_keywords: vec!["GraphQL".into()],
            min_experience_years: Some(3),
            contract_type: Some("業務委託".into()),
            ..Project::default()
        }
    }

    fn base_talent() -> Talent {
        Talent {
            desired_price_min: Some(80),
            residential_todofuken: Some("東京都".into()),
            residential_area: Some("関東".into()),
            possessed_skills_keywords: vec!["rust".into(), "graphql".into()],
            min_experience_years: Some(5),
            primary_contract_type: Some("業務委託".into()),
            ..Talent::default()
        }
    }

    #[test]
    fn ranks_candidates_by_detailed_score() {
        let engine = MatchingEngine::default();
        let mut weaker = base_project();
        weaker.monthly_tanka_max = Some(90);
        weaker.preferred_skills_keywords = vec![];

        let stronger = base_project();

        let results = engine.rank_projects(&base_talent(), &[weaker.clone(), stronger.clone()]);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].project, stronger);
        assert!(results[0].detailed_score.total >= results[1].detailed_score.total);
        assert!(results[0].prefilter_score.total >= results[1].prefilter_score.total);
    }

    #[test]
    fn filters_out_hard_ko_candidates() {
        let engine = MatchingEngine::default();
        let mut hard_ko = base_project();
        hard_ko.required_skills_keywords = vec!["Go".into()];

        let results = engine.rank_projects(&base_talent(), &[base_project(), hard_ko]);

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].project.required_skills_keywords,
            vec!["Rust".to_string()]
        );
    }

    #[test]
    fn keeps_softko_candidates_with_manual_review_flag() {
        let engine = MatchingEngine::default();
        let mut project = base_project();
        project.required_skills_keywords.clear();

        let results = engine.rank_projects(&base_talent(), &[project]);

        assert_eq!(results.len(), 1);
        assert!(results[0].ko.needs_manual_review);
        assert!(results[0].prefilter_score.total > 0.0);
    }

    #[test]
    fn ranks_talents_with_two_tower_metadata() {
        let engine = MatchingEngine::default();
        let mut project = base_project();
        project.id = Some(10);

        let mut strong_match = base_talent();
        strong_match.id = Some(1);
        strong_match.possessed_skills_keywords = vec!["rust".into(), "graphql".into()];

        let mut weaker_match = base_talent();
        weaker_match.id = Some(2);
        weaker_match.possessed_skills_keywords = vec!["rust".into()];

        let two_tower_config = TwoTowerConfig {
            enabled: true,
            weight: 0.2,
            ..Default::default()
        };
        let embedder = create_embedder("hash", two_tower_config.clone());

        let ranked = engine.rank_talents_for_project(
            &project,
            &[strong_match, weaker_match],
            Some(embedder.as_ref()),
            &two_tower_config,
        );

        assert_eq!(ranked.len(), 2);
        assert!(ranked.iter().all(|r| r.two_tower_score.is_some()));
        assert!(ranked
            .windows(2)
            .all(|w| w[0].total_score >= w[1].total_score));
        assert!(ranked
            .iter()
            .all(|r| r.two_tower_embedder.as_deref() == Some("hash")));
    }

    #[test]
    fn converts_ranked_talent_to_interaction_log() {
        let engine = MatchingEngine::default();
        let mut project = base_project();
        project.id = Some(20);

        let mut talent = base_talent();
        talent.id = Some(30);

        let two_tower_config = TwoTowerConfig {
            enabled: false,
            ..Default::default()
        };

        let ranked = engine.rank_talents_for_project(&project, &[talent], None, &two_tower_config);

        assert_eq!(ranked.len(), 1);
        let log = ranked[0]
            .to_interaction_log(
                "run-1",
                Some(99),
                Some("engine_v1".into()),
                Some("cfg_v1".into()),
                Some("two_tower_10pct".into()),
            )
            .expect("ids should be present");

        assert_eq!(log.talent_id, 30);
        assert_eq!(log.project_id, 20);
        assert_eq!(log.match_run_id, "run-1");
        assert_eq!(log.match_result_id, Some(99));
        assert_eq!(log.two_tower_score, None);
        assert_eq!(log.business_score, Some(ranked[0].detailed_score.total));
        assert_eq!(log.variant.as_deref(), Some("two_tower_10pct"));
    }

    #[test]
    #[serial]
    fn build_interaction_logs_includes_two_tower_scores_and_metadata() {
        with_env(
            &[
                ("TWO_TOWER_ENABLED", Some("1")),
                ("TWO_TOWER_DIMENSION", Some("128")),
                ("TWO_TOWER_WEIGHT", Some("0.2")),
                ("TWO_TOWER_EMBEDDER", Some("hash")),
            ],
            || {
                let mut project = base_project();
                project.id = Some(42);

                let mut talent_a = base_talent();
                talent_a.id = Some(1001);

                let mut talent_b = base_talent();
                talent_b.id = Some(1002);

                let runner = MatchRunner::from_env()
                    .with_engine_version("engine_v1")
                    .with_config_version("cfg_v1")
                    .with_variant("two_tower_10pct");

                let logs = runner.build_interaction_logs(&project, &[talent_a, talent_b], None);

                assert_eq!(logs.len(), 2);
                assert!(logs.iter().all(|l| l.two_tower_score.is_some()));
                assert!(logs
                    .iter()
                    .all(|l| l.two_tower_embedder.as_deref() == Some("hash")));
                assert!(logs.iter().all(|l| l.business_score.is_some()));
                assert!(logs.iter().all(|l| l.match_run_id.len() >= 26));
                assert!(logs
                    .iter()
                    .all(|l| l.engine_version.as_deref() == Some("engine_v1")));
                assert!(logs
                    .iter()
                    .all(|l| l.config_version.as_deref() == Some("cfg_v1")));
                assert!(logs
                    .iter()
                    .all(|l| l.variant.as_deref() == Some("two_tower_10pct")));
            },
        );
    }

    #[test]
    #[serial]
    fn build_interaction_logs_uses_match_result_ids_when_provided() {
        with_env(&[("TWO_TOWER_ENABLED", Some("1"))], || {
            let mut project = base_project();
            project.id = Some(77);

            let mut talent = base_talent();
            talent.id = Some(1234);

            let mut map = HashMap::new();
            map.insert(1234, 9999);

            let runner = MatchRunner::from_env();
            let logs = runner.build_interaction_logs(&project, &[talent], Some(&map));

            assert_eq!(logs.len(), 1);
            assert_eq!(logs[0].match_result_id, Some(9999));
        });
    }

    #[test]
    #[serial]
    fn build_match_result_inserts_populates_two_tower_and_business_scores() {
        with_env(&[("TWO_TOWER_ENABLED", Some("1"))], || {
            let mut project = base_project();
            project.id = Some(11);

            let mut talent = base_talent();
            talent.id = Some(22);

            let runner = MatchRunner::from_env()
                .with_engine_version("engine_v1")
                .with_config_version("rule_v1");

            let inserts = runner.build_match_result_inserts(&project, &[talent], None);
            assert_eq!(inserts.len(), 1);

            let (_, insert) = &inserts[0];
            assert_eq!(insert.talent_id, 22);
            assert_eq!(insert.project_id, 11);
            assert_eq!(insert.engine_version.as_deref(), Some("engine_v1"));
            assert_eq!(insert.rule_version.as_deref(), Some("rule_v1"));
            assert_eq!(insert.match_run_id.as_ref().unwrap().len(), 26);
            assert!(insert.score_total.is_some());
            assert!(insert.score_breakdown.is_some());
            let score_json = insert.score_breakdown.clone().unwrap();
            assert!(
                score_json
                    .get("two_tower_score")
                    .and_then(|v| v.as_f64())
                    .is_some(),
                "two_tower_score should be present"
            );
            assert!(
                score_json
                    .get("business_total")
                    .and_then(|v| v.as_f64())
                    .is_some(),
                "business_total should be present"
            );
        });
    }
}
