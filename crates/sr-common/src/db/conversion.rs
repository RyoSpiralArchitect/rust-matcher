use deadpool_postgres::PoolError;
use tokio_postgres::Error as PgError;
use tracing::instrument;

use crate::api::conversion::{ConversionRequest, ConversionResponse, ConversionSource};
use crate::db::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum ConversionStorageError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
    #[error("conversion actor is missing")]
    MissingActor,
}

#[instrument(skip(pool, actor, request))]
pub async fn insert_conversion_event(
    pool: &PgPool,
    actor: &str,
    request: &ConversionRequest,
) -> Result<ConversionResponse, ConversionStorageError> {
    let actor = actor.trim();
    if actor.is_empty() {
        return Err(ConversionStorageError::MissingActor);
    }

    let source = request
        .source
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or(ConversionSource::Gui.as_str());

    let client = pool.get().await?;

    let stmt = client
        .prepare_cached(
            "INSERT INTO ses.conversion_events (\
                interaction_id,\
                talent_id,\
                project_id,\
                stage,\
                actor,\
                source,\
                meta\
            ) VALUES (\
                $1, $2, $3, $4, $5, $6, $7\
            )\
            RETURNING id",
        )
        .await?;

    let row = client
        .query_one(
            &stmt,
            &[
                &request.interaction_id,
                &request.talent_id,
                &request.project_id,
                &request.stage.as_str(),
                &actor,
                &source,
                &request.meta,
            ],
        )
        .await?;

    Ok(ConversionResponse {
        id: row.get("id"),
        status: "created".to_string(),
    })
}
