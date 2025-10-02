//! All things related to the storage of destinations and notes

use core::fmt;
use std::net::IpAddr;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::types::ipnetwork::IpNetwork;
use sqlx::PgPool;
use uuid::Uuid;

pub use form_types::*;
pub use Config as DatabaseConfig;

use crate::aliases::Alias;
use crate::destinations::Destination;
use crate::notes::Note;
use crate::users::User;
use types::AuditEntryType;
use types::SqlxUser;
use types::UserRoleType;
use types::MIGRATOR;

mod form_types;
mod types;

/// Storage errors
#[derive(Debug)]
pub enum Error {
    /// A connection error with the storage
    Connection(String),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Connection(error) => write!(f, "Connection error: {error}"),
        }
    }
}

/// Result type for all storage interactions
pub type Result<T> = core::result::Result<T, Error>;

/// Database configuration
pub enum Config {
    /// Detect configuration from environment
    DetectConfig,

    /// Use existing connection
    ExistingConnection(PgPool),
}

/// Postgres storage
#[derive(Clone)]
pub struct Database {
    /// Pool of connections
    connection_pool: PgPool,
}

impl Database {
    /// Create a new Postgres storage
    pub async fn from_config(config: Config) -> Self {
        match config {
            Config::DetectConfig => Self::new().await,
            Config::ExistingConnection(pool) => Self::new_with_pool(pool).await,
        }
    }

    /// Create Postgres storage
    ///
    /// Use the `DATABASE_URL` environment variable
    ///
    /// Migrations will be run
    async fn new() -> Self {
        let database_connection_string = std::env::var("DATABASE_URL").expect("Valid DATABASE_URL");

        let connection_pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(&database_connection_string)
            .await
            .expect("Valid connection");

        Self::new_with_pool(connection_pool).await
    }

    /// Create Postgres storage with existing pool
    ///
    /// Migrations will be run
    async fn new_with_pool(connection_pool: PgPool) -> Self {
        let migration_result = MIGRATOR.run(&connection_pool).await;

        if let Err(err) = migration_result {
            panic!("Migrations could not run: {err}");
        }

        Self { connection_pool }
    }
}

impl Database {
    /// Find any single user
    ///
    /// Respects the soft-delete
    pub async fn find_any_single_user(&self) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            SqlxUser,
            r#"
            SELECT
                id,
                session_id,
                username,
                hashed_password,
                role AS "role: UserRoleType",
                created_at,
                updated_at,
                deleted_at
            FROM users
            WHERE deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map(User::from_sqlx_user_optional)
        .map_err(connection_error)?;

        Ok(user)
    }

    /// Finds all users
    ///
    /// Respects the soft-delete
    pub async fn find_all_users(&self) -> Result<Vec<User>> {
        let users = sqlx::query_as!(
            SqlxUser,
            r#"
            SELECT
                id,
                session_id,
                username,
                hashed_password,
                role AS "role: UserRoleType",
                created_at,
                updated_at,
                deleted_at
            FROM users
            WHERE deleted_at IS NULL
            "#,
        )
        .fetch_all(&self.connection_pool)
        .await
        .map(User::from_sqlx_user_multiple)
        .map_err(connection_error)?;

        Ok(users)
    }

    /// Finds a single user by its username
    ///
    /// Respects the soft-delete
    pub async fn find_single_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            SqlxUser,
            r#"
            SELECT
                id,
                session_id,
                username,
                hashed_password,
                role AS "role: UserRoleType",
                created_at,
                updated_at,
                deleted_at
            FROM users
            WHERE deleted_at IS NULL
                AND username = $1
            LIMIT 1
            "#,
            username,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map(User::from_sqlx_user_optional)
        .map_err(connection_error)?;

        Ok(user)
    }

    /// Finds a single user by its ID
    ///
    /// Respects the soft-delete
    pub async fn find_single_user_by_id(&self, id: &Uuid) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            SqlxUser,
            r#"
            SELECT
                id,
                session_id,
                username,
                hashed_password,
                role AS "role: UserRoleType",
                created_at,
                updated_at,
                deleted_at
            FROM users
            WHERE deleted_at IS NULL
                AND id = $1
            LIMIT 1
            "#,
            id,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map(User::from_sqlx_user_optional)
        .map_err(connection_error)?;

        Ok(user)
    }

    /// Create a single user
    pub async fn create_user(&self, values: &CreateUserValues<'_>) -> Result<User> {
        let user = sqlx::query_as!(
            SqlxUser,
            r#"
            INSERT INTO users (id, session_id, username, hashed_password, role)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id,
                session_id,
                username,
                hashed_password,
                role AS "role: UserRoleType",
                created_at,
                updated_at,
                deleted_at
            "#,
            Uuid::new_v4(),
            values.session_id,
            values.username,
            values.hashed_password,
            UserRoleType::from_role(values.role) as _,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map(User::from_sqlx_user)
        .map_err(connection_error)?;

        Ok(user)
    }

    /// Change the password of a user
    pub async fn change_password(
        &self,
        user: &User,
        values: &ChangePasswordValues<'_>,
    ) -> Result<User> {
        let user = sqlx::query_as!(
            SqlxUser,
            r#"
            UPDATE users
            SET session_id = $1, hashed_password = $2, updated_at = CURRENT_TIMESTAMP
            WHERE id = $3
            RETURNING
                id,
                session_id,
                username,
                hashed_password,
                role AS "role: UserRoleType",
                created_at,
                updated_at,
                deleted_at
            "#,
            values.session_id,
            values.hashed_password,
            user.id,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map(User::from_sqlx_user)
        .map_err(connection_error)?;

        Ok(user)
    }

    /// Soft-delete a user
    pub async fn delete_user(&self, user: &User) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE users
            SET deleted_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
            &user.id,
        )
        .execute(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(())
    }

    /// Find all destinations
    ///
    /// Respects the soft-delete
    pub async fn find_all_destinations(&self) -> Result<Vec<Destination>> {
        let destinations = sqlx::query_as!(
            Destination,
            r#"
            SELECT *
            FROM destinations
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(destinations)
    }

    /// Find a single destination by slug
    ///
    /// DOES NOT respect the soft-delete, handle with care
    pub async fn find_single_destination_by_slug(
        &self,
        slug: &'_ str,
    ) -> Result<Option<Destination>> {
        let destination = sqlx::query_as!(
            Destination,
            r#"
            SELECT *
            FROM destinations
            WHERE slug = $1
            LIMIT 1
            "#,
            slug,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(destination)
    }

    /// Find a single destination by ID
    ///
    /// Respects the soft-delete
    pub async fn find_single_destination_by_id(&self, id: &Uuid) -> Result<Option<Destination>> {
        let destination = sqlx::query_as!(
            Destination,
            r#"
            SELECT *
            FROM destinations
            WHERE deleted_at IS NULL AND id = $1
            LIMIT 1
            "#,
            id,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(destination)
    }

    /// Find a single destination by ID (unchecked)
    ///
    /// DOES NOT respect the soft-delete, handle with care
    pub async fn find_single_destination_by_id_unchecked(
        &self,
        id: &Uuid,
    ) -> Result<Option<Destination>> {
        let destination = sqlx::query_as!(
            Destination,
            r#"
            SELECT *
            FROM destinations
            WHERE id = $1
            LIMIT 1
            "#,
            id,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(destination)
    }

    /// Create a destination
    pub async fn create_destination(
        &self,
        values: &CreateDestinationValues<'_>,
    ) -> Result<Destination> {
        let destination = sqlx::query_as!(
            Destination,
            r#"
            INSERT INTO destinations (id, user_id, slug, url, is_permanent)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
            Uuid::new_v4(),
            values.user.id,
            values.slug,
            values.url.to_string(),
            values.is_permanent,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(destination)
    }

    /// Update a single destination
    pub async fn update_destination(
        &self,
        destination: &Destination,
        values: &UpdateDestinationValues<'_>,
    ) -> Result<Destination> {
        let updated_destination = sqlx::query_as!(
            Destination,
            r#"
            UPDATE destinations
            SET url = $1, is_permanent = $2, updated_at = CURRENT_TIMESTAMP
            WHERE id = $3
            RETURNING *
            "#,
            values
                .url
                .as_ref()
                .map_or(destination.url.clone(), ToString::to_string),
            values.is_permanent.unwrap_or(&destination.is_permanent),
            &destination.id,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(updated_destination)
    }

    /// Soft-delete a destination
    pub async fn delete_destination(&self, destination: &Destination) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE destinations
            SET deleted_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
            &destination.id,
        )
        .execute(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(())
    }

    /// Find all aliases of a destination
    ///
    /// Respects the soft-delete
    pub async fn find_all_aliases_by_destination(
        &self,
        destination: &Destination,
    ) -> Result<Vec<Alias>> {
        let aliases = sqlx::query_as!(
            Alias,
            r#"
            SELECT *
            FROM aliases
            WHERE deleted_at IS NULL AND destination_id = $1
            ORDER BY created_at DESC"#,
            destination.id,
        )
        .fetch_all(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(aliases)
    }

    /// Find all aliases of all destinations
    ///
    /// Respects the soft-delete
    pub async fn find_all_aliases_by_destinations(
        &self,
        destinations: &[Destination],
    ) -> Result<Vec<Alias>> {
        if destinations.is_empty() {
            return Ok(Vec::new());
        }

        let aliases = sqlx::query_as::<_, Alias>(
            r"
            SELECT *
            FROM aliases
            WHERE deleted_at IS NULL AND destination_id = ANY($1)
            ORDER BY created_at DESC",
        )
        .bind(destinations.iter().map(|d| d.id).collect::<Vec<Uuid>>())
        .fetch_all(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(aliases)
    }

    /// Find a single alias by slug
    ///
    /// DOES NOT respect the soft-delete, handle with care
    pub async fn find_single_alias_by_slug(&self, slug: &'_ str) -> Result<Option<Alias>> {
        let alias = sqlx::query_as!(
            Alias,
            r#"
            SELECT *
            FROM aliases
            WHERE slug = $1
            LIMIT 1
            "#,
            slug,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(alias)
    }

    /// Find single alias of a destination
    ///
    /// Respects the soft-delete
    pub async fn find_single_alias_by_id(
        &self,
        destination_id: &Uuid,
        alias_id: &Uuid,
    ) -> Result<Option<Alias>> {
        let alias = sqlx::query_as!(
            Alias,
            r#"
            SELECT *
            FROM aliases
            WHERE deleted_at IS NULL AND destination_id = $1 AND id = $2
            LIMIT 1
            "#,
            destination_id,
            alias_id,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(alias)
    }

    /// Create an alias
    pub async fn create_alias(
        &self,
        destination: &Destination,
        values: &CreateAliasValues<'_>,
    ) -> Result<Alias> {
        let alias = sqlx::query_as!(
            Alias,
            r#"
            INSERT INTO aliases (id, user_id, destination_id, slug)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            Uuid::new_v4(),
            values.user.id,
            destination.id,
            values.slug,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(alias)
    }

    /// Soft-delete an alias
    pub async fn delete_alias(&self, alias: &Alias) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE aliases
            SET deleted_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
            &alias.id,
        )
        .execute(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(())
    }

    /// Find all notes of a destination
    ///
    /// Respects the soft-delete
    pub async fn find_all_notes_by_destination(
        &self,
        destination: &Destination,
    ) -> Result<Vec<Note>> {
        let notes = sqlx::query_as!(
            Note,
            r#"
            SELECT *
            FROM notes
            WHERE deleted_at IS NULL AND destination_id = $1
            ORDER BY created_at DESC"#,
            destination.id,
        )
        .fetch_all(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(notes)
    }

    /// Find all notes of all destinations
    ///
    /// Respects the soft-delete
    pub async fn find_all_notes_by_destinations(
        &self,
        destinations: &[Destination],
    ) -> Result<Vec<Note>> {
        if destinations.is_empty() {
            return Ok(Vec::new());
        }

        let aliases = sqlx::query_as::<_, Note>(
            r"
            SELECT *
            FROM notes
            WHERE deleted_at IS NULL AND destination_id = ANY($1)
            ORDER BY created_at DESC",
        )
        .bind(destinations.iter().map(|d| d.id).collect::<Vec<Uuid>>())
        .fetch_all(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(aliases)
    }

    /// Find single note of a destination
    ///
    /// Respects the soft-delete
    pub async fn find_single_note_by_id(
        &self,
        destination_id: &Uuid,
        note_id: &Uuid,
    ) -> Result<Option<Note>> {
        let note = sqlx::query_as!(
            Note,
            r#"
            SELECT *
            FROM notes
            WHERE deleted_at IS NULL AND destination_id = $1 AND id = $2
            LIMIT 1
            "#,
            destination_id,
            note_id,
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(note)
    }

    /// Create a note
    pub async fn create_note(
        &self,
        destination: &Destination,
        values: &CreateNoteValues<'_>,
    ) -> Result<Note> {
        let note = sqlx::query_as!(
            Note,
            r#"
            INSERT INTO notes (id, user_id, destination_id, content)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            Uuid::new_v4(),
            values.user.id,
            destination.id,
            values.content,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(note)
    }

    /// Update a note
    pub async fn update_note(&self, note: &Note, values: &UpdateNoteValues<'_>) -> Result<Note> {
        let updated_note = sqlx::query_as!(
            Note,
            r#"
            UPDATE notes
            SET content = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            RETURNING *
            "#,
            values.content.unwrap_or(&note.content),
            &note.id,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(updated_note)
    }

    /// Soft-delete a note
    pub async fn delete_note(&self, note: &Note) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE notes
            SET deleted_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
            &note.id,
        )
        .execute(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(())
    }

    /// Save a hit on a destination
    pub async fn save_hit(
        &self,
        destination: &Destination,
        alias: Option<&Alias>,
        ip_address: Option<&IpAddr>,
        user_agent: Option<&String>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO hits (id, destination_id, alias_id, ip_address, user_agent)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            Uuid::new_v4(),
            destination.id,
            alias.map(|a| a.id),
            ip_address
                .map(ToString::to_string)
                .and_then(|ip| ip.parse::<IpNetwork>().ok()),
            user_agent,
        )
        .execute(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(())
    }

    /// Register a creative/destructive action on the audit trail
    pub async fn register_audit_trail(
        &self,
        created_by: &User,
        entry: &AuditEntry<'_>,
        ip_address: Option<&IpAddr>,
    ) -> Result<()> {
        let (user_id, destination_id, alias_is, note_id) = match entry {
            AuditEntry::CreateUser(user)
            | AuditEntry::ChangePassword(user)
            | AuditEntry::DeleteUser(user) => (Some(user.id), None, None, None),

            AuditEntry::CreateDestination(destination)
            | AuditEntry::UpdateDestination(destination)
            | AuditEntry::DeleteDestination(destination) => {
                (None, Some(destination.id), None, None)
            }

            AuditEntry::CreateAlias(destination, alias)
            | AuditEntry::DeleteAlias(destination, alias) => {
                (None, Some(destination.id), Some(alias.id), None)
            }

            AuditEntry::CreateNote(destination, note)
            | AuditEntry::UpdateNote(destination, note)
            | AuditEntry::DeleteNote(destination, note) => {
                (None, Some(destination.id), None, Some(note.id))
            }
        };

        sqlx::query!(
            r#"
            INSERT INTO audit_trail (id, type, created_by, user_id, destination_id, alias_id, note_id, ip_address)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            Uuid::new_v4(),
            AuditEntryType::from_audit_entry(entry) as _,
            created_by.id,
            user_id,
            destination_id,
            alias_is,
            note_id,
            ip_address
                .map(ToString::to_string)
                .and_then(|ip| ip.parse::<IpNetwork>().ok()),
        )
        .execute(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        Ok(())
    }
}

/// Convert `SQLx` to storage connection error
fn connection_error<E>(err: E) -> Error
where
    E: std::error::Error,
{
    Error::Connection(err.to_string())
}

/// The result of trying to fetch a destination by slug
#[derive(Debug)]
pub enum SlugFoundSummary {
    /// The destination exists and is not deleted
    DestinationExists(Destination),

    /// The destination exists but is deleted
    DestinationDeleted(Destination, Option<Alias>),

    /// The alias exists and is not deleted
    AliasExists(Alias, Destination),

    /// The alias exists but is deleted
    AliasDeleted(Alias, Destination),
}

impl SlugFoundSummary {
    /// Get the destination if it exists
    pub fn destination(&self) -> &Destination {
        match self {
            Self::DestinationExists(destination)
            | Self::DestinationDeleted(destination, _)
            | Self::AliasExists(_, destination)
            | Self::AliasDeleted(_, destination) => destination,
        }
    }

    /// Get the alias if it exists
    pub fn alias(&self) -> Option<&Alias> {
        match self {
            SlugFoundSummary::DestinationExists(_) => None,
            Self::DestinationDeleted(_, alias) => alias.as_ref(),
            Self::AliasExists(alias, _) | Self::AliasDeleted(alias, _) => Some(alias),
        }
    }

    /// Is the destination or alias deleted?
    pub fn is_deleted(&self) -> bool {
        matches!(self, Self::DestinationDeleted(..) | Self::AliasDeleted(..))
    }
}

/// Fetch destination from database by slug or alias slug
pub async fn fetch_destination_by_slug(
    database: &Database,
    slug: &str,
) -> Result<Option<SlugFoundSummary>> {
    let destination = database.find_single_destination_by_slug(slug).await?;

    if let Some(destination) = destination {
        return if destination.is_deleted() {
            Ok(Some(SlugFoundSummary::DestinationDeleted(
                destination,
                None,
            )))
        } else {
            Ok(Some(SlugFoundSummary::DestinationExists(destination)))
        };
    }

    let alias = database.find_single_alias_by_slug(slug).await?;

    if let Some(alias) = alias {
        let destination = database
            .find_single_destination_by_id_unchecked(&alias.destination_id)
            .await?
            .expect("Alias has valid destination_id");

        if destination.is_deleted() {
            Ok(Some(SlugFoundSummary::DestinationDeleted(
                destination,
                Some(alias),
            )))
        } else if alias.is_deleted() {
            Ok(Some(SlugFoundSummary::AliasDeleted(alias, destination)))
        } else {
            Ok(Some(SlugFoundSummary::AliasExists(alias, destination)))
        }
    } else {
        Ok(None)
    }
}
