use chrono::naive::NaiveDateTime;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Destination {
    pub id: Uuid,
    pub user_id: Uuid,
    pub slug: String,
    pub url: String,
    pub is_permanent: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}
