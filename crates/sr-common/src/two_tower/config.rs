#[derive(Debug, Clone)]
pub struct TwoTowerConfig {
    /// 埋め込み次元数（2のべき乗推奨: 256, 512, 1024）
    pub dimension: usize,
    /// Two-Tower スコアの重み（total_score 計算時）
    pub weight: f32,
    /// 有効/無効フラグ
    pub enabled: bool,
    /// 生スコアを 0.0〜1.0 にマッピングするための最小値
    pub score_min: f32,
    /// 生スコアを 0.0〜1.0 にマッピングするための最大値
    pub score_max: f32,
}

impl Default for TwoTowerConfig {
    fn default() -> Self {
        Self {
            dimension: 256,
            weight: 0.0,
            enabled: false,
            score_min: 0.0,
            score_max: 1.0,
        }
    }
}

impl TwoTowerConfig {
    /// 生スコアを 0.0〜1.0 に正規化する。
    /// score_max <= score_min の場合は 0.0〜1.0 のクランプだけを行う。
    pub fn normalize_score(&self, raw: f64) -> f64 {
        let min = self.score_min as f64;
        let max = self.score_max as f64;

        if max <= min {
            return raw.clamp(0.0, 1.0);
        }

        ((raw - min) / (max - min)).clamp(0.0, 1.0)
    }
}
