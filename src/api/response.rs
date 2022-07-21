//! API response helpers

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use serde::Serialize;

use crate::users::Role;

/// Hold data for a successful API interaction
pub struct Success<V>
where
    V: Serialize,
{
    status_code: StatusCode,
    data: Option<V>,
}

impl<V> Success<V>
where
    V: Serialize,
{
    pub fn ok(data: V) -> Self {
        Self {
            status_code: StatusCode::OK,
            data: Some(data),
        }
    }

    pub fn created(data: V) -> Self {
        Self {
            status_code: StatusCode::CREATED,
            data: Some(data),
        }
    }

    pub fn no_content() -> Self {
        Self {
            status_code: StatusCode::NO_CONTENT,
            data: None,
        }
    }
}

#[derive(Serialize)]
struct DataWrapper<D>
where
    D: Serialize,
{
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

/// Hold data for a failed API interaction
pub struct Error {
    status_code: StatusCode,
    message: String,
    description: Option<String>,
}

impl Error {
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

#[derive(Serialize)]
struct ErrorWrapper<D>
where
    D: Serialize,
{
    error: D,
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

impl Role {
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
