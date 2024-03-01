//! Form types

use url::Url;
use uuid::Uuid;

use crate::destinations::Destination;
use crate::notes::Note;
use crate::users::Role;
use crate::users::User;

/// Values to create a User
pub struct CreateUserValues<'a> {
    /// The initial session ID for the user
    pub session_id: &'a Uuid,

    /// The role of the user
    pub role: Role,

    /// The username
    pub username: &'a str,

    /// The hashed password
    pub hashed_password: &'a str,
}

/// Values to change a password of a user
pub struct ChangePasswordValues<'a> {
    /// New session ID to invalidate current tokens
    pub session_id: &'a Uuid,

    /// The new hashed password
    pub hashed_password: &'a str,
}

/// Values to create a Destination
pub struct CreateDestinationValues<'a> {
    /// The user creating the destination
    pub user: &'a User,

    /// The slug of the destination
    pub slug: &'a str,

    /// The URL the destination redirects to
    pub url: &'a Url,

    /// Make the destination as permanent
    pub is_permanent: &'a bool,
}

/// Values to update an Destination
pub struct UpdateDestinationValues<'a> {
    /// New (optional) url of the destination
    pub url: Option<Url>,

    /// Type to update destination with
    ///
    /// Can only be set to `false` if the destination already has `is_permanent=true`, otherwise
    /// only `true` is valid
    pub is_permanent: Option<&'a bool>,
}

/// Values to create an Note
pub struct CreateNoteValues<'a> {
    /// User creating the note
    pub user: &'a User,

    /// Content of the note
    ///
    /// Can be anything
    pub content: &'a str,
}

/// Values to update an Note
pub struct UpdateNoteValues<'a> {
    /// New content of the note
    pub content: Option<&'a String>,
}

/// Possible audit trail entry types
pub enum AuditEntry<'a> {
    /// User is created
    CreateUser(&'a User),

    /// User has a changed password
    ChangePassword(&'a User),

    /// User is deleted
    DeleteUser(&'a User),

    /// Destination is created
    CreateDestination(&'a Destination),

    /// Destination is updated
    UpdateDestination(&'a Destination),

    /// Destination is deleted
    DeleteDestination(&'a Destination),

    /// Note is created
    CreateNote(&'a Destination, &'a Note),

    /// Note is updated
    UpdateNote(&'a Destination, &'a Note),

    /// Note is deleted
    DeleteNote(&'a Destination, &'a Note),
}
