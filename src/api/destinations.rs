//! Destinations API endpoints
//!
//! Everything related to the destinations management

use std::marker::PhantomData;

use axum::Extension;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::aliases::Alias;
use crate::api::aliases::AliasResponse;
use crate::api::notes::NoteResponse;
use crate::api::request::IncludeParameters;
use crate::api::utils::fetch_destination;
use crate::database::fetch_destination_by_slug;
use crate::database::AuditEntry;
use crate::database::CreateDestinationValues;
use crate::database::Database;
use crate::database::UpdateDestinationValues;
use crate::destinations::Destination;
use crate::notes::Note;
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

    /// Should the query parameters of the root endpoint be forwarded to the destination?
    ///
    /// Only query parameters that are _not_ present in the `url` will be added
    pub forward_query_parameters: bool,

    /// Creation date
    pub created_at: NaiveDateTime,

    /// Last updated at
    pub updated_at: NaiveDateTime,

    /// List of aliases
    pub aliases: Option<Vec<AliasResponse>>,

    /// List of notes
    pub notes: Option<Vec<NoteResponse>>,
}

impl DestinationResponse {
    /// Create a response from a [`Destination`](Destination)
    ///
    /// Basically filtering which fields are shown to the user
    fn from_destination(
        destination: Destination,
        aliases: Option<Vec<Alias>>,
        notes: Option<Vec<Note>>,
    ) -> Self {
        Self {
            id: destination.id,
            slug: destination.slug,
            url: destination.url,
            is_permanent: destination.is_permanent,
            forward_query_parameters: destination.forward_query_parameters,
            created_at: destination.created_at,
            updated_at: destination.updated_at,
            aliases: aliases.map(AliasResponse::from_alias_multiple),
            notes: notes.map(NoteResponse::from_note_multiple),
        }
    }
}

/// Marker for single destination response builder
struct Single;

/// Marker for multiple destinations response builder
struct Multiple;

/// Destination response builder
struct DestinationResponseBuilder<T> {
    /// Single destination when building a single response
    single: Option<Destination>,

    /// Multiple destinations when building multiple responses
    multiple: Option<Vec<Destination>>,

    /// Optional aliases to include in the response(s)
    aliases: Option<Vec<Alias>>,

    /// Optional notes to include in the response(s)
    notes: Option<Vec<Note>>,

    /// Magic
    _marker: PhantomData<T>,
}

impl<T> DestinationResponseBuilder<T> {
    /// With aliases to include in the response(s)
    fn with_aliases(mut self, aliases: Vec<Alias>) -> Self {
        self.aliases = Some(aliases);
        self
    }

    /// With notes to include in the response(s)
    fn with_notes(mut self, notes: Vec<Note>) -> Self {
        self.notes = Some(notes);
        self
    }
}

impl DestinationResponseBuilder<Single> {
    /// Create a response builder for a single [`Destination`](Destination)
    fn new(destination: Destination) -> DestinationResponseBuilder<Single> {
        DestinationResponseBuilder {
            single: Some(destination),
            multiple: None,
            aliases: None,
            notes: None,
            _marker: PhantomData,
        }
    }

    /// Current destinations
    ///
    /// Just a single one
    fn destinations(&self) -> &[Destination] {
        self.single
            .as_ref()
            .map(std::slice::from_ref)
            .expect("Single destination must be provided")
    }

    /// Build the single destination response
    fn build(self) -> DestinationResponse {
        let destination = self.single.expect("Single destination must be provided");

        DestinationResponse::from_destination(destination, self.aliases, self.notes)
    }
}

impl DestinationResponseBuilder<Multiple> {
    /// Create a response builder for multiple [`Destination`](Destination)s
    fn new(destinations: Vec<Destination>) -> DestinationResponseBuilder<Multiple> {
        DestinationResponseBuilder {
            single: None,
            multiple: Some(destinations),
            aliases: None,
            notes: None,
            _marker: PhantomData,
        }
    }

    /// Current destinations
    fn destinations(&self) -> &[Destination] {
        self.multiple
            .as_ref()
            .expect("Multiple destinations must be provided")
    }

    /// Build the multiple destinations response
    fn build(mut self) -> Vec<DestinationResponse> {
        let mut destinations = self
            .multiple
            .take()
            .expect("Multiple destinations must be provided");

        destinations
            .drain(..)
            .map(|destination| {
                let filtered_aliases = self.aliases.as_mut().map(|aliases| {
                    let (for_destination, rest): (Vec<Alias>, Vec<Alias>) = aliases
                        .drain(..)
                        .partition(|alias| alias.destination_id == destination.id);

                    *aliases = rest;

                    for_destination
                });

                let filtered_notes = self.notes.as_mut().map(|notes| {
                    let (for_destination, rest): (Vec<Note>, Vec<Note>) = notes
                        .drain(..)
                        .partition(|note| note.destination_id == destination.id);

                    *notes = rest;

                    for_destination
                });

                DestinationResponse::from_destination(destination, filtered_aliases, filtered_notes)
            })
            .collect()
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

    let mut builder = DestinationResponseBuilder::<Multiple>::new(destinations);

    if include_parameters.aliases {
        let aliases = database
            .find_all_aliases_by_destinations(builder.destinations())
            .await
            .map_err(Error::internal_server_error)?;

        builder = builder.with_aliases(aliases);
    }

    if include_parameters.notes {
        let notes = database
            .find_all_notes_by_destinations(builder.destinations())
            .await
            .map_err(Error::internal_server_error)?;

        builder = builder.with_notes(notes);
    }

    Ok(Success::ok(builder.build()))
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

    let mut builder = DestinationResponseBuilder::<Single>::new(destination);

    if include_parameters.aliases {
        let aliases = database
            .find_all_aliases_by_destinations(builder.destinations())
            .await
            .map_err(Error::internal_server_error)?;

        builder = builder.with_aliases(aliases);
    }

    if include_parameters.notes {
        let notes = database
            .find_all_notes_by_destinations(builder.destinations())
            .await
            .map_err(Error::internal_server_error)?;

        builder = builder.with_notes(notes);
    }

    Ok(Success::ok(builder.build()))
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

    /// Should the query parameters of the root endpoint be forwarded to the destination?
    ///
    /// Only query parameters that are _not_ present in the `url` will be added
    forward_query_parameters: Option<bool>,
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
            forward_query_parameters: &form.forward_query_parameters.unwrap_or(false),
        };

        let destination = database
            .create_destination(&values)
            .await
            .map_err(Error::internal_server_error)?;

        audit_trail
            .register(AuditEntry::CreateDestination(&destination))
            .await;

        let builder = DestinationResponseBuilder::<Single>::new(destination);

        Ok(Success::created(builder.build()))
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

    /// Should the query parameters of the root endpoint be forwarded to the destination?
    ///
    /// Only query parameters that are _not_ present in the `url` will be added
    forward_query_parameters: Option<bool>,
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
        forward_query_parameters: form.forward_query_parameters.as_ref(),
    };

    let updated_destination = database
        .update_destination(&destination, &values)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::UpdateDestination(&destination))
        .await;

    let builder = DestinationResponseBuilder::<Single>::new(updated_destination);

    Ok(Success::ok(builder.build()))
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
