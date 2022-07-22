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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationResponse {
    pub id: Uuid,
    pub slug: String,
    pub url: String,
    pub is_permanent: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl DestinationResponse {
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

    fn from_destination_multiple(mut destinations: Vec<Destination>) -> Vec<Self> {
        destinations
            .drain(..)
            .map(Self::from_destination)
            .collect::<Vec<Self>>()
    }
}

pub async fn list<S: Storage>(
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
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

pub async fn single<S: Storage>(
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(destination_id): PathParameters<Uuid>,
) -> Result<Success<DestinationResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    get_destination(&storage, &destination_id)
        .await
        .map(|destination| Success::ok(DestinationResponse::from_destination(destination)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDestinationForm {
    slug: String,
    url: String,
    is_permanent: Option<bool>,
}

pub async fn create<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
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
        if destination.deleted_at.is_some() {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDestinationForm {
    url: Option<String>,
    is_permanent: Option<bool>,
}

pub async fn update<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(destination_id): PathParameters<Uuid>,
    Form(form): Form<UpdateDestinationForm>,
) -> Result<Success<DestinationResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = get_destination(&storage, &destination_id).await?;

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

pub async fn delete<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(destination_id): PathParameters<Uuid>,
) -> Result<Success<&'static str>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = get_destination(&storage, &destination_id).await?;

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

async fn get_destination<S: Storage>(
    storage: &S,
    destination_id: &Uuid,
) -> Result<Destination, Error> {
    storage
        .find_single_destination_by_id(destination_id)
        .await
        .map_err(Error::internal_server_error)?
        .map_or_else(|| Err(Error::not_found("Destination not found")), Ok)
}
