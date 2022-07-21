use std::marker::PhantomData;

use axum::headers::UserAgent;
use axum::http::header::LOCATION;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::Extension;
use axum::TypedHeader;
use axum_client_ip::ClientIp;

use crate::storage::Storage;

pub async fn root<S: Storage>(
    ip_address: Option<ClientIp>,
    user_agent: Option<TypedHeader<UserAgent>>,
    Extension(storage): Extension<S>,
    uri: Uri,
) -> Result<(StatusCode, HeaderMap), (StatusCode, String)> {
    let slug = uri.path().trim_matches('/');

    tracing::debug!("Looking for slug: /{slug}");

    let destination = storage
        .find_single_destination_by_slug(slug)
        .await
        .map_err(internal_error)?;

    let mut headers = HeaderMap::new();

    let status_code = if let Some(destination) = destination {
        storage
            .save_hit(
                &destination,
                ip_address.map(|i| i.0).as_ref(),
                user_agent.map(|i| i.0.to_string()).as_ref(),
            )
            .await
            .map_err(internal_error)?;

        if destination.deleted_at.is_some() {
            tracing::debug!(r#"Slug "{slug}" no longer exists"#);

            StatusCode::GONE
        } else {
            tracing::debug!(r#"Slug "{slug}" redirecting to: {}"#, destination.url);

            headers.insert(
                LOCATION,
                HeaderValue::from_str(&destination.url).expect("Valid URL"),
            );

            if destination.is_permanent {
                StatusCode::PERMANENT_REDIRECT
            } else {
                StatusCode::TEMPORARY_REDIRECT
            }
        }
    } else {
        tracing::debug!(r#"Slug "{slug}" not found"#);

        StatusCode::NOT_FOUND
    };

    Ok((status_code, headers))
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

struct Bla<S: Storage>(PhantomData<S>);

#[axum::async_trait]
impl<B, S: Storage> axum::extract::FromRequest<B> for Bla<S>
where
    B: Send,
{
    type Rejection = ();

    async fn from_request(
        req: &mut axum::extract::RequestParts<B>,
    ) -> Result<Self, Self::Rejection> {
        let Extension(_storage) = req.extract::<Extension<S>>().await.map_err(|_| ())?;

        Ok(Bla(PhantomData))
    }
}
