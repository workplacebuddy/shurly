//! Database storage types and functions

use chrono::NaiveDateTime;
use sqlx::migrate::Migrator;
use uuid::Uuid;

use crate::users::Role;
use crate::users::User;

use super::AuditEntry;

/// Migrator to run migrations on startup
pub static MIGRATOR: Migrator = sqlx::migrate!();

/// `SQLx` type for user role
#[derive(PartialEq, Debug, sqlx::Type)]
#[sqlx(type_name = "user_role_type")]
#[sqlx(rename_all = "kebab-case")]
pub enum UserRoleType {
    /// Admin
    Admin,

    /// Manager
    Manager,
}

impl UserRoleType {
    /// Create user role type from role
    pub fn from_role(role: Role) -> Self {
        match role {
            Role::Admin => UserRoleType::Admin,
            Role::Manager => UserRoleType::Manager,
        }
    }

    /// Create role from user role type
    pub fn to_role(&self) -> Role {
        match self {
            UserRoleType::Admin => Role::Admin,
            UserRoleType::Manager => Role::Manager,
        }
    }
}

/// `SQLx` type for audit trail entry type
#[derive(PartialEq, Debug, sqlx::Type)]
#[sqlx(type_name = "audit_trail_entry_type")]
#[sqlx(rename_all = "kebab-case")]
pub enum AuditEntryType {
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

    /// Alias is created
    CreateAlias,

    /// Alias is deleted
    DeleteAlias,

    /// Note is created
    CreateNote,

    /// Note is updated
    UpdateNote,

    /// Note is deleted
    DeleteNote,
}

impl AuditEntryType {
    /// Create audit entry type type from audit entry
    pub fn from_audit_entry(entry: &AuditEntry) -> Self {
        match entry {
            AuditEntry::CreateUser(_) => Self::CreateUser,
            AuditEntry::ChangePassword(_) => Self::ChangePassword,
            AuditEntry::DeleteUser(_) => Self::DeleteUser,

            AuditEntry::CreateDestination(_) => Self::CreateDestination,
            AuditEntry::UpdateDestination(_) => Self::UpdateDestination,
            AuditEntry::DeleteDestination(_) => Self::DeleteDestination,

            AuditEntry::CreateAlias(_, _) => Self::CreateAlias,
            AuditEntry::DeleteAlias(_, _) => Self::DeleteAlias,

            AuditEntry::CreateNote(_, _) => Self::CreateNote,
            AuditEntry::UpdateNote(_, _) => Self::UpdateNote,
            AuditEntry::DeleteNote(_, _) => Self::DeleteNote,
        }
    }
}

/// `SQLx` version of user
pub struct SqlxUser {
    /// User ID
    pub id: Uuid,

    /// Sessions ID
    pub session_id: Uuid,

    /// Username
    pub username: String,

    /// Hashed password
    pub hashed_password: String,

    /// User role
    pub role: UserRoleType,

    /// Creation date
    pub created_at: NaiveDateTime,

    /// Last updated at
    pub updated_at: NaiveDateTime,

    /// Deleted at
    pub deleted_at: Option<NaiveDateTime>,
}

impl User {
    /// Create user from `SQLx` version
    pub fn from_sqlx_user(user: SqlxUser) -> Self {
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

    /// Maybe create user from `SQLx` version
    pub fn from_sqlx_user_optional(user: Option<SqlxUser>) -> Option<Self> {
        user.map(Self::from_sqlx_user)
    }

    /// Create multiple user from `SQLx` version
    pub fn from_sqlx_user_multiple(mut users: Vec<SqlxUser>) -> Vec<Self> {
        users
            .drain(..)
            .map(Self::from_sqlx_user)
            .collect::<Vec<Self>>()
    }
}
