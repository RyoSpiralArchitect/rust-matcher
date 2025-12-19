pub mod calculation;
pub mod corrections;
pub mod date;
pub mod extraction;
pub mod matching;
pub mod normalize;
pub mod queue;
pub mod schema;
pub mod skill_normalizer;

// Commonly used data models for matching functions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Project {
    pub work_todofuken: Option<String>,
    pub work_area: Option<String>,
    pub remote_onsite: Option<String>,
    pub monthly_tanka_max: Option<u32>,
    pub required_skills_keywords: Vec<String>,
    pub japanese_skill: Option<String>,
    pub english_skill: Option<String>,
    pub contract_type: Option<String>,
    pub tech_kubun: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Talent {
    pub residential_todofuken: Option<String>,
    pub residential_area: Option<String>,
    pub desired_price_min: Option<u32>,
    pub possessed_skills_keywords: Vec<String>,
    pub japanese_skill: Option<String>,
    pub english_skill: Option<String>,
    pub primary_contract_type: Option<String>,
    pub secondary_contract_type: Option<String>,
    pub flow_depth: Option<String>,
}
