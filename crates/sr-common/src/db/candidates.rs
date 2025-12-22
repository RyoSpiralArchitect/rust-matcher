use std::collections::HashMap;

use chrono::{DateTime, Utc};
use deadpool_postgres::PoolError;
use serde_json::Value;
use tokio_postgres::Error as PgError;
use tracing::instrument;

use crate::api::match_response::{
    KoDecisionDto, MatchConfig, MatchDetails, MatchResponse, ScoreBreakdown,
};
use crate::db::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum CandidateFetchError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
    #[error("failed to map candidate row: {0}")]
    Mapping(String),
}

fn parse_score_breakdown(value: Option<Value>) -> ScoreBreakdown {
    value
        .and_then(|v| serde_json::from_value::<ScoreBreakdown>(v).ok())
        .unwrap_or_default()
}

fn parse_ko_reasons(value: Option<Value>) -> Vec<String> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => vec![],
    }
}

fn derive_ko_decisions(is_knockout: bool, ko_reasons: &[String]) -> HashMap<String, KoDecisionDto> {
    let mut map = HashMap::new();

    if is_knockout {
        let reason = ko_reasons
            .first()
            .cloned()
            .unwrap_or_else(|| "knockout".to_string());

        map.insert(
            "knockout".to_string(),
            KoDecisionDto {
                ko_type: "hard_ko".into(),
                reason: Some(reason),
                details: None,
            },
        );

        return map;
    }

    for (idx, reason) in ko_reasons.iter().enumerate() {
        map.insert(
            format!("soft_ko_{idx}"),
            KoDecisionDto {
                ko_type: "soft_ko".into(),
                reason: Some(reason.clone()),
                details: None,
            },
        );
    }

    map
}

#[instrument(skip(pool, config))]
pub async fn fetch_candidates_for_project(
    pool: &PgPool,
    project_id: i64,
    include_softko: bool,
    config: &MatchConfig,
) -> Result<Vec<MatchResponse>, CandidateFetchError> {
    let client = pool.get().await?;

    let mut conditions = vec!["il.project_id = $1".to_string()];
    if !include_softko {
        conditions.push("mr.is_knockout = false".to_string());
        conditions.push("mr.needs_manual_review = false".to_string());
    }
    let where_clause = conditions.join(" AND ");

    let query = format!(
        "SELECT \
            il.id AS interaction_id,\
            mr.talent_id,\
            mr.project_id,\
            mr.needs_manual_review,\
            mr.is_knockout,\
            mr.score_total,\
            mr.score_breakdown,\
            mr.ko_reasons,\
            mr.engine_version,\
            mr.rule_version,\
            mr.created_at,\
            il.two_tower_score\
        FROM ses.interaction_logs il\
        JOIN ses.match_results mr ON mr.id = il.match_result_id\
        WHERE {where_clause}\
        ORDER BY mr.score_total DESC NULLS LAST, il.created_at DESC"
    );

    let rows = client.query(&query, &[&project_id]).await?;

    let responses = rows
        .into_iter()
        .map(|row| {
            let ko_reasons = parse_ko_reasons(row.get("ko_reasons"));
            let is_knockout: bool = row.get("is_knockout");

            let mut response = MatchResponse {
                interaction_id: row.get("interaction_id"),
                talent_id: row.get::<_, i32>("talent_id") as i64,
                project_id: row.get::<_, i32>("project_id") as i64,
                auto_match_eligible: false,
                manual_review_required: row.get("needs_manual_review"),
                score: row.get::<_, Option<f64>>("score_total").unwrap_or_default() as f32,
                score_breakdown: parse_score_breakdown(row.get("score_breakdown")),
                two_tower_score: row
                    .get::<_, Option<f64>>("two_tower_score")
                    .map(|v| v as f32),
                ko_decisions: derive_ko_decisions(is_knockout, &ko_reasons),
                ko_reasons,
                details: MatchDetails::default(),
                engine_version: row
                    .get::<_, Option<String>>("engine_version")
                    .unwrap_or_else(|| "unknown".into()),
                rule_version: row
                    .get::<_, Option<String>>("rule_version")
                    .unwrap_or_else(|| "unknown".into()),
                matched_at: row
                    .get::<_, Option<DateTime<Utc>>>("created_at")
                    .unwrap_or_else(Utc::now),
            };

            if response.score_breakdown.business_total == 0.0 && response.score > 0.0 {
                response.score_breakdown.business_total = response.score;
            }

            if is_knockout || !response.ko_reasons.is_empty() {
                response.manual_review_required = true;
            }

            if response.is_near_threshold(config) {
                response.manual_review_required = true;
            }

            response.auto_match_eligible =
                !is_knockout && response.is_auto_match_eligible(config);
            Ok(response)
        })
        .collect::<Result<Vec<_>, CandidateFetchError>>()?;

    Ok(responses)
}
