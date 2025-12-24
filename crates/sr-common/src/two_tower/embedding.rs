#[derive(Debug, Clone)]
pub struct Embedding {
    pub vector: Vec<f32>,
    pub source: EmbeddingSource,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbeddingSource {
    Project,
    Talent,
}
