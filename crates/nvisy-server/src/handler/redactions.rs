//! Redaction-pass HTTP handlers.
//!
//! | Method   | Path                       | Description                          |
//! |----------|----------------------------|--------------------------------------|
//! | `POST`   | `/redactions`              | Submit a redaction pass              |
//! | `GET`    | `/redactions`              | List redactions (filterable)         |
//! | `GET`    | `/redactions/{id}`         | Get a redaction snapshot             |
//! | `POST`   | `/redactions/{id}/cancel`  | Cancel an in-progress redaction      |
//! | `DELETE` | `/redactions/{id}`         | Delete a finished redaction          |
//! | `DELETE` | `/redactions`              | Delete all finished redactions       |

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_document::pipeline::{Engine, RedactionFilter, RedactionSnapshot};

use super::request::{NewRedaction, RedactionPath, RedactionQuery};
use super::response::{RedactionId, RedactionList};
use crate::extract::{ActorId, Json, Path};
use crate::handler::error::Result;
use crate::middleware::{DEFAULT_PIPELINE_TIMEOUT, DEFAULT_READ_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::redactions";

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn create_redaction(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewRedaction>,
) -> Result<(StatusCode, Json<RedactionId>)> {
    let input = req.into_engine_input(actor_id);
    let id = engine.redact(input).await?;
    tracing::info!(target: TARGET, %id, "redaction pass submitted");
    Ok((StatusCode::ACCEPTED, Json(RedactionId { id })))
}

fn create_redaction_docs(op: TransformOperation) -> TransformOperation {
    op.id("createRedaction")
        .tag("redactions")
        .summary("Submit a redaction pass")
        .description(
            "References a previously completed detection by id and optionally \
             carries per-entity overrides (accept / reject / replace / add). \
             Re-imports the original content, applies overrides to the audit, \
             runs redaction + validation, and writes to the configured exports.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn list_redactions(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Query(query): Query<RedactionQuery>,
) -> Result<Json<RedactionList>> {
    let filter = RedactionFilter {
        status: query.status,
        detection_id: query.detection_id,
    };
    let entries = engine.list_redactions(actor_id, filter).await;
    let page = query.pagination.paginate(entries);
    Ok(Json(page))
}

fn list_redactions_docs(op: TransformOperation) -> TransformOperation {
    op.id("listRedactions")
        .tag("redactions")
        .summary("List redactions")
        .description(
            "Returns redaction passes for the caller, optionally filtered by \
             status and / or detection id.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_redaction(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RedactionPath { id }): Path<RedactionPath>,
) -> Result<Json<RedactionSnapshot>> {
    let snapshot = engine.get_redaction(actor_id, id).await?;
    Ok(Json(snapshot))
}

fn get_redaction_docs(op: TransformOperation) -> TransformOperation {
    op.id("getRedaction")
        .tag("redactions")
        .summary("Get a redaction snapshot")
        .description(
            "Returns the redaction pass snapshot. Once terminal with at least \
             one audit, the snapshot's `result` field carries the final audit \
             with `Execution` populated and `RedactionDecision` provenance on \
             every override-touched entry.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn cancel_redaction(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RedactionPath { id }): Path<RedactionPath>,
) -> Result<StatusCode> {
    engine.cancel_redaction(actor_id, id).await?;
    Ok(StatusCode::ACCEPTED)
}

fn cancel_redaction_docs(op: TransformOperation) -> TransformOperation {
    op.id("cancelRedaction")
        .tag("redactions")
        .summary("Cancel an in-progress redaction")
        .description("Triggers cooperative cancellation at the next yield point.")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn delete_redaction(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RedactionPath { id }): Path<RedactionPath>,
) -> Result<StatusCode> {
    engine.delete_redaction(actor_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn delete_redaction_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteRedaction")
        .tag("redactions")
        .summary("Delete a finished redaction")
        .description("Removes the in-memory record and cascades to the persisted result.")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn delete_all_redactions(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
) -> Result<StatusCode> {
    let deleted = engine.delete_all_redactions(actor_id).await;
    tracing::info!(target: TARGET, deleted, "all finished redactions deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_all_redactions_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllRedactions")
        .tag("redactions")
        .summary("Delete all finished redactions")
        .description("Active passes are preserved.")
}

pub fn routes_v1() -> ApiRouter<ServiceState> {
    let submit = ApiRouter::new()
        .api_route(
            "/redactions",
            post_with(create_redaction, create_redaction_docs),
        )
        .with_timeout(DEFAULT_PIPELINE_TIMEOUT);

    let read = ApiRouter::new()
        .api_route(
            "/redactions",
            get_with(list_redactions, list_redactions_docs)
                .delete_with(delete_all_redactions, delete_all_redactions_docs),
        )
        .api_route(
            "/redactions/{id}",
            get_with(get_redaction, get_redaction_docs)
                .delete_with(delete_redaction, delete_redaction_docs),
        )
        .api_route(
            "/redactions/{id}/cancel",
            post_with(cancel_redaction, cancel_redaction_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    submit.merge(read)
}
