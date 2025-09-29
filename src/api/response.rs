//! API response helpers

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use serde::Serialize;

use crate::database::SlugFoundSummary;
use crate::users::Role;

/// Hold data for a successful API response
pub struct Success<V>
where
    V: Serialize,
{
    /// Status code for the response
    status_code: StatusCode,

    /// Optional data of the successful response
    data: Option<V>,
}

impl<V> Success<V>
where
    V: Serialize,
{
    /// Create new Success response with `200 Ok` status code
    pub fn ok(data: V) -> Self {
        Self {
            status_code: StatusCode::OK,
            data: Some(data),
        }
    }

    /// Create new Success response with `201 Created` status code
    pub fn created(data: V) -> Self {
        Self {
            status_code: StatusCode::CREATED,
            data: Some(data),
        }
    }

    /// Create new Success response with `204 No content` status code
    pub fn no_content() -> Self {
        Self {
            status_code: StatusCode::NO_CONTENT,
            data: None,
        }
    }
}

/// Simple wrapper around the data
#[derive(Serialize)]
struct DataWrapper<D>
where
    D: Serialize,
{
    /// The wrapped data
    data: D,
}

impl<V> IntoResponse for Success<V>
where
    V: Serialize,
{
    fn into_response(self) -> Response {
        if let Some(data) = self.data {
            (self.status_code, Json(DataWrapper { data })).into_response()
        } else {
            self.status_code.into_response()
        }
    }
}

/// Hold data for a failed API response
#[derive(Debug)]
pub struct Error {
    /// The failed status code
    status_code: StatusCode,

    /// The error message
    message: String,

    /// An optional error description
    description: Option<String>,
}

impl Error {
    /// Create new Error response with `400 Bad request` status code
    pub fn bad_request<M>(message: M) -> Self
    where
        M: ToString,
    {
        Self {
            status_code: StatusCode::BAD_REQUEST,
            message: message.to_string(),
            description: None,
        }
    }

    /// Create new Error response with `403 Forbidden` status code
    pub fn forbidden<M>(message: M) -> Self
    where
        M: ToString,
    {
        Self {
            status_code: StatusCode::FORBIDDEN,
            message: message.to_string(),
            description: None,
        }
    }

    /// Create new Error response with `404 Not found` status code
    pub fn not_found<M>(message: M) -> Self
    where
        M: ToString,
    {
        Self {
            status_code: StatusCode::NOT_FOUND,
            message: message.to_string(),
            description: None,
        }
    }

    /// Create new Error response with `500 Internal server error` status code
    pub fn internal_server_error<M>(message: M) -> Self
    where
        M: ToString,
    {
        Self {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.to_string(),
            description: None,
        }
    }

    /// Create a version of the error with a description
    pub fn with_description<M>(&self, description: M) -> Self
    where
        M: ToString,
    {
        Self {
            status_code: self.status_code,
            message: self.message.clone(),
            description: Some(description.to_string()),
        }
    }
}

/// Error data wrapper
#[derive(Serialize)]
struct ErrorWrapper<D>
where
    D: Serialize,
{
    /// The error message
    error: D,

    /// Optional error description
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<D>,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (
            self.status_code,
            Json(ErrorWrapper {
                error: self.message,
                description: self.description,
            }),
        )
            .into_response()
    }
}

impl SlugFoundSummary {
    /// Convert the slug found summry into a proper API error
    pub fn into_error(self) -> Error {
        match self {
            SlugFoundSummary::DestinationExists(destination) => Error::bad_request(format!(
                "Slug already in use by destination ID {}",
                destination.id
            )),

            SlugFoundSummary::DestinationDeleted(destination, _) => Error::bad_request(format!(
                "Slug already in use by now deleted destination ID {}",
                destination.id
            )),

            SlugFoundSummary::AliasExists(alias, destination) => Error::bad_request(format!(
                "Slug already in use by alias ID {} for destination ID {}",
                alias.id, destination.id
            )),

            SlugFoundSummary::AliasDeleted(alias, destination) => Error::bad_request(format!(
                "Slug already in use by now deleted alias ID {} for destination ID {}",
                alias.id, destination.id
            )),
        }
    }
}

impl Role {
    /// Check if the current role matches the target role
    ///
    /// Will return a forbidden [`Error`](Error) which can be used like this:
    ///
    /// ```rust
    /// let role = Role::Manager;
    /// role.is_allowed(Role::Admin)?;
    /// ```
    pub fn is_allowed(self, target_role: Role) -> Result<(), Error> {
        match self {
            Role::Admin => Ok(()),
            Role::Manager => match target_role {
                Role::Admin => Err(Error::forbidden("Not allowed to acces")),
                Role::Manager => Ok(()),
            },
        }
    }
}
