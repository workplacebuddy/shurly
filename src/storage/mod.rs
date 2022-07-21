use std::net::IpAddr;

use axum::async_trait;
use thiserror::Error;
use url::Url;
use uuid::Uuid;

use crate::destinations::Destination;
use crate::notes::Note;
use crate::users::Role;
use crate::users::User;

#[cfg(not(feature = "postgres"))]
use memory::Memory;
#[cfg(feature = "postgres")]
use postgres::Postgres;

#[cfg(not(feature = "postgres"))]
mod memory;
#[cfg(feature = "postgres")]
mod postgres;

/// Setup the storage
#[cfg(not(feature = "postgres"))]
#[allow(clippy::unused_async)]
pub async fn setup() -> Memory {
    Memory::new()
}

/// Setup the storage
#[cfg(feature = "postgres")]
pub async fn setup() -> Postgres {
    Postgres::new().await
}

/// Storage errors
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum Error {
    #[error("Connection error: {0}")]
    Connection(String),
}

/// Result type for all storage interactions
pub type Result<T> = core::result::Result<T, Error>;

/// Values to create a User
pub struct CreateUserValues<'a> {
    pub session_id: &'a Uuid,
    pub role: Role,
    pub username: &'a str,
    pub hashed_password: &'a str,
}

/// Values to change a password of a user
pub struct ChangePasswordValues<'a> {
    pub session_id: &'a Uuid,
    pub hashed_password: &'a str,
}

/// Values to create a Destination
pub struct CreateDestinationValues<'a> {
    pub user: &'a User,
    pub slug: &'a str,
    pub url: &'a Url,
    pub is_permanent: &'a bool,
}

/// Values to update an Destination
pub struct UpdateDestinationValues<'a> {
    pub url: Option<Url>,
    pub is_permanent: Option<&'a bool>,
}

/// Values to create an Note
pub struct CreateNoteValues<'a> {
    pub user: &'a User,
    pub content: &'a str,
}

/// Values to update an Note
pub struct UpdateNoteValues<'a> {
    pub content: Option<&'a String>,
}

/// Possible audit trail entry types
pub enum AuditEntry<'a> {
    CreateUser(&'a User),
    ChangePassword(&'a User),
    DeleteUser(&'a User),

    CreateDestination(&'a Destination),
    UpdateDestination(&'a Destination),
    DeleteDestination(&'a Destination),

    CreateNote(&'a Destination, &'a Note),
    UpdateNote(&'a Destination, &'a Note),
    DeleteNote(&'a Destination, &'a Note),
}

#[async_trait]
pub trait Storage: Clone + Send + Sync + 'static {
    /// Find any single user
    ///
    /// Respects the soft-delete
    async fn find_any_single_user(&self) -> Result<Option<User>>;

    /// Finds all users
    ///
    /// Respects the soft-delete
    async fn find_all_users(&self) -> Result<Vec<User>>;

    /// Finds a single user by its username
    ///
    /// Respects the soft-delete
    async fn find_single_user_by_username(&self, username: &str) -> Result<Option<User>>;

    /// Finds a single user by its ID
    ///
    /// Respects the soft-delete
    async fn find_single_user_by_id(&self, id: &Uuid) -> Result<Option<User>>;

    /// Create a single user
    async fn create_user(&self, values: &CreateUserValues) -> Result<User>;

    /// Change the password of a user
    async fn change_password(&self, user: &User, values: &ChangePasswordValues) -> Result<User>;

    /// Soft-delete a user
    async fn delete_user(&self, user: &User) -> Result<()>;

    /// Find all destinations
    ///
    /// Respects the soft-delete
    async fn find_all_destinations(&self) -> Result<Vec<Destination>>;

    /// Find a single destination by slug
    ///
    /// DOES NOT respect the soft-delete, handle with care
    async fn find_single_destination_by_slug(&self, slug: &str) -> Result<Option<Destination>>;

    /// Find a single destination by ID
    ///
    /// Respects the soft-delete
    async fn find_single_destination_by_id(&self, id: &Uuid) -> Result<Option<Destination>>;

    /// Create a destination
    async fn create_destination(&self, values: &CreateDestinationValues) -> Result<Destination>;

    /// Update a single destination
    async fn update_destination(
        &self,
        destination: &Destination,
        values: &UpdateDestinationValues,
    ) -> Result<Destination>;

    /// Soft-delete a destination
    async fn delete_destination(&self, destination: &Destination) -> Result<()>;

    /// Find all notes of a destination
    ///
    /// Respects the soft-delete
    async fn find_all_notes_by_destination(&self, destination: &Destination) -> Result<Vec<Note>>;

    /// Find single note of a destination
    ///
    /// Respects the soft-delete
    async fn find_single_note_by_id(
        &self,
        destination_id: &Uuid,
        note_id: &Uuid,
    ) -> Result<Option<Note>>;

    /// Create a note
    async fn create_note(
        &self,
        destination: &Destination,
        values: &CreateNoteValues,
    ) -> Result<Note>;

    /// Update a note
    async fn update_note(&self, note: &Note, values: &UpdateNoteValues) -> Result<Note>;

    /// Soft-delete a note
    async fn delete_note(&self, note: &Note) -> Result<()>;

    /// Save a hit on a destination
    async fn save_hit(
        &self,
        destination: &Destination,
        ip_address: Option<&IpAddr>,
        user_agent: Option<&String>,
    ) -> Result<()>;

    /// Register a creative/destructive action on the audit trail
    async fn register_audit_trail(
        &self,
        user: &User,
        entry: &AuditEntry,
        ip_address: Option<&IpAddr>,
    ) -> Result<()>;
}
