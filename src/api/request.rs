//! API request helpers

use axum::async_trait;
use axum::body::HttpBody;
use axum::extract::rejection::JsonRejection;
use axum::extract::rejection::PathRejection;
use axum::extract::FromRequest;
use axum::extract::Json;
use axum::extract::Path;
use axum::extract::RequestParts;
use axum::BoxError;
use serde::de::DeserializeOwned;
use url::Url;

use super::Error;

/// Parse and normalize a slug
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

    Ok(slug.to_string())
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

fn parse_json<J>(json: Result<Json<J>, JsonRejection>) -> Result<J, Error> {
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

/// Wrapper for the JSON extractor
pub struct Form<F>(pub F);

#[async_trait]
impl<B, F> FromRequest<B> for Form<F>
where
    B: HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
    F: DeserializeOwned + Send,
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let json = Result::<Json<F>, JsonRejection>::from_request(req)
            .await
            .map_err(|_| Error::internal_server_error("Could not extract form"))?;

        parse_json(json).map(Form)
    }
}

fn parse_path<P>(path: Result<Path<P>, PathRejection>) -> Result<P, Error> {
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

pub struct PathParameters<P>(pub P);

#[async_trait]
impl<B, P> FromRequest<B> for PathParameters<P>
where
    B: Send,
    P: DeserializeOwned + Send,
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let path = Result::<Path<P>, PathRejection>::from_request(req)
            .await
            .map_err(|_| Error::internal_server_error("Could not extract path"))?;

        parse_path(path).map(PathParameters)
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
}
