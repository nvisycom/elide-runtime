use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::Json;
use nvisy_core::{Error, ErrorKind};
use nvisy_identify::Policies;

use super::request::RedactionRequest;
use super::response::{RedactionResponse, ServerError};
use crate::service::ServiceState;

/// `POST /api/v1/redaction` — run the redaction pipeline on uploaded content.
#[tracing::instrument(skip_all, fields(content_id = %req.content_id, actor = req.actor.as_deref()))]
pub async fn redact(
    State(_state): State<ServiceState>,
    Json(req): Json<RedactionRequest>,
) -> Result<impl IntoApiResponse, ServerError> {
    let _policies: Policies = serde_json::from_value(req.policies)
        .map_err(|e| Error::new(ErrorKind::Validation, format!("invalid policies: {e}")))?;

    let _graph = req.graph;
    let _connections = req.connections;

    Err::<Json<RedactionResponse>, _>(ServerError::from(Error::new(
        ErrorKind::Runtime,
        format!(
            "redaction endpoint not yet implemented (content_id: {}, actor: {})",
            req.content_id,
            req.actor.as_deref().unwrap_or("<none>"),
        ),
    )))
}
