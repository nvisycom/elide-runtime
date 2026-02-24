use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::Json;
use nvisy_core::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::Serialize;

use super::error::ServerError;
use crate::service::ServiceState;

/// Response body for `GET /api/v1/analytics`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AnalyticsSummary {
    /// Total number of pipeline runs.
    pub total_runs: u64,
    /// Total number of entities detected across all runs.
    pub total_entities_detected: u64,
    /// Total number of redactions applied across all runs.
    pub total_redactions_applied: u64,
}

/// `GET /api/v1/analytics` — retrieve aggregate pipeline analytics.
pub async fn summary(
    State(_state): State<ServiceState>,
) -> Result<impl IntoApiResponse, ServerError> {
    Err::<Json<AnalyticsSummary>, _>(ServerError::from(Error::new(
        ErrorKind::Runtime,
        "analytics endpoint not yet implemented",
    )))
}
