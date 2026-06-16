//! Custom `Path` extractor that converts rejections into [`Error`].
//!
//! Wraps [`Path`] so that invalid path parameters (e.g. a malformed
//! UUID) produce our standard [`ErrorResponse`] instead of axum's
//! default plain-text rejection.
//!
//! [`Path`]: axum::extract::Path
//! [`ErrorResponse`]: crate::handler::response::ErrorResponse

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::Operation;
use axum::extract::rejection::PathRejection;
use axum::extract::{FromRequestParts, Path as AxumPath};
use axum::http::request::Parts;

use crate::handler::error::{Error, ErrorKind};

/// A path extractor that rejects with [`Error`] instead of axum's
/// default [`PathRejection`].
///
/// Delegates to [`Path`], mapping any rejection to
/// [`ErrorKind::MissingPathParam`].
///
/// [`Path`]: axum::extract::Path
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    S: Send + Sync,
    AxumPath<T>: FromRequestParts<S, Rejection = PathRejection>,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        AxumPath::<T>::from_request_parts(parts, state)
            .await
            .map(|AxumPath(v)| Self(v))
            .map_err(|rejection| ErrorKind::MissingPathParam.with_message(rejection.body_text()))
    }
}

impl<T: schemars::JsonSchema> OperationInput for Path<T> {
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        AxumPath::<T>::operation_input(ctx, operation);
    }
}
