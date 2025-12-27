/// コサイン類似度（0.0〜1.0）
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        metrics::counter!(
            "two_tower_similarity_dimension_mismatch_total",
            "a_len" => a.len().to_string(),
            "b_len" => b.len().to_string(),
        )
        .increment(1);
        tracing::warn!(
            a_len = a.len(),
            b_len = b.len(),
            "embedding dimension mismatch; returning zero similarity"
        );
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        metrics::counter!("two_tower_similarity_zero_norm_total").increment(1);
        return 0.0;
    }

    // Clamp to [0, 1] for normalized similarity
    let similarity = ((dot / (norm_a * norm_b)) + 1.0) / 2.0;
    metrics::histogram!("two_tower_similarity_distribution", "source" => "two_tower")
        .record(similarity as f64);
    similarity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_returns_one_for_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let sim = cosine_similarity(&a, &b);

        assert!((sim - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cosine_similarity_handles_zero_vectors() {
        let a = vec![0.0, 0.0];
        let b = vec![0.0, 0.0];

        let sim = cosine_similarity(&a, &b);

        assert_eq!(sim, 0.0);
    }

    #[test]
    fn cosine_similarity_returns_zero_on_dimension_mismatch() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0];

        let sim = cosine_similarity(&a, &b);

        assert_eq!(sim, 0.0);
    }
}
