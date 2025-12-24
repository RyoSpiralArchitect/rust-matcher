use std::{cmp::Ordering, collections::HashMap};

use super::{
    ko_checks::run_all_ko_checks,
    ko_unified::KnockoutResultV2,
    prefilter::{EnhancedPreFilter, PreFilterConfig, PrefilterCandidate},
    scoring::{
        MatchScore, MatchingConfig, calculate_detailed_score, calculate_total_score_with_two_tower,
    },
};
use crate::{
    Project, Talent,
    db::interaction_logs::InteractionLogInsert,
    two_tower::{TwoTowerConfig, TwoTowerEmbedder, create_embedder, load_config_from_env},
};

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
    pub two_tower_score: Option<f32>,
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
        let mut two_tower_scores: HashMap<i64, f32> = HashMap::new();
        let mut embedder_meta: Option<(&'static str, String)> = None;

        if let Some(embedder) = two_tower {
            embedder_meta = Some((embedder.name(), embedder.version().to_string()));
            for (talent_id, score) in embedder.rank_talents(project, talents) {
                two_tower_scores.insert(talent_id, score);
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
            let tt_score = talent.id.and_then(|id| two_tower_scores.get(&id).cloned());

            let total_score = calculate_total_score_with_two_tower(
                business_score,
                0.0,
                0.0,
                tt_score,
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
                two_tower_score: tt_score,
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
            two_tower_score: self.two_tower_score.map(|v| v as f64),
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(ranked.windows(2).all(|w| w[0].total_score >= w[1].total_score));
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
}
