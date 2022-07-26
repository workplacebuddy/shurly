//! Memory storage
//!
//! Will be destroyed on system shutdown

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use axum::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::destinations::Destination;
use crate::notes::Note;
use crate::users::User;

use super::AuditEntry;
use super::ChangePasswordValues;
use super::CreateDestinationValues;
use super::CreateNoteValues;
use super::CreateUserValues;
use super::Result;
use super::Storage;
use super::UpdateDestinationValues;
use super::UpdateNoteValues;

/// An in-memory storage
///
/// Will be destroyed on system shutdown
#[derive(Clone, Debug)]
pub struct Memory {
    /// All users in storage
    users: Arc<Mutex<HashMap<Uuid, User>>>,

    /// All destinations in storage
    destinations: Arc<Mutex<HashMap<Uuid, Destination>>>,

    /// All notes in storage
    notes: Arc<Mutex<HashMap<Uuid, Note>>>,
}

impl Memory {
    /// Create a new empty Memory storage
    pub fn new() -> Self {
        Self {
            users: Arc::new(Mutex::new(HashMap::new())),
            destinations: Arc::new(Mutex::new(HashMap::new())),
            notes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Storage for Memory {
    async fn find_any_single_user(&self) -> Result<Option<User>> {
        Ok(self
            .users
            .lock()
            .await
            .values()
            .find(|user| user.deleted_at.is_none())
            .cloned())
    }

    async fn find_all_users(&self) -> Result<Vec<User>> {
        Ok(self
            .users
            .lock()
            .await
            .values()
            .filter(|user| user.deleted_at.is_none())
            .cloned()
            .collect())
    }

    async fn find_single_user_by_username(&self, username: &str) -> Result<Option<User>> {
        Ok(self
            .users
            .lock()
            .await
            .values()
            .find(|user| user.username == username && user.deleted_at.is_none())
            .cloned())
    }

    async fn find_single_user_by_id(&self, id: &Uuid) -> Result<Option<User>> {
        Ok(self
            .users
            .lock()
            .await
            .values()
            .find(|user| &user.id == id && user.deleted_at.is_none())
            .cloned())
    }

    async fn create_user(&self, values: &CreateUserValues) -> Result<User> {
        let user = User {
            id: Uuid::new_v4(),
            session_id: *values.session_id,
            username: values.username.to_string(),
            hashed_password: values.hashed_password.to_string(),
            role: values.role,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            deleted_at: None,
        };

        self.users.lock().await.insert(user.id, user.clone());

        Ok(user)
    }

    async fn change_password(&self, user: &User, values: &ChangePasswordValues) -> Result<User> {
        Ok(self
            .users
            .lock()
            .await
            .get_mut(&user.id)
            .map(|mut user| {
                user.session_id = *values.session_id;
                user.hashed_password = values.hashed_password.to_string();

                user.clone()
            })
            .expect("HashMap is the source of the user"))
    }

    async fn delete_user(&self, user: &User) -> Result<()> {
        if let Some(mut user) = self.users.lock().await.get_mut(&user.id) {
            user.deleted_at = Some(Utc::now().naive_utc());
        }

        Ok(())
    }

    async fn find_all_destinations(&self) -> Result<Vec<Destination>> {
        Ok(self
            .destinations
            .lock()
            .await
            .values()
            .filter(|destination| destination.deleted_at.is_none())
            .cloned()
            .collect())
    }

    async fn find_single_destination_by_slug(&self, slug: &'_ str) -> Result<Option<Destination>> {
        Ok(self
            .destinations
            .lock()
            .await
            .values()
            .find(|destination| destination.slug == slug)
            .cloned())
    }

    async fn find_single_destination_by_id(&self, id: &Uuid) -> Result<Option<Destination>> {
        Ok(self
            .destinations
            .lock()
            .await
            .values()
            .find(|destination| &destination.id == id && destination.deleted_at.is_none())
            .cloned())
    }

    async fn create_destination(&self, values: &CreateDestinationValues) -> Result<Destination> {
        let destination = Destination {
            id: Uuid::new_v4(),
            user_id: values.user.id,
            slug: values.slug.to_string(),
            url: values.url.to_string(),
            is_permanent: *values.is_permanent,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            deleted_at: None,
        };

        self.destinations
            .lock()
            .await
            .insert(destination.id, destination.clone());

        Ok(destination)
    }

    async fn update_destination(
        &self,
        destination: &Destination,
        values: &UpdateDestinationValues,
    ) -> Result<Destination> {
        Ok(self
            .destinations
            .lock()
            .await
            .get_mut(&destination.id)
            .map(|mut destination| {
                if let Some(url) = &values.url {
                    destination.url = url.to_string();
                }

                if let Some(is_permanent) = &values.is_permanent {
                    destination.is_permanent = **is_permanent;
                }

                destination.clone()
            })
            .expect("HashMap is the source of the destination"))
    }

    async fn delete_destination(&self, destination: &Destination) -> Result<()> {
        if let Some(mut destination) = self.destinations.lock().await.get_mut(&destination.id) {
            destination.deleted_at = Some(Utc::now().naive_utc());
        }

        Ok(())
    }

    async fn find_all_notes_by_destination(&self, destination: &Destination) -> Result<Vec<Note>> {
        Ok(self
            .notes
            .lock()
            .await
            .values()
            .filter_map(|note| {
                if note.destination_id == destination.id && note.deleted_at.is_none() {
                    Some(note.clone())
                } else {
                    None
                }
            })
            .collect())
    }

    async fn find_single_note_by_id(
        &self,
        _destination_id: &Uuid,
        note_id: &Uuid,
    ) -> Result<Option<Note>> {
        Ok(self
            .notes
            .lock()
            .await
            .values()
            .find(|note| &note.id == note_id && note.deleted_at.is_none())
            .cloned())
    }

    async fn create_note(
        &self,
        destination: &Destination,
        values: &CreateNoteValues,
    ) -> Result<Note> {
        let note = Note {
            id: Uuid::new_v4(),
            user_id: values.user.id,
            destination_id: destination.id,
            content: values.content.to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            deleted_at: None,
        };

        self.notes.lock().await.insert(note.id, note.clone());

        Ok(note)
    }

    async fn update_note(&self, note: &Note, values: &UpdateNoteValues) -> Result<Note> {
        Ok(self
            .notes
            .lock()
            .await
            .get_mut(&note.id)
            .map(|mut note| {
                if let Some(content) = values.content {
                    note.content = content.to_string();
                }

                note.clone()
            })
            .expect("HashMap is the source of the note"))
    }

    async fn delete_note(&self, note: &Note) -> Result<()> {
        if let Some(mut note) = self.notes.lock().await.get_mut(&note.id) {
            note.deleted_at = Some(Utc::now().naive_utc());
        }

        Ok(())
    }

    async fn save_hit(
        &self,
        _destination: &Destination,
        _ip_address: Option<&IpAddr>,
        _user_agent: Option<&String>,
    ) -> Result<()> {
        Ok(())
    }

    async fn register_audit_trail(
        &self,
        _created_by: &User,
        _entry: &AuditEntry,
        _ip_address: Option<&IpAddr>,
    ) -> Result<()> {
        Ok(())
    }
}
