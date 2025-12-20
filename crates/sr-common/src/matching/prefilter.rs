use std::cmp::Ordering;

use super::{ko_checks::run_all_ko_checks, scoring::calculate_prefilter_score};
use crate::{matching::ko_unified::KnockoutResultV2, matching::scoring::MatchScore, Project, Talent};

#[derive(Debug, Clone)]
pub struct PreFilterConfig {
    /// 返却する候補の最大数（スコア降順で切り詰め）
    pub max_candidates: usize,
    /// 通過させる最小スコア（0.0〜1.0）。これ未満は除外。
    pub min_score: f64,
}

impl Default for PreFilterConfig {
    fn default() -> Self {
        Self {
            max_candidates: 500,
            min_score: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrefilterCandidate {
    pub project: Project,
    pub score: f64,
    pub ko: KnockoutResultV2,
    pub match_score: MatchScore,
}

pub struct EnhancedPreFilter {
    config: PreFilterConfig,
}

impl EnhancedPreFilter {
    pub fn new(config: PreFilterConfig) -> Self {
        Self { config }
    }

    pub fn default() -> Self {
        Self::new(PreFilterConfig::default())
    }

    /// KO 判定と prefilter スコアをまとめて実行し、合格した場合のみ返す
    pub fn evaluate_candidate(
        &self,
        talent: &Talent,
        project: &Project,
    ) -> Option<PrefilterCandidate> {
        let ko = run_all_ko_checks(project, talent);
        if ko.is_hard_knockout {
            return None;
        }

        let match_score = calculate_prefilter_score(project, talent);
        if match_score.total <= self.config.min_score {
            return None;
        }

        Some(PrefilterCandidate {
            project: project.clone(),
            score: match_score.total,
            ko,
            match_score,
        })
    }

    /// 事前フィルタリング（候補絞り込み）
    pub fn filter_candidates(
        &self,
        talent: &Talent,
        projects: &[Project],
    ) -> Vec<PrefilterCandidate> {
        let mut candidates: Vec<_> = projects
            .iter()
            .filter_map(|project| self.evaluate_candidate(talent, project))
            .collect();

        candidates.sort_by(|a, b| match b
            .score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
        {
            Ordering::Equal => Ordering::Equal,
            other => other,
        });
        candidates.truncate(self.config.max_candidates);
        candidates
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
            required_skills_keywords: vec!["Rust".into(), "AWS".into()],
            min_experience_years: Some(3),
            contract_type: Some("業務委託".into()),
            project_keywords: Some(vec!["fast".into()]),
            ..Project::default()
        }
    }

    fn base_talent() -> Talent {
        Talent {
            desired_price_min: Some(80),
            residential_todofuken: Some("東京都".into()),
            residential_area: Some("関東".into()),
            possessed_skills_keywords: vec!["rust".into(), "aws".into()],
            min_experience_years: Some(5),
            primary_contract_type: Some("業務委託".into()),
            ..Talent::default()
        }
    }

    #[test]
    fn filters_out_hard_ko_projects() {
        let filter = EnhancedPreFilter::default();
        let mut talent = base_talent();
        talent.ng_keywords = Some(vec!["fast".into()]);

        let accepted = filter.filter_candidates(&talent, &[base_project()]);
        assert!(accepted.is_empty());
    }

    #[test]
    fn returns_candidates_sorted_with_limit() {
        let mut low_project = base_project();
        low_project.monthly_tanka_max = Some(90);

        let mut high_project = base_project();
        high_project.monthly_tanka_max = Some(150);

        let filter = EnhancedPreFilter::new(PreFilterConfig {
            max_candidates: 1,
            min_score: 0.0,
        });

        let results = filter.filter_candidates(
            &base_talent(),
            &[low_project.clone(), high_project.clone()],
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project, high_project);
        assert!(results[0].score >= 0.1);
    }

    #[test]
    fn keeps_softko_candidates_with_scores() {
        let filter = EnhancedPreFilter::default();
        let mut project = base_project();
        project.required_skills_keywords.clear();

        let results = filter.filter_candidates(&base_talent(), &[project.clone()]);
        assert_eq!(results.len(), 1);
        assert!(results[0].ko.needs_manual_review);
        assert!(results[0].score > 0.1);
    }
}
