//!Postgres storage

use std::net::IpAddr;
use std::time::Duration;

use axum::async_trait;
use chrono::NaiveDateTime;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::ipnetwork::IpNetwork;
use sqlx::PgPool;
use uuid::Uuid;

use crate::destinations::Destination;
use crate::notes::Note;
use crate::users::Role;
use crate::users::User;

use super::AuditEntry;
use super::ChangePasswordValues;
use super::CreateDestinationValues;
use super::CreateNoteValues;
use super::CreateUserValues;
use super::Error;
use super::Result;
use super::Storage;
use super::UpdateDestinationValues;
use super::UpdateNoteValues;

/// Migrator to run migrations on startup
static MIGRATOR: Migrator = sqlx::migrate!();

/// Postgres type for user role
#[derive(PartialEq, Debug, sqlx::Type)]
#[sqlx(type_name = "user_role_type")]
#[sqlx(rename_all = "kebab-case")]
enum UserRoleType {
    /// Admin
    Admin,

    /// Manager
    Manager,
}

impl UserRoleType {
    /// Create user role type from role
    fn from_role(role: Role) -> Self {
        match role {
            Role::Admin => UserRoleType::Admin,
            Role::Manager => UserRoleType::Manager,
        }
    }

    /// Create role from user role type
    fn to_role(&self) -> Role {
        match self {
            UserRoleType::Admin => Role::Admin,
            UserRoleType::Manager => Role::Manager,
        }
    }
}

/// Postgres type for audit trail entry type
#[derive(PartialEq, Debug, sqlx::Type)]
#[sqlx(type_name = "audit_trail_entry_type")]
#[sqlx(rename_all = "kebab-case")]
enum AuditEntryType {
    /// User is created
    CreateUser,

    /// User has changed password
    ChangePassword,

    /// User is deleted
    DeleteUser,

    /// Destination is created
    CreateDestination,

    /// Destination is updated
    UpdateDestination,

    /// Destination is deleted
    DeleteDestination,

    /// Note is deleted
    CreateNote,

    /// Note is updated
    UpdateNote,

    /// Note is deleted
    DeleteNote,
}

impl AuditEntryType {
    /// Create audit entry type type from audit entry
    fn from_audit_entry(entry: &AuditEntry) -> Self {
        match entry {
            AuditEntry::CreateUser(_) => Self::CreateUser,
            AuditEntry::ChangePassword(_) => Self::ChangePassword,
            AuditEntry::DeleteUser(_) => Self::DeleteUser,

            AuditEntry::CreateDestination(_) => Self::CreateDestination,
            AuditEntry::UpdateDestination(_) => Self::UpdateDestination,
            AuditEntry::DeleteDestination(_) => Self::DeleteDestination,

            AuditEntry::CreateNote(_, _) => Self::CreateNote,
            AuditEntry::UpdateNote(_, _) => Self::UpdateNote,
            AuditEntry::DeleteNote(_, _) => Self::DeleteNote,
        }
    }
}

/// Postgres storage
#[derive(Clone)]
pub struct Postgres {
    /// Pool of connections
    connection_pool: PgPool,
}

impl Postgres {
    /// Create Postgres storage
    ///
    /// Use the `DATABASE_URL` environment variable
    ///
    /// Migrations will be run
    pub async fn new() -> Self {
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
    pub async fn new_with_pool(connection_pool: PgPool) -> Self {
        let migration_result = MIGRATOR.run(&connection_pool).await;

        if let Err(err) = migration_result {
            panic!("Migrations could not run: {err}");
        }

        Self { connection_pool }
    }
}

/// Postgres version of user
struct PostgresUser {
    /// User ID
    id: Uuid,

    /// Sessions ID
    session_id: Uuid,

    /// Username
    username: String,

    /// Hashed password
    hashed_password: String,

    /// User role
    role: UserRoleType,

    /// Creation date
    created_at: NaiveDateTime,

    /// Last updated at
    updated_at: NaiveDateTime,

    /// Deleted at
    deleted_at: Option<NaiveDateTime>,
}

impl User {
    /// Create user from postgres version
    fn from_postgres_user(user: PostgresUser) -> Self {
        Self {
            id: user.id,
            session_id: user.session_id,
            username: user.username,
            hashed_password: user.hashed_password,
            role: user.role.to_role(),
            created_at: user.created_at,
            updated_at: user.updated_at,
            deleted_at: user.deleted_at,
        }
    }

    /// Maybe create user from postgres version
    fn from_postgres_user_optional(user: Option<PostgresUser>) -> Option<Self> {
        user.map(Self::from_postgres_user)
    }

    /// Create multiple user from postgres version
    fn from_postgres_user_multiple(mut users: Vec<PostgresUser>) -> Vec<Self> {
        users
            .drain(..)
            .map(Self::from_postgres_user)
            .collect::<Vec<Self>>()
    }
}

#[async_trait]
impl Storage for Postgres {
    async fn find_any_single_user(&self) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            PostgresUser,
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
        .map(User::from_postgres_user_optional)
        .map_err(connection_error)?;

        Ok(user)
    }

    async fn find_all_users(&self) -> Result<Vec<User>> {
        let users = sqlx::query_as!(
            PostgresUser,
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
        .map(User::from_postgres_user_multiple)
        .map_err(connection_error)?;

        Ok(users)
    }

    async fn find_single_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            PostgresUser,
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
        .map(User::from_postgres_user_optional)
        .map_err(connection_error)?;

        Ok(user)
    }

    async fn find_single_user_by_id(&self, id: &Uuid) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            PostgresUser,
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
        .map(User::from_postgres_user_optional)
        .map_err(connection_error)?;

        Ok(user)
    }

    async fn create_user(&self, values: &CreateUserValues) -> Result<User> {
        let user = sqlx::query_as!(
            PostgresUser,
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
        .map(User::from_postgres_user)
        .map_err(connection_error)?;

        Ok(user)
    }

    async fn change_password(&self, user: &User, values: &ChangePasswordValues) -> Result<User> {
        let user = sqlx::query_as!(
            PostgresUser,
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
        .map(User::from_postgres_user)
        .map_err(connection_error)?;

        Ok(user)
    }

    async fn delete_user(&self, user: &User) -> Result<()> {
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

    async fn find_all_destinations(&self) -> Result<Vec<Destination>> {
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

    async fn find_single_destination_by_slug(&self, slug: &'_ str) -> Result<Option<Destination>> {
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

    async fn find_single_destination_by_id(&self, id: &Uuid) -> Result<Option<Destination>> {
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

    async fn create_destination(&self, values: &CreateDestinationValues) -> Result<Destination> {
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

    async fn update_destination(
        &self,
        destination: &Destination,
        values: &UpdateDestinationValues,
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

    async fn delete_destination(&self, destination: &Destination) -> Result<()> {
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

    async fn find_all_notes_by_destination(&self, destination: &Destination) -> Result<Vec<Note>> {
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

    async fn find_single_note_by_id(
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

    async fn create_note(
        &self,
        destination: &Destination,
        values: &CreateNoteValues,
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

    async fn update_note(&self, note: &Note, values: &UpdateNoteValues) -> Result<Note> {
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

    async fn delete_note(&self, note: &Note) -> Result<()> {
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

    async fn save_hit(
        &self,
        destination: &Destination,
        ip_address: Option<&IpAddr>,
        user_agent: Option<&String>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO hits (id, destination_id, ip_address, user_agent)
            VALUES ($1, $2, $3, $4)
            "#,
            Uuid::new_v4(),
            destination.id,
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

    async fn register_audit_trail(
        &self,
        created_by: &User,
        entry: &AuditEntry,
        ip_address: Option<&IpAddr>,
    ) -> Result<()> {
        let (user_id, destination_id, note_id) = match entry {
            AuditEntry::CreateUser(user)
            | AuditEntry::ChangePassword(user)
            | AuditEntry::DeleteUser(user) => (Some(user.id), None, None),

            AuditEntry::CreateDestination(destination)
            | AuditEntry::UpdateDestination(destination)
            | AuditEntry::DeleteDestination(destination) => (None, Some(destination.id), None),

            AuditEntry::CreateNote(destination, note)
            | AuditEntry::UpdateNote(destination, note)
            | AuditEntry::DeleteNote(destination, note) => {
                (None, Some(destination.id), Some(note.id))
            }
        };

        sqlx::query!(
            r#"
            INSERT INTO audit_trail (id, type, created_by, user_id, destination_id, note_id, ip_address)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            Uuid::new_v4(),
            AuditEntryType::from_audit_entry(entry) as _,
            created_by.id,
            user_id,
            destination_id,
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
