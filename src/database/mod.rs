//! All things related to the storage of destinations and notes

use core::fmt;
use std::net::IpAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike as _;
use chrono::Utc;
use moka::future::Cache;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::ipnetwork::IpNetwork;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub use Config as DatabaseConfig;
pub use form_types::*;

use crate::aliases::Alias;
use crate::destinations::Destination;
use crate::notes::Note;
use crate::users::User;
use types::AuditEntryType;
use types::MIGRATOR;
use types::SqlxUser;
use types::UserRoleType;

mod form_types;
mod types;

/// Storage errors
#[derive(Debug)]
pub enum Error {
    /// A connection error with the storage
    Connection(String),

    /// A problem scheduling a page hit save
    PageHitScheduling(String),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Connection(error) => write!(f, "Connection error: {error}"),
            Error::PageHitScheduling(error) => {
                write!(f, "Page hit scheduling error: {error}")
            }
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

/// Handler to initiate database shutdown
///
/// Only stops the page hit collector
#[derive(Default, Clone)]
pub struct DatabaseShutdownHandler {
    /// Internal cancellation token to see if shutdown is initiated
    is_shutting_token: CancellationToken,

    /// Internal cancellation token to see if shutdown is completed
    is_shutdown_completed_token: CancellationToken,
}

impl DatabaseShutdownHandler {
    /// Trigger the shutdown
    pub fn shutdown(&self) {
        self.is_shutting_token.cancel();
    }

    /// Make the shutdown as complete
    fn complete(&self) {
        self.is_shutdown_completed_token.cancel();
    }

    /// Is the shutdown completed
    pub async fn completed(&self) {
        self.is_shutdown_completed_token.cancelled().await;
    }
}

/// The capacity of the page hit collecto channel
///
/// This influences the performance of the root endpoint, it's the buffer for how many page hits
/// can be scheduled before the page hits actually need to be stored. Bursts of thousands of
/// requests will saturate this and later will requests will need to wait a bit, making the
/// database connection the slow factor in those requests.
const PAGE_HIT_COLLECTOR_CHANNEL_CAPACITY: usize = 10_000;

/// Postgres storage
#[derive(Clone)]
pub struct Database {
    /// Pool of connections
    connection_pool: PgPool,

    /// Cache for the slug found summaries
    slug_found_cache: SlugFoundCache,

    /// Channel sender to schedule page hit saves
    page_hit_sender: mpsc::Sender<PageHitInformation>,
}

/// Page hit information
struct PageHitInformation {
    /// The destination ID
    destination_id: Uuid,

    /// The alias ID
    alias_id: Option<Uuid>,

    /// The IP address
    ip_address: Option<IpAddr>,

    /// The user agent
    user_agent: Option<String>,

    /// The moment this page hit happened
    when: DateTime<Utc>,
}

impl Database {
    /// Create a new Postgres storage
    pub async fn from_config(config: Config, shutdown_handler: DatabaseShutdownHandler) -> Self {
        match config {
            Config::DetectConfig => Self::new(shutdown_handler).await,
            Config::ExistingConnection(pool) => Self::new_with_pool(pool, shutdown_handler).await,
        }
    }

    /// Create Postgres storage
    ///
    /// Use the `DATABASE_URL` environment variable
    ///
    /// Migrations will be run
    async fn new(shutdown_handler: DatabaseShutdownHandler) -> Self {
        let database_connection_string = std::env::var("DATABASE_URL").expect("Valid DATABASE_URL");

        let connection_pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(&database_connection_string)
            .await
            .expect("Valid connection");

        Self::new_with_pool(connection_pool, shutdown_handler).await
    }

    /// Create Postgres storage with existing pool
    ///
    /// Migrations will be run
    async fn new_with_pool(
        connection_pool: PgPool,
        shutdown_handler: DatabaseShutdownHandler,
    ) -> Self {
        let migration_result = MIGRATOR.run(&connection_pool).await;

        if let Err(err) = migration_result {
            panic!("Migrations could not run: {err}");
        }

        let (page_hit_sender, mut page_hit_receiver) =
            mpsc::channel(PAGE_HIT_COLLECTOR_CHANNEL_CAPACITY);

        let database = Self {
            connection_pool,
            slug_found_cache: SlugFoundCache::default(),
            page_hit_sender,
        };

        let database_ = database.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;

                    page_hit_info = page_hit_receiver.recv() => {
                        if let Some(page_hit_info) = page_hit_info {
                            if let Err(err) = database_.save_hit(page_hit_info).await {
                                tracing::error!("Failed to save page hit: {err}");
                            }
                        } else {
                            tracing::warn!("Page hit receiver channel closed");
                        }
                    }

                    () = shutdown_handler.is_shutting_token.cancelled() => {
                        tracing::trace!("Page hit channel cancellled");

                        // the biased select has handled all remaining page hits
                        shutdown_handler.complete();
                        break;
                    }
                }
            }
        });

        database
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
            INSERT INTO destinations (id, user_id, slug, url, is_permanent, forward_query_parameters)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
            Uuid::new_v4(),
            values.user.id,
            values.slug,
            values.url.to_string(),
            values.is_permanent,
            values.forward_query_parameters,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        self.slug_found_cache.invalidate(values.slug).await;

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
            SET url = $1, is_permanent = $2, forward_query_parameters = $3, updated_at = CURRENT_TIMESTAMP
            WHERE id = $4
            RETURNING *
            "#,
            values
                .url
                .as_ref()
                .map_or(destination.url.clone(), ToString::to_string),
            values.is_permanent.unwrap_or(&destination.is_permanent),
            values
                .forward_query_parameters
                .unwrap_or(&destination.forward_query_parameters),
            &destination.id,
        )
        .fetch_one(&self.connection_pool)
        .await
        .map_err(connection_error)?;

        self.slug_found_cache.invalidate(&destination.slug).await;

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

        self.slug_found_cache.invalidate(&destination.slug).await;

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

        self.slug_found_cache.invalidate(values.slug).await;

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

        self.slug_found_cache.invalidate(&alias.slug).await;

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

    /// Schedule saving a hit in the background
    pub async fn schedule_save_hit(
        &self,
        destination_id: Uuid,
        alias_id: Option<Uuid>,
        ip_address: Option<IpAddr>,
        user_agent: Option<String>,
    ) -> Result<()> {
        self.page_hit_sender
            .send(PageHitInformation {
                destination_id,
                alias_id,
                ip_address,
                user_agent,
                // capture the moment the page hit happened, do no rely on the database to
                // set this when the record is inserted, it could be delayed back pressure
                when: Utc::now(),
            })
            .await
            .map_err(|err| Error::PageHitScheduling(format!("Could not schedule page hit: {err}")))
    }

    /// Save a hit on a destination
    async fn save_hit(&self, page_hit: PageHitInformation) -> Result<()> {
        #[expect(deprecated)] // sqlx expect a `NaiveDateTime`
        sqlx::query!(
            r#"
            INSERT INTO hits (id, destination_id, alias_id, ip_address, user_agent, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            Uuid::new_v4(),
            page_hit.destination_id,
            page_hit.alias_id,
            page_hit.ip_address.map(IpNetwork::from),
            page_hit.user_agent,
            NaiveDateTime::from_timestamp(page_hit.when.timestamp(), page_hit.when.nanosecond(),),
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

    /// Fetch (cached) destination from database by slug or alias slug
    pub async fn fetch_cached_destination_by_slug(
        &self,
        slug: &str,
    ) -> core::result::Result<Arc<Option<SlugFoundSummary>>, Arc<Error>> {
        self.slug_found_cache
            .try_get_with_by_ref(slug, async {
                fetch_destination_by_slug(self, slug).await.map(Arc::new)
            })
            .await
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

/// The default maximum capacity of the slug found cache
const DEFAULT_CACHE_MAX_CAPACITY: u64 = 10_000;

/// Cache for the slug found summaries
#[derive(Clone)]
struct SlugFoundCache(Cache<String, Arc<Option<SlugFoundSummary>>>);

impl Default for SlugFoundCache {
    fn default() -> Self {
        Self(Cache::new(DEFAULT_CACHE_MAX_CAPACITY))
    }
}

impl Deref for SlugFoundCache {
    type Target = Cache<String, Arc<Option<SlugFoundSummary>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
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
