//! Destinations

use chrono::naive::NaiveDateTime;
use uuid::Uuid;

/// Destination in all its glory
#[derive(Clone, Debug)]
pub struct Destination {
    /// Destination ID
    pub id: Uuid,

    /// The ID of the user that created it
    #[allow(dead_code)] // used by sqlx
    pub user_id: Uuid,

    /// External identifier for the root
    pub slug: String,

    /// Location where the destination goes
    pub url: String,

    /// Type of destination
    pub is_permanent: bool,

    /// Creation date
    pub created_at: NaiveDateTime,

    /// Last updated at
    pub updated_at: NaiveDateTime,

    /// Soft-deleted at
    pub deleted_at: Option<NaiveDateTime>,
}

impl Destination {
    /// Is the destination soft-deleted?
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
