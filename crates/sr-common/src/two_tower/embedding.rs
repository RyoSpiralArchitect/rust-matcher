#[derive(Debug, Clone)]
pub struct Embedding {
    pub vector: Vec<f32>,
    pub dimension: usize,
    pub source: EmbeddingSource,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Embedding {
    pub fn new(vector: Vec<f32>, dimension: usize, source: EmbeddingSource) -> Self {
        Self {
            vector,
            dimension,
            source,
            created_at: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbeddingSource {
    Project,
    Talent,
}
