use tracing::instrument;

use crate::api::interaction_event::{
    InteractionEventRequest, InteractionEventResponse, InteractionEventSource,
    InteractionEventStatus, InteractionEventType,
};
use crate::db::{db_error, PgPool};

db_error!(InteractionEventStorageError {
    #[error("interaction event actor is missing")]
    MissingActor,
});

#[instrument(skip(pool, actor, request))]
pub async fn insert_interaction_event(
    pool: &PgPool,
    actor: &str,
    request: &InteractionEventRequest,
) -> Result<InteractionEventResponse, InteractionEventStorageError> {
    let actor = actor.trim();
    if actor.is_empty() {
        return Err(InteractionEventStorageError::MissingActor);
    }

    let source = request
        .source
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or(InteractionEventSource::Gui.as_str());

    let client = pool.get().await?;

    if request.event_type == InteractionEventType::Shortlisted {
        let update_stmt = client
            .prepare_cached(
                "UPDATE ses.interaction_events\
                 SET meta = $1, idempotency_key = $2, source = $3, created_at = NOW()\
                 WHERE interaction_id = $4 AND actor = $5 AND event_type = 'shortlisted'\
                 RETURNING id",
            )
            .await?;

        if let Some(row) = client
            .query_opt(
                &update_stmt,
                &[
                    &request.meta,
                    &request.idempotency_key,
                    &source,
                    &request.interaction_id,
                    &actor,
                ],
            )
            .await?
        {
            return Ok(InteractionEventResponse {
                id: row.get("id"),
                status: InteractionEventStatus::Updated,
            });
        }
    }

    let insert_stmt = client
        .prepare_cached(
            "INSERT INTO ses.interaction_events (\
                interaction_id,\
                event_type,\
                actor,\
                source,\
                idempotency_key,\
                meta\
            ) VALUES (\
                $1, $2, $3, $4, $5, $6\
            )\
            ON CONFLICT (idempotency_key) DO UPDATE\
            SET meta = EXCLUDED.meta,\
                source = EXCLUDED.source,\
                created_at = NOW()\
            RETURNING id, xmax = 0 AS inserted",
        )
        .await?;

    let row = client
        .query_one(
            &insert_stmt,
            &[
                &request.interaction_id,
                &request.event_type.as_str(),
                &actor,
                &source,
                &request.idempotency_key,
                &request.meta,
            ],
        )
        .await?;

    let inserted: bool = row.get("inserted");
    let status = if inserted {
        InteractionEventStatus::Created
    } else {
        InteractionEventStatus::Updated
    };

    Ok(InteractionEventResponse {
        id: row.get("id"),
        status,
    })
}
