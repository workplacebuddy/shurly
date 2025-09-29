//! Aliases API endpoints
//!
//! Everything related to the aliases management

use axum::Extension;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::aliases::Alias;
use crate::api::parse_slug;
use crate::api::utils::fetch_destination;
use crate::database::fetch_destination_by_slug;
use crate::database::AuditEntry;
use crate::database::CreateAliasValues;
use crate::database::Database;
use crate::users::Role;

use super::AuditTrail;
use super::CurrentUser;
use super::Error;
use super::Form;
use super::PathParameters;
use super::Success;

/// Alias response going to the user
///
/// Basically filtering which fields are shown to the user
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasResponse {
    /// Alias ID
    pub id: Uuid,

    /// Destination ID
    pub destination_id: Uuid,

    /// Slug of the alias
    pub slug: String,

    /// Creation date
    pub created_at: NaiveDateTime,

    /// Last updated at
    pub updated_at: NaiveDateTime,
}

impl AliasResponse {
    /// Create a response from a [`Alias`](Alias)
    ///
    /// Basically filtering which fields are shown to the user
    fn from_alias(alias: Alias) -> Self {
        Self {
            id: alias.id,
            destination_id: alias.destination_id,
            slug: alias.slug,
            created_at: alias.created_at,
            updated_at: alias.updated_at,
        }
    }

    /// Create a response from multiple [`Alias`](Alias)s
    ///
    /// Basically filtering which fields are shown to the user
    pub fn from_alias_multiple(mut aliases: Vec<Alias>) -> Vec<Self> {
        aliases
            .drain(..)
            .map(Self::from_alias)
            .collect::<Vec<Self>>()
    }
}

/// List all aliases for a destination
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:7000/api/destinations/<uuid>/aliases
/// ```
///
/// Response:
/// ```json
/// { "data": [ { "id": "<uuid>", "slug": "some-alternative" ... } ] }
/// ```
pub async fn list(
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    PathParameters(destination_id): PathParameters<Uuid>,
) -> Result<Success<Vec<AliasResponse>>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = fetch_destination(&database, &destination_id).await?;

    let aliases = database
        .find_all_aliases_by_destination(&destination)
        .await
        .map_err(Error::internal_server_error)?;

    Ok(Success::ok(AliasResponse::from_alias_multiple(aliases)))
}

/// Get single alias of a destination
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:7000/api/destinations/<uuid>/aliases/<uuid>
/// ```
///
/// Response:
/// ```json
/// { "data": { "id": "<uuid>", "slug": "some-alternative" ... } }
/// ```
pub async fn single(
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    PathParameters((destination_id, alias_id)): PathParameters<(Uuid, Uuid)>,
) -> Result<Success<AliasResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = fetch_destination(&database, &destination_id).await?;

    fetch_alias(&database, &destination.id, &alias_id)
        .await
        .map(|alias| Success::ok(AliasResponse::from_alias(alias)))
}

/// Create alias form
///
/// Fields to create an alias
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAliasForm {
    /// Slug for an alias
    slug: String,
}

/// Create an alias based on the [`CreateAliasForm`](CreateAliasForm) form
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     -d '{ "slug": "some-alternative" }' \
///     http://localhost:7000/api/destinations/<uuid>/aliases
/// ```
///
/// Response
/// ```json
/// { "data": { "id": "<uuid>", "slug": "some-alternative" ... } }
/// ```
pub async fn create(
    audit_trail: AuditTrail,
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    PathParameters(destination_id): PathParameters<Uuid>,
    Form(form): Form<CreateAliasForm>,
) -> Result<Success<AliasResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let slug = parse_slug(&form.slug)?;

    if slug.starts_with("api/") {
        return Err(Error::bad_request("Slug can not start with 'api/'"));
    }

    let slug_found_summary = fetch_destination_by_slug(&database, &slug)
        .await
        .map_err(Error::internal_server_error)?;

    if let Some(slug_found_summary) = slug_found_summary {
        Err(slug_found_summary.into_error())
    } else {
        let destination = fetch_destination(&database, &destination_id).await?;

        let values = CreateAliasValues {
            user: &current_user,
            slug: &slug,
        };

        let alias = database
            .create_alias(&destination, &values)
            .await
            .map_err(Error::internal_server_error)?;

        audit_trail
            .register(AuditEntry::CreateAlias(&destination, &alias))
            .await;

        Ok(Success::created(AliasResponse::from_alias(alias)))
    }
}

/// Delete an alias
///
/// Request:
/// ```sh
/// curl -v -XDELETE \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:7000/api/destinations/<uuid>/aliases/<uuid>
/// ```
pub async fn delete(
    audit_trail: AuditTrail,
    Extension(database): Extension<Database>,
    current_user: CurrentUser,
    PathParameters((destination_id, alias_id)): PathParameters<(Uuid, Uuid)>,
) -> Result<Success<&'static str>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = fetch_destination(&database, &destination_id).await?;
    let alias = fetch_alias(&database, &destination.id, &alias_id).await?;

    database
        .delete_alias(&alias)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::DeleteAlias(&destination, &alias))
        .await;

    Ok(Success::<&'static str>::no_content())
}

/// Fetch alias from database
async fn fetch_alias(
    database: &Database,
    destination_id: &Uuid,
    alias_id: &Uuid,
) -> Result<Alias, Error> {
    database
        .find_single_alias_by_id(destination_id, alias_id)
        .await
        .map_err(Error::internal_server_error)?
        .map_or_else(|| Err(Error::not_found("Alias not found")), Ok)
}
