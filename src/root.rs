//! The root!
//!
//! The most important part of Shurly, the actual redirect logic

use std::borrow::Cow;
use std::collections::HashSet;

use axum::Extension;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::Html;
use axum::response::Redirect;
use axum_extra::TypedHeader;
use axum_extra::headers::UserAgent;
use percent_encoding::percent_decode_str;
use unicode_normalization::UnicodeNormalization;

use crate::api::Error;
use crate::api::parse_url;
use crate::client_ip::ClientIp;
use crate::database::Database;

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
/// A lookup in database will be done looking for the right slug, based on the path
pub async fn root(
    client_ip: Option<ClientIp>,
    user_agent: Option<TypedHeader<UserAgent>>,
    Extension(database): Extension<Database>,
    incoming_uri: Uri,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    let slug = incoming_uri.path().trim_matches('/');
    let slug = url_decode_slug(slug)?;

    tracing::debug!("Looking for slug: /{slug}");

    let slug_found_summary = database
        .fetch_cached_destination_by_slug(&slug)
        .await
        .map_err(internal_error)?;

    if let Some(slug_found_summary) = &*slug_found_summary {
        let destination = slug_found_summary.destination();

        database
            .save_hit(
                destination,
                slug_found_summary.alias(),
                client_ip.map(|i| i.ip_address.0).as_ref(),
                user_agent.map(|i| i.0.to_string()).as_ref(),
            )
            .await
            .map_err(internal_error)?;

        if slug_found_summary.is_deleted() {
            tracing::debug!(r#"Slug "{slug}" no longer exists"#);

            Err((
                StatusCode::GONE,
                render_error_template("Page not longer exists"),
            ))
        } else {
            tracing::debug!(r#"Slug "{slug}" redirecting to: {}"#, destination.url);

            let mut location_url = destination.url.clone();

            if destination.forward_query_parameters
                && let Some(path_and_query) = incoming_uri.path_and_query()
            {
                let location = {
                    let mut location_parsed = parse_url(&location_url).map_err(map_api_error)?;

                    let location_query_param_names = location_parsed
                        .query_pairs()
                        .map(|(name, _)| Cow::Owned(name.to_string()))
                        .collect::<HashSet<Cow<str>>>();

                    let mut location_query_pairs = location_parsed.query_pairs_mut();

                    let incoming_parsed =
                        parse_url(format!("https://www.example.com{path_and_query}"))
                            .map_err(map_api_error)?;

                    let incoming_query_params = incoming_parsed.query_pairs();

                    for (key, value) in incoming_query_params {
                        // skip query params that are already in the location, params from the
                        // location are leading. overwriting this could result is problematic
                        // redirects. adding query params might already be an issue in some cases,
                        // the redirect location should be able to handle the extra params, this is
                        // why the option to append them is behind an option per destination.
                        if !location_query_param_names.contains(&key) {
                            location_query_pairs.append_pair(&key, &value);
                        }
                    }

                    drop(location_query_pairs);

                    location_parsed
                };

                location_url = location.into();
            }

            if destination.is_permanent {
                Ok(Redirect::permanent(&location_url))
            } else {
                Ok(Redirect::temporary(&location_url))
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

/// Utility function for mapping any API error into a HTML response.
fn map_api_error(err: Error) -> (StatusCode, Html<String>) {
    (err.status_code, render_error_template(&err.message))
}

/// URL decode slug
///
/// Will:
/// - Convert percent encoded characters to their UTF-8 representation
/// - Normalize the slug to NFC form (same as database storage)
///
/// Uses percentage encoding for the decoding, might error in case of invalid UTF-8
fn url_decode_slug(slug: &str) -> Result<String, (StatusCode, Html<String>)> {
    let decoded = percent_decode_str(slug);

    decoded
        .decode_utf8()
        .map(|decoded| decoded.nfc().to_string())
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
    fn test_url_decode_slug_unicode() {
        // 'ä' with a single code point U+00E4
        let slug_one = String::from_utf8(vec![195, 164]).unwrap();
        assert_eq!(url_decode_slug(&slug_one).unwrap(), slug_one);

        // 'ä' with two code points: U+0061 U+03080
        let slug_two = String::from_utf8(vec![97, 204, 136]).unwrap();

        // the two code points are normalized to U+00E4
        assert_eq!(url_decode_slug(&slug_two).unwrap(), slug_one);
    }

    #[test]
    fn test_url_decode_slug_invalid() {
        let error = url_decode_slug("%c0").unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, error.0);
    }
}
