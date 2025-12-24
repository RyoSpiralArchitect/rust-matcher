use super::{Embedding, EmbeddingSource, TwoTowerEmbedder};
use crate::{Project, Talent};

/// Candle (Rust-native) を使用した Two-Tower（スタブ）
///
/// Phase 4+ でPyTorchモデルをRustに移植
pub struct CandleTwoTower {
    dimension: usize,
}

impl CandleTwoTower {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl TwoTowerEmbedder for CandleTwoTower {
    fn name(&self) -> &'static str {
        "candle"
    }

    fn version(&self) -> &str {
        "v1"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed_project(&self, _project: &Project) -> Embedding {
        // Phase 4+: Candle 推論
        Embedding {
            vector: vec![0.0; self.dimension],
            source: EmbeddingSource::Project,
            created_at: chrono::Utc::now(),
        }
    }

    fn embed_talent(&self, _talent: &Talent) -> Embedding {
        // Phase 4+: Candle 推論
        Embedding {
            vector: vec![0.0; self.dimension],
            source: EmbeddingSource::Talent,
            created_at: chrono::Utc::now(),
        }
    }
}
