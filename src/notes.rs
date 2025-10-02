//! Notes

use chrono::naive::NaiveDateTime;
use sqlx::prelude::FromRow;
use uuid::Uuid;

/// A note in all its glory
#[derive(Clone, Debug, FromRow)]
pub struct Note {
    /// The note ID
    pub id: Uuid,

    /// The ID of the user that created it
    #[allow(dead_code)] // used by sqlx
    pub user_id: Uuid,

    /// Destination this note belongs to
    #[allow(dead_code)] // used by sqlx
    pub destination_id: Uuid,

    /// The actual content of the note
    pub content: String,

    /// Creation date
    pub created_at: NaiveDateTime,

    /// Last updated at
    pub updated_at: NaiveDateTime,

    /// Soft-deleted at
    #[allow(dead_code)] // used by sqlx
    pub deleted_at: Option<NaiveDateTime>,
}
