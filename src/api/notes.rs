use axum::Extension;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::destinations::Destination;
use crate::notes::Note;
use crate::storage::AuditEntry;
use crate::storage::CreateNoteValues;
use crate::storage::Storage;
use crate::storage::UpdateNoteValues;
use crate::users::Role;

use super::AuditTrail;
use super::CurrentUser;
use super::Error;
use super::Form;
use super::PathParameters;
use super::Success;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteResponse {
    pub id: Uuid,
    pub content: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl NoteResponse {
    fn from_note(note: Note) -> Self {
        Self {
            id: note.id,
            content: note.content,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }

    fn from_note_multiple(mut notes: Vec<Note>) -> Vec<Self> {
        notes.drain(..).map(Self::from_note).collect::<Vec<Self>>()
    }
}

pub async fn list<S: Storage>(
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(destination_id): PathParameters<Uuid>,
) -> Result<Success<Vec<NoteResponse>>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = get_destination(&storage, &destination_id).await?;

    let notes = storage
        .find_all_notes_by_destination(&destination)
        .await
        .map_err(Error::internal_server_error)?;

    Ok(Success::ok(NoteResponse::from_note_multiple(notes)))
}

pub async fn single<S: Storage>(
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters((destination_id, note_id)): PathParameters<(Uuid, Uuid)>,
) -> Result<Success<NoteResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = get_destination(&storage, &destination_id).await?;

    get_note(&storage, &destination.id, &note_id)
        .await
        .map(|note| Success::ok(NoteResponse::from_note(note)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteForm {
    content: String,
}

pub async fn create<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(destination_id): PathParameters<Uuid>,
    Form(form): Form<CreateNoteForm>,
) -> Result<Success<NoteResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = get_destination(&storage, &destination_id).await?;

    let values = CreateNoteValues {
        user: &current_user,
        content: &form.content,
    };

    let note = storage
        .create_note(&destination, &values)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::CreateNote(&destination, &note))
        .await;

    Ok(Success::created(NoteResponse::from_note(note)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNoteForm {
    content: Option<String>,
}

pub async fn update<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters((destination_id, note_id)): PathParameters<(Uuid, Uuid)>,
    Form(form): Form<UpdateNoteForm>,
) -> Result<Success<NoteResponse>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = get_destination(&storage, &destination_id).await?;
    let note = get_note(&storage, &destination.id, &note_id).await?;

    let values = UpdateNoteValues {
        content: form.content.as_ref(),
    };

    let note = storage
        .update_note(&note, &values)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::UpdateNote(&destination, &note))
        .await;

    Ok(Success::ok(NoteResponse::from_note(note)))
}

pub async fn delete<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters((destination_id, note_id)): PathParameters<(Uuid, Uuid)>,
) -> Result<Success<&'static str>, Error> {
    current_user.role.is_allowed(Role::Manager)?;

    let destination = get_destination(&storage, &destination_id).await?;
    let note = get_note(&storage, &destination.id, &note_id).await?;

    storage
        .delete_note(&note)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::DeleteNote(&destination, &note))
        .await;

    Ok(Success::<&'static str>::no_content())
}

async fn get_destination<S: Storage>(
    storage: &S,
    destination_id: &Uuid,
) -> Result<Destination, Error> {
    storage
        .find_single_destination_by_id(destination_id)
        .await
        .map_err(Error::internal_server_error)?
        .map_or_else(|| Err(Error::not_found("Destination not found")), Ok)
}

async fn get_note<S: Storage>(
    storage: &S,
    destination_id: &Uuid,
    note_id: &Uuid,
) -> Result<Note, Error> {
    storage
        .find_single_note_by_id(destination_id, note_id)
        .await
        .map_err(Error::internal_server_error)?
        .map_or_else(|| Err(Error::not_found("Note not found")), Ok)
}
