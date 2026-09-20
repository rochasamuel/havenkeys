//! JSON extraction that cannot leak the request back to the client.
//!
//! `axum::Json`'s own rejection quotes the offending input ("invalid type:
//! string \"hunter2\", expected …"), which would put request values into
//! responses and, through the trace layer, into logs. `Json<T>` here answers
//! with a fixed message instead.

use crate::error::ApiError;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};

pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(JsonRejection::BytesRejection(_)) => Err(ApiError::TooLarge),
            Err(_) => Err(ApiError::InvalidRequest("request is not valid")),
        }
    }
}
