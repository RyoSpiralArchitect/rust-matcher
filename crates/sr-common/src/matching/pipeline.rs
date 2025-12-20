use std::cmp::Ordering;

use super::{
    ko_checks::run_all_ko_checks,
    ko_unified::KnockoutResultV2,
    prefilter::{EnhancedPreFilter, PreFilterConfig, PrefilterCandidate},
    scoring::{calculate_detailed_score, MatchScore},
};
use crate::{Project, Talent};

#[derive(Debug, Clone)]
pub struct RankedMatch {
    pub project: Project,
    pub ko: KnockoutResultV2,
    pub prefilter_score: MatchScore,
    pub detailed_score: MatchScore,
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

        ranked.sort_by(|a, b| match b
            .detailed_score
            .total
            .partial_cmp(&a.detailed_score.total)
            .unwrap_or(Ordering::Equal)
        {
            Ordering::Equal => Ordering::Equal,
            other => other,
        });

        ranked
    }

    fn build_ranked_match(
        &self,
        talent: &Talent,
        candidate: PrefilterCandidate,
    ) -> RankedMatch {
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
        assert_eq!(results[0].project.required_skills_keywords, vec!["Rust".to_string()]);
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
}
