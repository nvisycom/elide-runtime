//! Custom `ActorId` extractor that reads the `X-Actor-Id` header.
//!
//! Wraps the raw header value into a typed [`ActorId`] newtype,
//! rejecting requests that omit the header or supply an invalid UUID
//! with our standard [`ErrorResponse`].
//!
//! [`ErrorResponse`]: crate::handler::response::ErrorResponse

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{
    HeaderStyle, Operation, Parameter, ParameterData, ParameterSchemaOrContent, SchemaObject,
};
use aide::operation::add_parameters;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use derive_more::{Deref, Display};
use schemars::JsonSchema;
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

impl OperationInput for ActorId {
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        let schema = Uuid::json_schema(&mut ctx.schema);
        add_parameters(
            ctx,
            operation,
            [Parameter::Header {
                parameter_data: ParameterData {
                    name: ACTOR_ID_HEADER.to_owned(),
                    description: Some(
                        "UUID identifying the calling actor. Required on every actor-scoped endpoint.".to_owned(),
                    ),
                    required: true,
                    deprecated: None,
                    format: ParameterSchemaOrContent::Schema(SchemaObject {
                        json_schema: schema,
                        example: None,
                        external_docs: None,
                    }),
                    example: None,
                    examples: Default::default(),
                    explode: None,
                    extensions: Default::default(),
                },
                style: HeaderStyle::Simple,
            }],
        );
    }
}

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
