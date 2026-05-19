//! Custom `Json` extractor that converts rejections into [`Error`].
//!
//! Wraps [`axum::Json`] so that malformed JSON bodies produce our
//! standard [`ErrorResponse`]
//! instead of axum's default plain-text rejection.
//!
//! [`ErrorResponse`]: crate::handler::response::ErrorResponse

use aide::OperationInput;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::response::{IntoResponse, Response};

use crate::handler::error::{Error, ErrorKind};

/// A JSON extractor that rejects with [`Error`] instead of axum's
/// default [`JsonRejection`].
///
/// On the **request** side it deserialises `T` from the body, mapping
/// any rejection to [`ErrorKind::BadRequest`].
///
/// On the **response** side it delegates to [`axum::Json`], so
/// handlers can use a single `Json` type for both input and output.
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    S: Send + Sync,
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(req, state)
            .await
            .map(|axum::Json(v)| Self(v))
            .map_err(|rejection| ErrorKind::BadRequest.with_message(rejection.body_text()))
    }
}

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl<T: schemars::JsonSchema> OperationInput for Json<T> {
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        axum::Json::<T>::operation_input(ctx, operation);
    }
}

impl<T: schemars::JsonSchema + serde::Serialize> aide::OperationOutput for Json<T> {
    type Inner = T;

    fn operation_response(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Option<aide::openapi::Response> {
        axum::Json::<T>::operation_response(ctx, operation)
    }
}
