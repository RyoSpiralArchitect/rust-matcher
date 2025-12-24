use crate::{Project, Talent};

/// 重み付きトークン
#[derive(Debug, Clone)]
pub struct WeightedToken {
    pub token: String,
    pub weight: f32,
}

impl WeightedToken {
    pub fn new(token: impl Into<String>, weight: f32) -> Self {
        Self {
            token: token.into(),
            weight,
        }
    }
}

/// トークン形式（共通トークン方式）
/// - skill:<normalized>        (スキル - Project/Talent 共通)
/// - loc:<todofuken>           (都道府県 - 共通)
/// - loc:area:<area>           (エリア)
/// - loc:station:<station>     (駅)
/// - remote:<type>             (リモート形態)
/// - exp:<bucket>              (経験年数バケット)
/// - contract:<type>           (契約形態)
/// - tanka:<bucket>            (単価バケット)
/// - lang:ja:<level>           (日本語レベル)
/// - lang:en:<level>           (英語レベル)
pub fn tokenize_project(project: &Project) -> Vec<WeightedToken> {
    let mut tokens = Vec::new();

    for skill in &project.required_skills_keywords {
        tokens.push(WeightedToken::new(
            format!("skill:{}", skill.to_lowercase()),
            2.0,
        ));
    }
    for skill in &project.preferred_skills_keywords {
        tokens.push(WeightedToken::new(
            format!("skill:{}", skill.to_lowercase()),
            1.0,
        ));
    }

    if let Some(ref pref) = project.work_todofuken {
        tokens.push(WeightedToken::new(format!("loc:{}", pref), 1.5));
    }
    if let Some(ref area) = project.work_area {
        tokens.push(WeightedToken::new(format!("loc:area:{}", area), 1.0));
    }
    if let Some(ref station) = project.work_station {
        tokens.push(WeightedToken::new(format!("loc:station:{}", station), 0.5));
    }

    if let Some(ref remote) = project.remote_onsite {
        tokens.push(WeightedToken::new(format!("remote:{}", remote), 1.5));
    }

    if let Some(years) = project.min_experience_years {
        tokens.push(WeightedToken::new(
            format!("exp:{}", exp_years_bucket(years)),
            1.0,
        ));
    }

    if let Some(ref contract) = project.contract_type {
        tokens.push(WeightedToken::new(format!("contract:{}", contract), 1.0));
    }

    if let Some(max_tanka) = project.monthly_tanka_max {
        tokens.push(WeightedToken::new(
            format!("tanka:{}", tanka_bucket(max_tanka)),
            1.0,
        ));
    }

    if let Some(ref ja) = project.japanese_skill {
        tokens.push(WeightedToken::new(format!("lang:ja:{}", ja), 1.0));
    }
    if let Some(ref en) = project.english_skill {
        tokens.push(WeightedToken::new(format!("lang:en:{}", en), 1.0));
    }

    tokens
}

pub fn tokenize_talent(talent: &Talent) -> Vec<WeightedToken> {
    let mut tokens = Vec::new();

    for skill in &talent.possessed_skills_keywords {
        tokens.push(WeightedToken::new(
            format!("skill:{}", skill.to_lowercase()),
            1.0,
        ));
    }

    if let Some(ref pref) = talent.residential_todofuken {
        tokens.push(WeightedToken::new(format!("loc:{}", pref), 1.5));
    }
    if let Some(ref area) = talent.residential_area {
        tokens.push(WeightedToken::new(format!("loc:area:{}", area), 1.0));
    }
    if let Some(ref station) = talent.nearest_station {
        tokens.push(WeightedToken::new(format!("loc:station:{}", station), 0.5));
    }

    if let Some(ref remote) = talent.desired_remote_onsite {
        tokens.push(WeightedToken::new(format!("remote:{}", remote), 1.5));
    }

    if let Some(years) = talent.min_experience_years {
        tokens.push(WeightedToken::new(
            format!("exp:{}", exp_years_bucket(years)),
            1.0,
        ));
    }

    if let Some(ref contract) = talent.primary_contract_type {
        tokens.push(WeightedToken::new(format!("contract:{}", contract), 1.0));
    }

    if let Some(min_price) = talent.desired_price_min {
        tokens.push(WeightedToken::new(
            format!("tanka:{}", tanka_bucket(min_price)),
            1.0,
        ));
    }

    if let Some(ref ja) = talent.japanese_skill {
        tokens.push(WeightedToken::new(format!("lang:ja:{}", ja), 1.0));
    }
    if let Some(ref en) = talent.english_skill {
        tokens.push(WeightedToken::new(format!("lang:en:{}", en), 1.0));
    }

    tokens
}

/// 経験年数バケット: 0-2, 3-5, 6-10, 11+
fn exp_years_bucket(years: i32) -> &'static str {
    match years {
        0..=2 => "0-2",
        3..=5 => "3-5",
        6..=10 => "6-10",
        _ => "11+",
    }
}

/// 単価バケット: 30以下, 30-50, 50-70, 70-100, 100+（万円）
fn tanka_bucket(tanka: u32) -> &'static str {
    match tanka {
        0..=299_999 => "under30",
        300_000..=499_999 => "30-50",
        500_000..=699_999 => "50-70",
        700_000..=999_999 => "70-100",
        _ => "100+",
    }
}
