//! Destinations API endpoints
//!
//! Everything related to the destinations management

use axum::Extension;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::destinations::Destination;
use crate::storage::AuditEntry;
use crate::storage::CreateDestinationValues;
use crate::storage::Storage;
use crate::storage::UpdateDestinationValues;
use crate::users::Role;

use super::parse_slug;
use super::parse_url;
use super::AuditTrail;
use super::CurrentUser;
use super::Error;
use super::Form;
use super::PathParameters;
use super::Success;

/// Destination response going to the user
///
/// Basically filtering which fields are shown to the user
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationResponse {
    /// Destination ID
    pub id: Uuid,

    /// Slug used to identify the destination by the root
    pub slug: String,

    /// Url where root will redirect to
    pub url: String,

    /// Type of destination
    pub is_permanent: bool,

    /// Creation date
    pub created_at: NaiveDateTime,

    /// Last updated at
    pub updated_at: NaiveDateTime,
}

impl DestinationResponse {
    /// Create a response from a [`Destination`](Destination)
    ///
    /// Basically filtering which fields are shown to the user
    fn from_destination(destination: Destination) -> Self {
        Self {
            id: destination.id,
            slug: destination.slug,
            url: destination.url,
            is_permanent: destination.is_permanent,
            created_at: destination.created_at,
            updated_at: destination.updated_at,
        }
    }

    /// Create a response from multiple [`Destination`](Destination)s
    ///
    /// Basically filtering which fields are shown to the user
    fn from_destination_multiple(mut destinations: Vec<Destination>) -> Vec<Self> {
        destinations
            .drain(..)
            .map(Self::from_destination)
            .collect::<Vec<Self>>()
    }
}

/// List all destinations
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:7000/api/destinations
/// ```
///
/// Response:
/// ```json
/// { "data": [ { "id": "<uuid>", "slug": "some-easy-name" ... } ] }
/// ```
pub async fn list(
    Extension(storage): Extension<Storage>,
    current_user: CurrentUser,
) -> Result<Success<Vec<DestinationResponse>>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destinations = storage
        .find_all_destinations()
        .await
        .map_err(Error::internal_server_error)?;

    Ok(Success::ok(DestinationResponse::from_destination_multiple(
        destinations,
    )))
}

/// Get a single destination
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:7000/api/destinations/<uuid>
/// ```
///
/// Response:
/// ```json
/// { "data": { "id": "<uuid>", "slug": "some-easy-name" ... } }
/// ```
pub async fn single(
    Extension(storage): Extension<Storage>,
    current_user: CurrentUser,
    PathParameters(destination_id): PathParameters<Uuid>,
) -> Result<Success<DestinationResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    fetch_destination(&storage, &destination_id)
        .await
        .map(|destination| Success::ok(DestinationResponse::from_destination(destination)))
}

/// Create destination form
///
/// Fields to create a destination with
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDestinationForm {
    /// Slug to create a destination with
    ///
    /// The slug is normalized:
    /// - Leading and trailing slashes are removed
    /// - Unicode normalization
    slug: String,

    /// Url to create a destination with
    url: String,

    /// Type to create a destination with
    is_permanent: Option<bool>,
}

/// Create a destination based on the [`CreateDestinationForm`](CreateDestinationForm) form
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     -d '{ "slug": "some-easy-name", "url": "https://www.example.com/" }' \
///     http://localhost:7000/api/destinations
/// ```
///
/// Response
/// ```json
/// { "data": { "id": "<uuid>", "slug": "some-easy-name" ... } }
/// ```
pub async fn create(
    audit_trail: AuditTrail,
    Extension(storage): Extension<Storage>,
    current_user: CurrentUser,
    Form(form): Form<CreateDestinationForm>,
) -> Result<Success<DestinationResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let slug = parse_slug(&form.slug)?;
    let url = parse_url(&form.url)?;

    let destination = storage
        .find_single_destination_by_slug(&slug)
        .await
        .map_err(Error::internal_server_error)?;

    if let Some(destination) = destination {
        if destination.is_deleted() {
            Err(Error::bad_request("Slug already exists and is deleted"))
        } else {
            Err(Error::bad_request("Slug already exists"))
        }
    } else {
        let values = CreateDestinationValues {
            user: &current_user,
            slug: &slug,
            url: &url,
            is_permanent: &form.is_permanent.unwrap_or(false),
        };

        let destination = storage
            .create_destination(&values)
            .await
            .map_err(Error::internal_server_error)?;

        audit_trail
            .register(AuditEntry::CreateDestination(&destination))
            .await;

        Ok(Success::created(DestinationResponse::from_destination(
            destination,
        )))
    }
}

/// Update destination form
///
/// Fields to update a destination with, all fields are optional and are not touched when not
/// provided
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDestinationForm {
    /// New note to update destination with
    url: Option<String>,

    /// Type to update destination with
    ///
    /// Can only be set to `false` if the destination already has `is_permanent=true`, otherwise
    /// only `true` is valid
    is_permanent: Option<bool>,
}

/// Update a destinations based on the [`UpdateDestinationForm`](UpdateDestinationForm) form
///
/// Only provided values are processed, the other fields of the destination will not be touched
///
/// Request:
/// ```sh
/// curl -v -XPATCH -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     -d '{ "url": "https://www.example.com/", "isPermanent": true }' \
///     http://localhost:7000/api/destinations/<uuid>
/// ```
///
/// Response
/// ```json
/// { "data": { "id": "<uuid>", "slug": "some-easy-name" ... } }
/// ```
pub async fn update(
    audit_trail: AuditTrail,
    Extension(storage): Extension<Storage>,
    current_user: CurrentUser,
    PathParameters(destination_id): PathParameters<Uuid>,
    Form(form): Form<UpdateDestinationForm>,
) -> Result<Success<DestinationResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = fetch_destination(&storage, &destination_id).await?;

    if destination.is_permanent {
        return Err(Error::bad_request("Permanent URLs can not be updated"));
    }

    let url = if let Some(ref url) = form.url {
        Some(parse_url(url)?)
    } else {
        None
    };

    let values = UpdateDestinationValues {
        url,
        is_permanent: form.is_permanent.as_ref(),
    };

    let updated_destination = storage
        .update_destination(&destination, &values)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::UpdateDestination(&destination))
        .await;

    Ok(Success::ok(DestinationResponse::from_destination(
        updated_destination,
    )))
}

/// Delete a destination
///
/// Permanent destinations can not be deleted
///
/// Request:
/// ```sh
/// curl -v -XDELETE \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:7000/api/destinations/<uuid>
/// ```
pub async fn delete(
    audit_trail: AuditTrail,
    Extension(storage): Extension<Storage>,
    current_user: CurrentUser,
    PathParameters(destination_id): PathParameters<Uuid>,
) -> Result<Success<&'static str>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = fetch_destination(&storage, &destination_id).await?;

    if destination.is_permanent {
        return Err(Error::bad_request("Permanent URLs can not be deleted"));
    }

    storage
        .delete_destination(&destination)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::DeleteDestination(&destination))
        .await;

    Ok(Success::<&'static str>::no_content())
}

/// Fetch destination from storage
async fn fetch_destination(storage: &Storage, destination_id: &Uuid) -> Result<Destination, Error> {
    storage
        .find_single_destination_by_id(destination_id)
        .await
        .map_err(Error::internal_server_error)?
        .map_or_else(|| Err(Error::not_found("Destination not found")), Ok)
}
