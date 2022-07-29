//! The root!
//!
//! The most important part of Shurly, the actual redirect logic

use axum::headers::UserAgent;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::Html;
use axum::response::Redirect;
use axum::Extension;
use axum::TypedHeader;
use axum_client_ip::ClientIp;
use percent_encoding::percent_decode_str;

use crate::storage::Storage;

/// Template for 404 page
const NOT_FOUND: &str = include_str!("pages/404.html");

/// Template for error page
///
/// Has a placeholder to inject a current error message
const ERROR: &str = include_str!("pages/500.html");

/// The root!
///
/// All wildcard requests end up in this function.
///
/// A lookup in storage will be done looking for the right slug, based on the path
pub async fn root<S: Storage>(
    ip_address: Option<ClientIp>,
    user_agent: Option<TypedHeader<UserAgent>>,
    Extension(storage): Extension<S>,
    uri: Uri,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let slug = uri.path().trim_matches('/');
    let slug = url_decode_slug(slug)?;

    tracing::debug!("Looking for slug: /{slug}");

    let destination = storage
        .find_single_destination_by_slug(&slug)
        .await
        .map_err(internal_error)?;

    if let Some(destination) = destination {
        storage
            .save_hit(
                &destination,
                ip_address.map(|i| i.0).as_ref(),
                user_agent.map(|i| i.0.to_string()).as_ref(),
            )
            .await
            .map_err(internal_error)?;

        if destination.is_deleted() {
            tracing::debug!(r#"Slug "{slug}" no longer exists"#);

            Err((
                StatusCode::GONE,
                render_error_template("Page not longer exists"),
            ))
        } else {
            tracing::debug!(r#"Slug "{slug}" redirecting to: {}"#, destination.url);

            if destination.is_permanent {
                Ok(Redirect::permanent(&destination.url))
            } else {
                Ok(Redirect::temporary(&destination.url))
            }
        }
    } else {
        tracing::debug!(r#"Slug "{slug}" not found"#);

        Err((StatusCode::NOT_FOUND, render_not_found_template()))
    }
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, Html<String>)
where
    E: std::error::Error,
{
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        render_error_template(&err.to_string()),
    )
}

/// URL decode slug
///
/// Uses percentage encoding for the decoding, might error in case of invalid UTF-8
fn url_decode_slug(slug: &str) -> Result<String, (StatusCode, Html<String>)> {
    let decoded = percent_decode_str(slug);

    decoded
        .decode_utf8()
        .map(|decoded| decoded.to_string())
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                render_error_template("URL contains invalid UTF-8 characters"),
            )
        })
}

/// Create a HTML version of not found template
fn render_not_found_template() -> Html<String> {
    Html(NOT_FOUND.to_string())
}

/// Very, very simple template renderer
///
/// Only replaces the `{error}` in the template with the given string
///
/// Make sure to not use user provided error messages, those are _NOT_ safe
fn render_error_template(error: &str) -> Html<String> {
    Html(ERROR.replace("{error}", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_decode_slug_space() {
        let slug = url_decode_slug("%20").unwrap();
        assert_eq!(" ".to_string(), slug);
    }

    #[test]
    fn test_url_decode_slug_invalid() {
        let error = url_decode_slug("%c0").unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, error.0);
    }
}
