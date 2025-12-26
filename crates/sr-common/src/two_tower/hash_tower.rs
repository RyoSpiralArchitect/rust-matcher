use super::{Embedding, EmbeddingSource, TwoTowerConfig, TwoTowerEmbedder, tokenizer};
use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};

/// 固定 seed（決定論的 hash のため）
/// ⚠️ この値を変更すると全 embedding が変わる → two_tower_version を上げること
const HASH_SEED_K0: u64 = 0x0123_4567_89ab_cdef;
const HASH_SEED_K1: u64 = 0xfedc_ba98_7654_3210;

/// Feature Hashing を用いた決定論的 Two-Tower
///
/// - 学習不要（固定ハッシュ関数）
/// - 高速（O(n) where n = token count）
/// - 重み付きトークンで必須/優遇を区別
/// - SipHash13 + 固定 seed で Rust バージョン間の安定性を保証
pub struct HashTwoTower {
    pub config: TwoTowerConfig,
}

impl HashTwoTower {
    pub fn new(config: TwoTowerConfig) -> Self {
        let mut cfg = config;
        cfg.dimension = cfg.dimension.max(1);
        Self { config: cfg }
    }

    /// トークンをハッシュして次元インデックスに変換
    /// SipHash13 + 固定 seed で決定論的に計算
    fn hash_token(&self, token: &str) -> usize {
        let mut hasher = SipHasher13::new_with_keys(HASH_SEED_K0, HASH_SEED_K1);
        token.hash(&mut hasher);
        (hasher.finish() as usize) % self.config.dimension
    }

    /// 重み付きトークン列を埋め込みベクトルに変換
    fn tokens_to_embedding(
        &self,
        tokens: Vec<tokenizer::WeightedToken>,
        source: EmbeddingSource,
    ) -> Embedding {
        let mut vector = vec![0.0f32; self.config.dimension];

        for wt in &tokens {
            let idx = self.hash_token(&wt.token);
            // Sign hashing: 偶数ハッシュ → +weight, 奇数ハッシュ → -weight
            let sign = if self.hash_token(&format!("{}_sign", wt.token)) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            vector[idx] += sign * wt.weight;
        }

        // L2正規化
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vector {
                *v /= norm;
            }
        }

        Embedding {
            vector,
            source,
            created_at: chrono::Utc::now(),
        }
    }
}

impl TwoTowerEmbedder for HashTwoTower {
    fn name(&self) -> &'static str {
        "hash"
    }

    fn version(&self) -> &str {
        // トークン設計やハッシュ関数が変わったらバージョンを上げる
        "v2"
    }

    fn dimension(&self) -> usize {
        self.config.dimension
    }

    fn embed_project(&self, project: &crate::Project) -> Embedding {
        let tokens = tokenizer::tokenize_project(project);
        self.tokens_to_embedding(tokens, EmbeddingSource::Project)
    }

    fn embed_talent(&self, talent: &crate::Talent) -> Embedding {
        let tokens = tokenizer::tokenize_talent(talent);
        self.tokens_to_embedding(tokens, EmbeddingSource::Talent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Project, Talent};

    #[test]
    fn hash_tower_produces_normalized_vectors() {
        let tower = HashTwoTower::new(TwoTowerConfig::default());

        let project = Project {
            required_skills_keywords: vec!["rust".into(), "python".into()],
            work_todofuken: Some("東京都".into()),
            ..Default::default()
        };

        let emb = tower.embed_project(&project);

        let norm: f32 = emb.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "L2 norm should be 1.0, got {}",
            norm
        );
    }

    #[test]
    fn similar_inputs_have_higher_similarity() {
        let tower = HashTwoTower::new(TwoTowerConfig::default());

        let project = Project {
            required_skills_keywords: vec!["rust".into(), "aws".into()],
            work_todofuken: Some("東京都".into()),
            ..Default::default()
        };

        let similar_talent = Talent {
            possessed_skills_keywords: vec!["rust".into(), "aws".into(), "docker".into()],
            residential_todofuken: Some("東京都".into()),
            ..Default::default()
        };

        let different_talent = Talent {
            possessed_skills_keywords: vec!["cobol".into(), "oracle".into()],
            residential_todofuken: Some("北海道".into()),
            ..Default::default()
        };

        let proj_emb = tower.embed_project(&project);
        let similar_emb = tower.embed_talent(&similar_talent);
        let different_emb = tower.embed_talent(&different_talent);

        let similar_score = tower.similarity(&proj_emb, &similar_emb);
        let different_score = tower.similarity(&proj_emb, &different_emb);

        assert!(
            similar_score > different_score,
            "Similar talent should have higher score: {} vs {}",
            similar_score,
            different_score
        );
    }

    #[test]
    fn required_skill_match_beats_preferred_skill_match() {
        let tower = HashTwoTower::new(TwoTowerConfig::default());

        let project = Project {
            required_skills_keywords: vec!["rust".into()],
            preferred_skills_keywords: vec!["python".into()],
            ..Default::default()
        };

        let required_match = Talent {
            possessed_skills_keywords: vec!["rust".into()],
            ..Default::default()
        };

        let preferred_match = Talent {
            possessed_skills_keywords: vec!["python".into()],
            ..Default::default()
        };

        let proj_emb = tower.embed_project(&project);
        let req_emb = tower.embed_talent(&required_match);
        let pref_emb = tower.embed_talent(&preferred_match);

        let req_score = tower.similarity(&proj_emb, &req_emb);
        let pref_score = tower.similarity(&proj_emb, &pref_emb);

        assert!(
            req_score > pref_score,
            "Required skill match should beat preferred: {} vs {}",
            req_score,
            pref_score
        );
    }
}
