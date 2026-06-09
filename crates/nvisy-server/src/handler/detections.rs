//! Detection-pass HTTP handlers.
//!
//! | Method   | Path                       | Description                          |
//! |----------|----------------------------|--------------------------------------|
//! | `POST`   | `/detections`              | Submit a detection pass              |
//! | `GET`    | `/detections`              | List detections (filterable)         |
//! | `GET`    | `/detections/{id}`         | Get a detection snapshot             |
//! | `POST`   | `/detections/{id}/cancel`  | Cancel an in-progress detection      |
//! | `DELETE` | `/detections/{id}`         | Delete a finished detection          |
//! | `DELETE` | `/detections`              | Delete all finished detections       |

use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_document::pipeline::{DetectionFilter, DetectionSnapshot, Engine};

use super::request::{DetectionPath, DetectionQuery, NewDetection};
use super::response::{DetectionId, DetectionList, Page};
use crate::extract::{ActorId, Json, Path};
use crate::handler::error::Result;
use crate::middleware::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::detections";

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn create_detection(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewDetection>,
) -> Result<(StatusCode, Json<DetectionId>)> {
    let input = req.into_engine_input(actor_id);
    let id = engine.detect(input).await?;
    tracing::info!(target: TARGET, %id, "detection pass submitted");
    Ok((StatusCode::ACCEPTED, Json(DetectionId { id })))
}

fn create_detection_docs(op: TransformOperation) -> TransformOperation {
    op.id("createDetection")
        .tag("detections")
        .summary("Submit a detection pass")
        .description(
            "Runs imports → extraction → recognition → deduplication → policy \
             evaluation. Stops before applying any redaction; the result audit \
             holds the policy chain's pending decisions for review before a \
             matching redaction pass.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn list_detections(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Query(query): Query<DetectionQuery>,
) -> Result<Json<DetectionList>> {
    let filter = DetectionFilter {
        status: query.status,
    };
    let entries = engine.list_detections(actor_id, filter).await;
    let page = Page::paginate(entries, &query.pagination);
    Ok(Json(page))
}

fn list_detections_docs(op: TransformOperation) -> TransformOperation {
    op.id("listDetections")
        .tag("detections")
        .summary("List detections")
        .description("Returns detection passes for the caller, optionally filtered by status.")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_detection(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(DetectionPath { id }): Path<DetectionPath>,
) -> Result<Json<DetectionSnapshot>> {
    let snapshot = engine.get_detection(actor_id, id).await?;
    Ok(Json(snapshot))
}

fn get_detection_docs(op: TransformOperation) -> TransformOperation {
    op.id("getDetection")
        .tag("detections")
        .summary("Get a detection snapshot")
        .description(
            "Returns the detection pass snapshot. Once the pass reaches a \
             terminal state with at least one audit, the snapshot's `result` \
             field carries the immutable `DetectionResult` a redaction pass \
             references.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn cancel_detection(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(DetectionPath { id }): Path<DetectionPath>,
) -> Result<StatusCode> {
    engine.cancel_detection(actor_id, id).await?;
    Ok(StatusCode::ACCEPTED)
}

fn cancel_detection_docs(op: TransformOperation) -> TransformOperation {
    op.id("cancelDetection")
        .tag("detections")
        .summary("Cancel an in-progress detection")
        .description("Triggers cooperative cancellation at the next yield point.")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn delete_detection(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(DetectionPath { id }): Path<DetectionPath>,
) -> Result<StatusCode> {
    engine.delete_detection(actor_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn delete_detection_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteDetection")
        .tag("detections")
        .summary("Delete a finished detection")
        .description("Removes the in-memory record and cascades to the persisted result.")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn delete_all_detections(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
) -> Result<StatusCode> {
    let deleted = engine.delete_all_detections(actor_id).await;
    tracing::info!(target: TARGET, deleted, "all finished detections deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_all_detections_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllDetections")
        .tag("detections")
        .summary("Delete all finished detections")
        .description("Active passes are preserved.")
}

pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read = ApiRouter::new()
        .api_route(
            "/detections",
            get_with(list_detections, list_detections_docs),
        )
        .api_route(
            "/detections/{id}",
            get_with(get_detection, get_detection_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    let write = ApiRouter::new()
        .api_route(
            "/detections",
            post_with(create_detection, create_detection_docs)
                .delete_with(delete_all_detections, delete_all_detections_docs),
        )
        .api_route(
            "/detections/{id}",
            delete_with(delete_detection, delete_detection_docs),
        )
        .api_route(
            "/detections/{id}/cancel",
            post_with(cancel_detection, cancel_detection_docs),
        )
        .with_timeout(DEFAULT_WRITE_TIMEOUT);

    read.merge(write)
}
