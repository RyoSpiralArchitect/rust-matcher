use super::{Embedding, EmbeddingSource, TwoTowerEmbedder};
use crate::{Project, Talent};

/// ONNX Runtime を使用した Two-Tower（スタブ）
///
/// Phase 4 で学習済みモデルを読み込む
pub struct OnnxTwoTower {
    _model_path: String,
    dimension: usize,
}

impl OnnxTwoTower {
    pub fn new(model_path: &str, dimension: usize) -> Result<Self, String> {
        Ok(Self {
            _model_path: model_path.to_string(),
            dimension,
        })
    }
}

impl TwoTowerEmbedder for OnnxTwoTower {
    fn name(&self) -> &'static str {
        "onnx"
    }

    fn version(&self) -> &str {
        // モデルファイル名やメタデータからバージョンを取得する設計
        // 例: "20241215" (学習日) or "v3.2"
        "v1"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed_project(&self, _project: &Project) -> Embedding {
        // Phase 4: ONNX 推論を実装
        Embedding::new(
            vec![0.0; self.dimension],
            self.dimension,
            EmbeddingSource::Project,
        )
    }

    fn embed_talent(&self, _talent: &Talent) -> Embedding {
        // Phase 4: ONNX 推論を実装
        Embedding::new(
            vec![0.0; self.dimension],
            self.dimension,
            EmbeddingSource::Talent,
        )
    }
}
