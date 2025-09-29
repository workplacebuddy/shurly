//! Destinations API endpoints
//!
//! Everything related to the destinations management

use axum::Extension;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::aliases::Alias;
use crate::api::aliases::AliasResponse;
use crate::api::request::IncludeParameters;
use crate::api::utils::fetch_destination;
use crate::database::fetch_destination_by_slug;
use crate::database::AuditEntry;
use crate::database::CreateDestinationValues;
use crate::database::Database;
use crate::database::UpdateDestinationValues;
use crate::destinations::Destination;
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

    /// List of aliases
    pub aliases: Option<Vec<AliasResponse>>,
}

impl DestinationResponse {
    /// Create a response from a [`Destination`](Destination)
    ///
    /// Basically filtering which fields are shown to the user
    fn from_destination(destination: Destination, aliases: Option<Vec<Alias>>) -> Self {
        Self {
            id: destination.id,
            slug: destination.slug,
            url: destination.url,
            is_permanent: destination.is_permanent,
            created_at: destination.created_at,
            updated_at: destination.updated_at,
            aliases: aliases.map(AliasResponse::from_alias_multiple),
        }
    }

    /// Create a response from multiple [`Destination`](Destination)s
    ///
    /// Basically filtering which fields are shown to the user
    fn from_destination_multiple(
        mut destinations: Vec<Destination>,
        mut aliases: Option<Vec<Alias>>,
    ) -> Vec<Self> {
        destinations
            .drain(..)
            .map(|destination| {
                let filtered_aliases = aliases.as_mut().map(|aliases| {
                    let (for_destination, rest): (Vec<Alias>, Vec<Alias>) = aliases
                        .drain(..)
                        .partition(|alias| alias.destination_id == destination.id);

                    *aliases = rest;

                    for_destination
                });

                Self::from_destination(destination, filtered_aliases)
            })
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
///
/// Optionally the aliases of the destinations can be included:
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:7000/api/destinations?include=aliases
/// ```
///
/// Response:
/// ```json
/// { "data": [ { "id": "<uuid>", "slug": "some-easy-name", ..., "aliases": [ { "id": "<uuid>", ... } ] } ] }
/// ```
pub async fn list(
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    include_parameters: IncludeParameters,
) -> Result<Success<Vec<DestinationResponse>>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destinations = database
        .find_all_destinations()
        .await
        .map_err(Error::internal_server_error)?;

    let aliases = if include_parameters.aliases {
        let aliases = database
            .find_all_aliases_by_destinations(&destinations)
            .await
            .map_err(Error::internal_server_error)?;

        Some(aliases)
    } else {
        None
    };

    Ok(Success::ok(DestinationResponse::from_destination_multiple(
        destinations,
        aliases,
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
///
/// Optionally the aliases of the destinations can be included:
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:7000/api/destinations/<uuid>?include=aliases
/// ```
///
/// Response:
/// ```json
/// { "data": { "id": "<uuid>", "slug": "some-easy-name", ..., "aliases": [ { "id": "<uuid>", ... } ] } }
/// ```
pub async fn single(
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    PathParameters(destination_id): PathParameters<Uuid>,
    include_parameters: IncludeParameters,
) -> Result<Success<DestinationResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = fetch_destination(&database, &destination_id).await?;

    let aliases = if include_parameters.aliases {
        let aliases = database
            .find_all_aliases_by_destination(&destination)
            .await
            .map_err(Error::internal_server_error)?;

        Some(aliases)
    } else {
        None
    };

    Ok(Success::ok(DestinationResponse::from_destination(
        destination,
        aliases,
    )))
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
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    Form(form): Form<CreateDestinationForm>,
) -> Result<Success<DestinationResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let slug = parse_slug(&form.slug)?;
    let url = parse_url(&form.url)?;

    if slug.starts_with("api/") {
        return Err(Error::bad_request("Slug can not start with 'api/'"));
    }

    let slug_found_summary = fetch_destination_by_slug(&database, &slug)
        .await
        .map_err(Error::internal_server_error)?;

    if let Some(slug_found_summary) = slug_found_summary {
        Err(slug_found_summary.into_error())
    } else {
        let values = CreateDestinationValues {
            user: &current_user,
            slug: &slug,
            url: &url,
            is_permanent: &form.is_permanent.unwrap_or(false),
        };

        let destination = database
            .create_destination(&values)
            .await
            .map_err(Error::internal_server_error)?;

        audit_trail
            .register(AuditEntry::CreateDestination(&destination))
            .await;

        Ok(Success::created(DestinationResponse::from_destination(
            destination,
            None,
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
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    PathParameters(destination_id): PathParameters<Uuid>,
    Form(form): Form<UpdateDestinationForm>,
) -> Result<Success<DestinationResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = fetch_destination(&database, &destination_id).await?;

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

    let updated_destination = database
        .update_destination(&destination, &values)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::UpdateDestination(&destination))
        .await;

    Ok(Success::ok(DestinationResponse::from_destination(
        updated_destination,
        None,
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
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    PathParameters(destination_id): PathParameters<Uuid>,
) -> Result<Success<&'static str>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = fetch_destination(&database, &destination_id).await?;

    if destination.is_permanent {
        return Err(Error::bad_request("Permanent URLs can not be deleted"));
    }

    database
        .delete_destination(&destination)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::DeleteDestination(&destination))
        .await;

    Ok(Success::<&'static str>::no_content())
}
