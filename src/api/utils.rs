//! Utility functions for the API

use uuid::Uuid;

use crate::api::Error;
use crate::database::Database;
use crate::destinations::Destination;

/// Fetch destination from database
pub async fn fetch_destination(
    database: &Database,
    destination_id: &Uuid,
) -> Result<Destination, Error> {
    database
        .find_single_destination_by_id(destination_id)
        .await
        .map_err(Error::internal_server_error)?
        .map_or_else(|| Err(Error::not_found("Destination not found")), Ok)
}
