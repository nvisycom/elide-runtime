use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::Json;
use nvisy_core::{Error, ErrorKind};

use super::response::{AnalyticsSummary, ServerError};
use crate::service::ServiceState;

/// `GET /api/v1/analytics` — retrieve aggregate pipeline analytics.
#[tracing::instrument(skip_all)]
pub async fn summary(
    State(_state): State<ServiceState>,
) -> Result<impl IntoApiResponse, ServerError> {
    Err::<Json<AnalyticsSummary>, _>(ServerError::from(Error::new(
        ErrorKind::Runtime,
        "analytics endpoint not yet implemented",
    )))
}
