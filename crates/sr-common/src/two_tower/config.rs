#[derive(Debug, Clone)]
pub struct TwoTowerConfig {
    /// 埋め込み次元数（2のべき乗推奨: 256, 512, 1024）
    pub dimension: usize,
    /// Two-Tower スコアの重み（total_score 計算時）
    pub weight: f32,
    /// 有効/無効フラグ
    pub enabled: bool,
}

impl Default for TwoTowerConfig {
    fn default() -> Self {
        Self {
            dimension: 256,
            weight: 0.0,
            enabled: false,
        }
    }
}
