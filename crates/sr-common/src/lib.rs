pub mod api;
pub mod calculation;
pub mod corrections;
pub mod date;
pub mod db;
pub mod extraction;
pub mod matching;
pub mod normalize;
pub mod queue;
pub mod schema;
pub mod skill_normalizer;

use date::NormalizedStartDate;

// Commonly used data models for matching functions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Project {
    pub id: Option<i64>,
    pub work_todofuken: Option<String>,
    pub work_area: Option<String>,
    pub remote_onsite: Option<String>,
    pub work_station: Option<String>,
    pub onsite_frequency: Option<f32>,
    pub settlement_range: Option<String>,
    pub interviews_count: Option<i32>,
    pub hiring_headcount: Option<i32>,
    pub monthly_tanka_min: Option<u32>,
    pub monthly_tanka_max: Option<u32>,
    pub required_skills_keywords: Vec<String>,
    pub preferred_skills_keywords: Vec<String>,
    pub min_experience_years: Option<i32>,
    pub japanese_skill: Option<String>,
    pub english_skill: Option<String>,
    pub contract_type: Option<String>,
    pub flow_dept: Option<String>,
    pub project_type: Option<Vec<String>>,
    pub jinzai_flow_limit: Option<String>,
    pub is_kojin_ok: Option<bool>,
    pub tech_kubun: Option<String>,
    pub project_keywords: Option<Vec<String>>,
    pub age_limit_lower: Option<i32>,
    pub age_limit_upper: Option<i32>,
    pub foreigner_allowed: Option<bool>,
    pub start_date: Option<NormalizedStartDate>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Talent {
    pub id: Option<i64>,
    pub residential_todofuken: Option<String>,
    pub residential_area: Option<String>,
    pub desired_price_min: Option<u32>,
    pub possessed_skills_keywords: Vec<String>,
    pub min_experience_years: Option<i32>,
    pub japanese_skill: Option<String>,
    pub english_skill: Option<String>,
    pub gender: Option<String>,
    pub primary_contract_type: Option<String>,
    pub secondary_contract_type: Option<String>,
    pub flow_depth: Option<String>,
    pub nearest_station: Option<String>,
    pub desired_remote_onsite: Option<String>,
    pub ng_keywords: Option<Vec<String>>,
    pub birth_year: Option<i32>,
    pub nationality: Option<String>,
    pub availability_date: Option<NormalizedStartDate>,
}
