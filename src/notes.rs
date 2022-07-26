//! Notes

use chrono::naive::NaiveDateTime;
use uuid::Uuid;

/// A note in all its glory
#[derive(Clone, Debug)]
pub struct Note {
    /// The note ID
    pub id: Uuid,

    /// The ID of the user that created it
    pub user_id: Uuid,

    /// Destination this note belongs to
    pub destination_id: Uuid,

    /// The actual content of the note
    pub content: String,

    /// Creation date
    pub created_at: NaiveDateTime,

    /// Last updated at
    pub updated_at: NaiveDateTime,

    /// Soft-deleted at
    pub deleted_at: Option<NaiveDateTime>,
}

impl Note {
    /// Is the note soft-deleted?
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
