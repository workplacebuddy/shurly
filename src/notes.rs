use chrono::naive::NaiveDateTime;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Note {
    pub id: Uuid,
    pub user_id: Uuid,
    pub destination_id: Uuid,
    pub content: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}
