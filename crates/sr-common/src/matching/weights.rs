/// Prefilter重み（粗選別用）
/// 目的: False Negative を減らす（取りこぼさない）
/// → スキル重視（合わない人を残すより、合う人を逃さない）
pub const PREFILTER_WEIGHTS: Weights = Weights {
    tanka: 0.25,
    location: 0.15,
    skills: 0.40,     // 詳細より +0.00（other分を割当）
    experience: 0.10, // 詳細より -0.05
    contract: 0.05,
    other: 0.05,
};

/// Detailed重み（ランキング用）
/// 目的: False Positive を減らす（精度を上げる）
/// → 経験年数も重視（よりバランスの取れた評価）
pub const DETAILED_WEIGHTS: Weights = Weights {
    tanka: 0.25,
    location: 0.15,
    skills: 0.40,
    experience: 0.15,
    contract: 0.05,
    other: 0.0,
};

#[derive(Debug, Clone, Copy)]
pub struct Weights {
    pub tanka: f64,
    pub location: f64,
    pub skills: f64,
    pub experience: f64,
    pub contract: f64,
    pub other: f64,
}

impl Weights {
    pub fn sum(&self) -> f64 {
        self.tanka + self.location + self.skills + self.experience + self.contract + self.other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one() {
        let pre_sum = PREFILTER_WEIGHTS.sum();
        let detailed_sum = DETAILED_WEIGHTS.sum();
        assert!((pre_sum - 1.0).abs() < 1e-6);
        assert!((detailed_sum - 1.0).abs() < 1e-6);
    }
}
