/// 単価計算の共通パラメータ
#[allow(non_snake_case)]
pub mod TankaParams {
    /// ベース単価（万円）- 人材・案件共通
    pub const BASE_TANKA: f64 = 35.0;

    /// 経験年数別加算（万円/年）
    pub const EXP_RATE_1_TO_5: f64 = 5.0; // 1-5年: +5万/年
    pub const EXP_RATE_6_TO_10: f64 = 4.0; // 6-10年: +4万/年
    pub const EXP_RATE_11_PLUS: f64 = 3.0; // 11年+: +3万/年
    pub const EXP_YEARS_CAP: i32 = 20; // 経験年数上限（20年でキャップ）

    /// スキルプレミアム（乗数）
    pub const PREMIUM_CLOUD: f64 = 0.15; // AWS/GCP/Azure: +15%
    pub const PREMIUM_PM_PMO: f64 = 0.15; // PM/PMO: +15%
    pub const PREMIUM_AI_ML: f64 = 0.10; // AI/ML/機械学習: +10%
    pub const PREMIUM_MODERN_DEV: f64 = 0.10; // Python/Go/TypeScript: +10%
    pub const PREMIUM_CAP: f64 = 1.5; // プレミアム上限: 最大1.5x

    /// 単価レンジ幅（万円）
    pub const RANGE_WIDTH: f64 = 5.0; // 最終単価 ±5万円

    /// 案件最低単価（万円）- 案件のみに適用
    pub const PROJECT_MIN_TANKA: i32 = 50;
    pub const PROJECT_MIN_TANKA_MAX: i32 = 55; // max側も同様にフロア
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PremiumCategory {
    Cloud,
    PmPmo,
    AiMl,
    ModernDev,
}

fn experience_addition(years: i32) -> (f64, Vec<String>) {
    use TankaParams::*;
    let capped_years = years.max(0).min(EXP_YEARS_CAP);
    let mut addition = 0.0;
    let mut details = Vec::new();

    for year in 1..=capped_years {
        let delta = if year <= 5 {
            EXP_RATE_1_TO_5
        } else if year <= 10 {
            EXP_RATE_6_TO_10
        } else {
            EXP_RATE_11_PLUS
        };
        addition += delta;
    }

    if capped_years > 0 {
        details.push(format!(
            "experience({}y): +{:.1}万円",
            capped_years, addition
        ));
    }

    (addition, details)
}

fn premium_multiplier(categories: &[PremiumCategory]) -> (f64, Vec<String>) {
    use TankaParams::*;
    let mut multiplier = 1.0;
    let mut details = Vec::new();

    for cat in categories {
        let (label, pct) = match cat {
            PremiumCategory::Cloud => ("cloud", PREMIUM_CLOUD),
            PremiumCategory::PmPmo => ("pm_pmo", PREMIUM_PM_PMO),
            PremiumCategory::AiMl => ("ai_ml", PREMIUM_AI_ML),
            PremiumCategory::ModernDev => ("modern_dev", PREMIUM_MODERN_DEV),
        };
        multiplier *= 1.0 + pct;
        details.push(format!("{}:+{:.0}%", label, pct * 100.0));
    }

    if multiplier > PREMIUM_CAP {
        details.push(format!("cap:{:.1}x", PREMIUM_CAP));
        multiplier = PREMIUM_CAP;
    }

    (multiplier, details)
}

fn calculate_range(final_tanka: f64) -> (i32, i32) {
    use TankaParams::RANGE_WIDTH;
    let min_tanka = (final_tanka - RANGE_WIDTH).floor() as i32;
    let max_tanka = (final_tanka + RANGE_WIDTH).ceil() as i32;
    (min_tanka, max_tanka)
}

/// 人材単価計算（フロアなし）
pub fn calculate_talent_tanka(
    experience_years: i32,
    premium_categories: &[PremiumCategory],
) -> (i32, i32, String) {
    use TankaParams::*;
    let mut logic = Vec::new();

    let (exp_addition, exp_logic) = experience_addition(experience_years);
    logic.extend(exp_logic);
    let base = BASE_TANKA + exp_addition;

    let (multiplier, premium_logic) = premium_multiplier(premium_categories);
    logic.extend(premium_logic);

    let final_tanka = base * multiplier;
    let (min_tanka, max_tanka) = calculate_range(final_tanka);

    (min_tanka, max_tanka, logic.join("; "))
}

/// 案件単価計算（最低単価フロア適用）
pub fn calculate_project_tanka(
    experience_years: i32,
    premium_categories: &[PremiumCategory],
) -> (i32, i32, String) {
    use TankaParams::*;
    let (min_tanka, max_tanka, logic) =
        calculate_talent_tanka(experience_years, premium_categories);

    (
        min_tanka.max(PROJECT_MIN_TANKA),
        max_tanka.max(PROJECT_MIN_TANKA_MAX),
        logic,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn talent_tanka_has_no_floor() {
        let (min_tanka, max_tanka, _) = calculate_talent_tanka(0, &[]);
        assert_eq!(min_tanka, 30);
        assert_eq!(max_tanka, 40);
    }

    #[test]
    fn project_tanka_applies_floor() {
        let (min_tanka, max_tanka, _) = calculate_project_tanka(0, &[]);
        assert_eq!(min_tanka, TankaParams::PROJECT_MIN_TANKA);
        assert_eq!(max_tanka, TankaParams::PROJECT_MIN_TANKA_MAX);
    }

    #[test]
    fn premium_multiplier_caps() {
        let (min_tanka, max_tanka, logic) = calculate_project_tanka(
            15,
            &[
                PremiumCategory::Cloud,
                PremiumCategory::PmPmo,
                PremiumCategory::AiMl,
                PremiumCategory::ModernDev,
            ],
        );

        assert!(logic.contains("cap"));
        // base: 35 + 5*5 + 4*5 + 3*5 = 35 + 25 + 20 + 15 = 95
        // multiplier capped at 1.5
        assert_eq!(min_tanka, 137); // floor((95*1.5)-5) = 137
        assert_eq!(max_tanka, 148); // ceil((95*1.5)+5) = 148
    }
}
