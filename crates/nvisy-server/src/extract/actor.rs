//! Custom `ActorId` extractor that reads the `X-Actor-Id` header.
//!
//! Wraps the raw header value into a typed [`ActorId`] newtype,
//! rejecting requests that omit the header or supply an invalid UUID
//! with our standard [`ErrorResponse`](crate::handler::response::ErrorResponse).

use aide::OperationInput;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use derive_more::{Deref, Display};
use uuid::Uuid;

use crate::handler::error::{Error, ErrorKind};

/// The header name used to identify the calling actor.
pub const ACTOR_ID_HEADER: &str = "x-actor-id";

/// Actor identity extracted from the `X-Actor-Id` request header.
///
/// Every request that operates on actor-scoped resources must include
/// this header. The extractor parses the value as a UUID and rejects
/// with [`ErrorKind::Unauthorized`] when the header is missing, or
/// [`ErrorKind::BadRequest`] when it cannot be parsed.
#[derive(Debug, Clone, Copy, Deref, Display)]
pub struct ActorId(pub Uuid);

impl OperationInput for ActorId {}

impl<S: Send + Sync> FromRequestParts<S> for ActorId {
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let value = parts
            .headers
            .get(ACTOR_ID_HEADER)
            .ok_or_else(|| ErrorKind::Unauthorized.with_message("missing X-Actor-Id header"))?
            .to_str()
            .map_err(|_| {
                ErrorKind::BadRequest.with_message("X-Actor-Id header contains invalid characters")
            })?;

        let id = value.parse::<Uuid>().map_err(|_| {
            ErrorKind::BadRequest.with_message("X-Actor-Id header is not a valid UUID")
        })?;

        Ok(Self(id))
    }
}
