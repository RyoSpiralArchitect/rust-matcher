use tracing::instrument;

use crate::api::conversion::{ConversionRequest, ConversionResponse, ConversionSource};
use crate::db::util::TimedClientExt;
use crate::db::{db_error, validated_actor, PgPool};

db_error!(ConversionStorageError {
    #[error("conversion actor is missing")]
    MissingActor,
});

#[instrument(skip(pool, actor, request))]
pub async fn insert_conversion_event(
    pool: &PgPool,
    actor: &str,
    request: &ConversionRequest,
) -> Result<ConversionResponse, ConversionStorageError> {
    let actor = validated_actor(actor).ok_or(ConversionStorageError::MissingActor)?;

    let source = request
        .source
        .as_ref()
        .map(AsRef::as_ref)
        .unwrap_or(ConversionSource::Gui.as_ref());

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
        .timed_query_one(
            &stmt,
            &[
                &request.interaction_id,
                &request.talent_id,
                &request.project_id,
                &request.stage.as_ref(),
                &actor,
                &source,
                &request.meta,
            ],
            "insert_conversion_event",
        )
        .await?;

    Ok(ConversionResponse {
        id: row.get("id"),
        status: "created".to_string(),
    })
}
