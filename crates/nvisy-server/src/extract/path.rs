//! Custom `Path` extractor that converts rejections into [`Error`].
//!
//! Wraps [`axum::extract::Path`] so that invalid path parameters
//! (e.g. a malformed UUID) produce our standard
//! [`ErrorResponse`](crate::handler::response::ErrorResponse)
//! instead of axum's default plain-text rejection.

use aide::OperationInput;
use axum::extract::FromRequestParts;
use axum::extract::rejection::PathRejection;
use axum::http::request::Parts;

use crate::handler::error::{Error, ErrorKind};

/// A path extractor that rejects with [`Error`] instead of axum's
/// default [`PathRejection`].
///
/// Delegates to [`axum::extract::Path`], mapping any rejection to
/// [`ErrorKind::MissingPathParam`].
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    S: Send + Sync,
    axum::extract::Path<T>: FromRequestParts<S, Rejection = PathRejection>,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(v)| Self(v))
            .map_err(|rejection| ErrorKind::MissingPathParam.with_message(rejection.body_text()))
    }
}

impl<T: schemars::JsonSchema> OperationInput for Path<T> {
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        axum::extract::Path::<T>::operation_input(ctx, operation);
    }
}
