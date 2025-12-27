pub mod candle_tower;
pub mod config;
pub mod embedding;
pub mod hash_tower;
pub mod onnx_tower;
pub mod similarity;
pub mod tokenizer;

use crate::{Project, Talent};
pub use candle_tower::CandleTwoTower;
pub use config::TwoTowerConfig;
pub use embedding::{Embedding, EmbeddingSource};
pub use hash_tower::HashTwoTower;
pub use onnx_tower::OnnxTwoTower;
pub use similarity::cosine_similarity;
use tracing::warn;

/// Two-Tower モデルの抽象インターフェース
///
/// 実装例:
/// - HashTwoTower: Feature Hashing（決定論的、学習不要）
/// - OnnxTwoTower: ONNX Runtime（学習済みモデル読み込み）
/// - CandleTwoTower: Candle（Rust-native推論）
///
/// interaction_logs には name() と version() が記録される。
pub trait TwoTowerEmbedder: Send + Sync {
    /// 実装名（"hash", "onnx", "candle"）
    /// → interaction_logs.two_tower_embedder
    fn name(&self) -> &'static str;

    /// バージョン情報（モデルの世代管理用）
    /// → interaction_logs.two_tower_version
    /// 例: "v1", "20241215", "hash-v2"
    fn version(&self) -> &str;

    /// 埋め込み次元数
    fn dimension(&self) -> usize;

    /// 案件を埋め込みベクトルに変換
    fn embed_project(&self, project: &Project) -> Embedding;

    /// 人材を埋め込みベクトルに変換
    fn embed_talent(&self, talent: &Talent) -> Embedding;

    /// 複数の人材を一括で埋め込み（デフォルト実装: ループ）
    /// ONNX/Candle ではバッチ推論でオーバーライド推奨
    fn embed_talents(&self, talents: &[Talent]) -> Vec<Embedding> {
        talents.iter().map(|t| self.embed_talent(t)).collect()
    }

    /// 2つの埋め込みベクトルの類似度（0.0〜1.0）
    fn similarity(&self, a: &Embedding, b: &Embedding) -> f32 {
        if a.dimension != b.dimension {
            warn!(
                source_a = ?a.source,
                source_b = ?b.source,
                a_dimension = a.dimension,
                b_dimension = b.dimension,
                a_vec_len = a.vector.len(),
                b_vec_len = b.vector.len(),
                "embedding dimension mismatch; returning zero similarity"
            );
            return 0.0;
        }
        cosine_similarity(&a.vector, &b.vector)
    }

    /// 複数の人材を案件に対してランキング
    /// バッチ推論（embed_talents）を使用して高速化
    ///
    /// **注意**: talent.id が None の場合は 0 を返す。
    /// 0 が有効な talent_id でないことを前提としている。
    fn rank_talents(&self, project: &Project, talents: &[Talent]) -> Vec<(i64, f32)> {
        let project_emb = self.embed_project(project);

        // バッチで全Talentを埋め込み（ONNX/Candleで効果的）
        let talent_embs = self.embed_talents(talents);

        // スコア計算
        let mut scores: Vec<_> = talents
            .iter()
            .zip(talent_embs.iter())
            .map(|(t, emb)| {
                let sim = self.similarity(&project_emb, emb);
                (t.id.unwrap_or(0), sim)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }
}

/// Two-Tower 実装のファクトリ
pub fn create_embedder(name: &str, config: TwoTowerConfig) -> Box<dyn TwoTowerEmbedder> {
    match name {
        "hash" => Box::new(HashTwoTower::new(config)),
        "onnx" => {
            let model_path = std::env::var("TWO_TOWER_ONNX_PATH")
                .unwrap_or_else(|_| "models/two_tower.onnx".into());
            match OnnxTwoTower::new(&model_path, config.dimension) {
                Ok(tower) => Box::new(tower),
                Err(_) => Box::new(HashTwoTower::new(config)),
            }
        }
        "candle" => Box::new(CandleTwoTower::new(config.dimension)),
        _ => Box::new(HashTwoTower::new(config)),
    }
}

/// 環境変数から Two-Tower 設定を読み込み
pub fn load_config_from_env() -> TwoTowerConfig {
    TwoTowerConfig {
        dimension: std::env::var("TWO_TOWER_DIMENSION")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(256),
        weight: std::env::var("TWO_TOWER_WEIGHT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        enabled: std::env::var("TWO_TOWER_ENABLED")
            .ok()
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false),
        score_min: std::env::var("TWO_TOWER_SCORE_MIN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        score_max: std::env::var("TWO_TOWER_SCORE_MAX")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0),
    }
}
