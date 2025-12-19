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
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Talent {
    pub residential_todofuken: Option<String>,
    pub residential_area: Option<String>,
}
