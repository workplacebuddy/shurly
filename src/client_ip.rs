//! Optional client IP address extractor.
//!
//! The version of `axum_client_ip` no longer supports optional IP address extraction.

use std::convert::Infallible;

use axum::extract::FromRequestParts as _;
use axum::extract::OptionalFromRequestParts;
use axum::http::request::Parts;

/// Client IP address extractor.
#[derive(Debug, Clone)]
pub struct ClientIp {
    /// Internal IP address
    pub ip_address: axum_client_ip::ClientIp,
}

impl<S> OptionalFromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        let ip_address = axum_client_ip::ClientIp::from_request_parts(parts, state).await;

        Ok(ip_address.ok().map(|ip_address| Self { ip_address }))
    }
}
