//! API request helpers

use axum::extract::rejection::JsonRejection;
use axum::extract::rejection::PathRejection;
use axum::extract::FromRequest;
use axum::extract::FromRequestParts;
use axum::extract::Json;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::Request;
use axum::http::request::Parts;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;
use url::Url;

use super::Error;

/// Parse and normalize a slug
///
/// Will:
/// - Remove leading and trailing slashes
/// - Reject if slug contains `?` or `#`
/// - Will normalize the slug to NFC form
///
/// Will return an [`Error`](Error) when the slug contains invalid characters
///
/// ```rust
/// let slug = "/some-slug";
/// assert_eq!(parse_slug(slug), "some-slug".to_string())
/// ```
pub fn parse_slug(slug: &str) -> Result<String, Error> {
    let slug = slug.trim_matches('/');

    for ch in slug.chars() {
        if ch == '?' {
            return Err(Error::bad_request(r#"Slug can not contain "?""#));
        }

        if ch == '#' {
            return Err(Error::bad_request(r##"Slug can not contain "#""##));
        }
    }

    // unicode normalization, prefer NFC form
    Ok(slug.nfc().collect())
}

/// Parse and validate a URL
///
/// ```rust
/// let url = "https://www.example.com/";
/// assert!(parse_url(url).is_ok())
/// ```
pub fn parse_url<I>(url: I) -> Result<Url, Error>
where
    I: AsRef<str>,
{
    Url::parse(url.as_ref()).map_err(Error::bad_request)
}

/// Handle incoming [`Json`](Json) with proper API error handling
///
/// When the json is invalid, a [`Error`](Error) describing the issue will be returned
fn handle_json<J>(json: Result<Json<J>, JsonRejection>) -> Result<J, Error> {
    match json {
        Ok(Json(json)) => Ok(json),
        Err(err) => match err {
            JsonRejection::JsonDataError(err) => {
                Err(Error::bad_request("Data error").with_description(err))
            }
            JsonRejection::JsonSyntaxError(err) => Err(Error::bad_request("JSON syntax error")
                .with_description(std::error::Error::source(&err).expect("A valid source"))),
            JsonRejection::MissingJsonContentType(_err) => Err(Error::bad_request(
                "Missing `application/json` content type",
            )),
            JsonRejection::BytesRejection(err) => {
                Err(Error::bad_request("Invalid characters in JSON").with_description(err))
            }
            err => Err(Error::bad_request("Unknown JSON error").with_description(err)),
        },
    }
}

/// Wrapper around the [`Json`](Json) extractor
pub struct Form<F>(pub F);

impl<S, F> FromRequest<S> for Form<F>
where
    S: Send + Sync,
    F: DeserializeOwned + Send,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let json = Result::<Json<F>, JsonRejection>::from_request(req, state)
            .await
            .map_err(|_| Error::internal_server_error("Could not extract form"))?;

        handle_json(json).map(Form)
    }
}

/// Handle incoming [`Path`](Path) with proper API error handling
///
/// When the path is invalid, a [`Error`](Error) describing the issue will be returned
fn handle_path<P>(path: Result<Path<P>, PathRejection>) -> Result<P, Error> {
    match path {
        Ok(Path(path)) => Ok(path),
        Err(err) => match err {
            PathRejection::FailedToDeserializePathParams(err) => {
                Err(Error::bad_request("Invalid path parameter").with_description(err))
            }
            PathRejection::MissingPathParams(err) => {
                Err(Error::bad_request("Missing path parameter").with_description(err))
            }
            err => Err(Error::bad_request("Unknown path error").with_description(err)),
        },
    }
}

/// Wrapper around the [`Path`](Path) extractor
pub struct PathParameters<P>(pub P);

impl<S, P> FromRequestParts<S> for PathParameters<P>
where
    S: Send + Sync,
    P: DeserializeOwned + Send,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let path = Result::<Path<P>, PathRejection>::from_request_parts(parts, state)
            .await
            .map_err(|_| Error::internal_server_error("Could not extract path"))?;

        handle_path(path).map(PathParameters)
    }
}

/// What should be included?
#[derive(Debug, Default)]
pub struct IncludeParameters {
    /// Should the aliases be included?
    pub aliases: bool,

    /// Should the notes be included?
    pub notes: bool,
}

/// The include query parameter
#[derive(Deserialize)]
struct IncludeQueryParameter {
    /// The include query parameter
    include: Option<String>,
}

impl<S> FromRequestParts<S> for IncludeParameters
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let include_query_parameter =
            Query::<IncludeQueryParameter>::from_request_parts(parts, state)
                .await
                .map_err(|_| {
                    Error::internal_server_error("Could not extract include query parameter")
                })?;

        let mut include_parameters = IncludeParameters {
            aliases: false,
            notes: false,
        };

        if let Some(include) = &include_query_parameter.include {
            for part in include.split(',') {
                match part.trim() {
                    "aliases" => include_parameters.aliases = true,
                    "notes" => include_parameters.notes = true,
                    unknown => {
                        return Err(Error::bad_request("Unknown include parameter")
                            .with_description(format!("Unknown include parameter: {unknown}")))
                    }
                }
            }
        }

        Ok(include_parameters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slug() {
        let slug = "/some-slug";
        assert_eq!(parse_slug(slug).unwrap(), "some-slug".to_string());

        let slug = "some-slug/";
        assert_eq!(parse_slug(slug).unwrap(), "some-slug".to_string());

        let slug = "some-slug";
        assert_eq!(parse_slug(slug).unwrap(), slug.to_string());
    }

    #[test]
    fn test_parse_url() {
        let url = "https://www.example.com/";
        assert!(parse_url(url).is_ok());
    }

    #[test]
    fn test_unicode_normalization() {
        // 'ä' with a single code point U+00E4
        let slug_one = String::from_utf8(vec![195, 164]).unwrap();
        assert_eq!(parse_slug(&slug_one).unwrap(), slug_one);

        // 'ä' with two code points: U+0061 U+03080
        let slug_two = String::from_utf8(vec![97, 204, 136]).unwrap();

        // the two code points are normalized to U+00E4
        assert_eq!(parse_slug(&slug_two).unwrap(), slug_one);
    }
}
