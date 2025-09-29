//! Aliases

use chrono::naive::NaiveDateTime;
use uuid::Uuid;

/// Alias for a destination
#[derive(Clone, Debug)]
pub struct Alias {
    /// Destination ID
    pub id: Uuid,

    /// The ID of the user that created it
    #[expect(dead_code)] // used by sqlx
    pub user_id: Uuid,

    /// External identifier for the alias
    pub slug: String,

    /// Location where the destination goes
    pub destination_id: Uuid,

    /// Creation date
    pub created_at: NaiveDateTime,

    /// Last updated at
    pub updated_at: NaiveDateTime,

    /// Soft-deleted at
    pub deleted_at: Option<NaiveDateTime>,
}

impl Alias {
    /// Is the alias soft-deleted?
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
