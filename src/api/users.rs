//! User API management

use std::collections::HashMap;
use std::ops::Deref;

use axum::Extension;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::password::generate;
use crate::password::hash;
use crate::password::verify;
use crate::storage::AuditEntry;
use crate::storage::ChangePasswordValues;
use crate::storage::CreateUserValues;
use crate::storage::Storage;
use crate::users::Role;
use crate::users::User;

use super::current_user::generate_token;
use super::current_user::Token;
use super::AuditTrail;
use super::CurrentUser;
use super::Error;
use super::Form;
use super::JwtKeys;
use super::PathParameters;
use super::Success;

/// The user response information
///
/// A subset of all the information, ready to be serialized for the outside world
#[derive(Debug, Serialize)]
pub struct UserResponse {
    /// The user ID
    pub id: Uuid,

    /// The username
    pub username: String,

    /// The role of the user
    pub role: Role,

    /// The password, if generated
    // Password should only be added when newly generated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl UserResponse {
    /// Create a user response from a [`User`](User)
    fn from_user(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: user.role,
            password: None,
        }
    }

    /// Add a password to the user response
    ///
    /// This is explicit extra action to take, to make sure this is really what you want to do
    fn set_password(&mut self, password: &str) {
        self.password = Some(password.to_string());
    }

    /// Create a user response from multiple [`User`](User)s
    fn from_user_multiple(mut users: Vec<User>) -> Vec<Self> {
        users.drain(..).map(Self::from_user).collect::<Vec<Self>>()
    }
}

/// Login form
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginForm {
    /// Username of the user
    username: String,
    /// Password of the user
    password: String,
}

/// Get a token for a user "session"
///
/// The token can then be used to access the rest of the API routes by using it in the
/// `Authorization` header
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -d '{ "username": "admin", "password": "verysecret" }' \
///     http://localhost:6000/api/users/token
/// ```
///
/// Response
/// ```json
/// { "data": { "type": "Bearer", "access_token": "some token" } }
/// ```
pub async fn token<S: Storage>(
    Extension(jwt_keys): Extension<JwtKeys>,
    Extension(storage): Extension<S>,
    Form(form): Form<LoginForm>,
) -> Result<Success<Token>, Error> {
    let user = storage
        .find_single_user_by_username(&form.username)
        .await
        .map_err(Error::internal_server_error)?;

    if let Some(user) = user {
        if verify(&user.hashed_password, &form.password) {
            let token = generate_token(&jwt_keys, &user)?;

            Ok(Success::ok(token))
        } else {
            Err(Error::bad_request("Invalid user"))
        }
    } else {
        Err(Error::bad_request("Invalid user"))
    }
}

/// List all users
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:6000/api/users
/// ```
///
/// Response:
/// ```json
/// { "data": [ { "id": "<uuid>", "username": "some-username" ... } ] }
/// ```
pub async fn list<S: Storage>(
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
) -> Result<Success<Vec<UserResponse>>, Error> {
    current_user.role.is_allowed(Role::Admin)?;

    let users = storage
        .find_all_users()
        .await
        .map_err(Error::internal_server_error)?;

    Ok(Success::ok(UserResponse::from_user_multiple(users)))
}

/// Get a single user or the current user
///
/// By passing `me` instead of a user ID, the current user is returned
///
/// Request user:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:6000/api/users/<uuid>
/// ```
///
/// Request me:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:6000/api/users/me
/// ```
///
/// Response:
/// ```json
/// { "data": { "id": "<uuid>", "username": "some-username" ... } }
/// ```
pub async fn single<S: Storage>(
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(params): PathParameters<HashMap<String, Uuid>>,
) -> Result<Success<UserResponse>, Error> {
    let user = if let Some(user_id) = params.get("user") {
        current_user.role.is_allowed(Role::Admin)?;
        fetch_user(&storage, user_id).await?
    } else {
        current_user.role.is_allowed(Role::Manager)?;
        current_user.deref().clone()
    };

    Ok(Success::ok(UserResponse::from_user(user)))
}

/// Create user form
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserForm {
    /// Role of the new user
    role: Role,
    /// Username of the new user
    username: String,
    /// Optional password of the new user
    ///
    /// When not provided a new password will be generated and returned in the response, this will
    /// be the only time the password is visible -- make sure to capture it.
    password: Option<String>,
}

/// Create a user based on the [`CreateUserForm`](CreateUserForm) form
///
/// Request:
/// ```sh
/// curl -v -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     -d '{ "role": "manager", "username": "some-other-username" }' \
///     http://localhost:6000/api/users
/// ```
///
/// Response
/// ```json
/// { "data": { "id": "<uuid>", "username": "some-other-username", "password": "veryverysecret" } }
/// ```
pub async fn create<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    Form(form): Form<CreateUserForm>,
) -> Result<Success<UserResponse>, Error> {
    current_user.role.is_allowed(Role::Admin)?;

    let user = storage
        .find_single_user_by_username(&form.username)
        .await
        .map_err(Error::internal_server_error)?;

    if let Some(user) = user {
        if user.is_deleted() {
            Err(Error::bad_request("User already exists and is deleted"))
        } else {
            Err(Error::bad_request("User already exists"))
        }
    } else {
        let (is_generated, password) = if let Some(password) = form.password {
            (false, password)
        } else {
            (true, generate())
        };

        let hashed_password = hash(&password);

        let values = CreateUserValues {
            session_id: &Uuid::new_v4(),
            role: form.role,
            username: &form.username,
            hashed_password: &hashed_password,
        };

        let user = storage
            .create_user(&values)
            .await
            .map_err(Error::internal_server_error)?;

        audit_trail.register(AuditEntry::CreateUser(&user)).await;

        let mut response = UserResponse::from_user(user);

        // only add the generated password, its the only time the password is known to anybody
        if is_generated {
            response.set_password(&password);
        }

        Ok(Success::created(response))
    }
}

/// Change password form
///
/// New password is optional
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordForm {
    /// Current password for verification
    current_password: String,
    /// New (optional) password
    ///
    /// When not provided a new password will be generated and returned in the response, this will
    /// be the only time the password is visible -- make sure to capture it.
    password: Option<String>,
}

/// Change the password of a user or the current user
///
/// By passing `me` instead of a user ID, the password of the current user is changed
///
/// Changing your password will invalidate your current access token
///
/// Request:
/// ```sh
/// curl -v -XPUT -H 'Content-Type: application/json' \
///     -H 'Authorization: Bearer tokentokentoken' \
///     -d '{ "currentPassword": "verysecret", "password": "veryverysecret" }' \
///     http://localhost:6000/api/destinations
/// ```
///
/// Response
/// ```json
/// { "data": { "type": "Bearer", "access_token": "some token" } }
/// ```
pub async fn change_password<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(jwt_keys): Extension<JwtKeys>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(params): PathParameters<HashMap<String, Uuid>>,
    Form(form): Form<ChangePasswordForm>,
) -> Result<Success<Token>, Error> {
    let user = if let Some(user_id) = params.get("user") {
        current_user.role.is_allowed(Role::Admin)?;
        fetch_user(&storage, user_id).await?
    } else {
        current_user.role.is_allowed(Role::Manager)?;
        current_user.deref().clone()
    };

    if !verify(&user.hashed_password, &form.current_password) {
        return Err(Error::bad_request("Invalid password"));
    }

    let password = form.password.unwrap_or_else(generate);
    let hashed_password = hash(&password);

    let values = ChangePasswordValues {
        session_id: &Uuid::new_v4(),
        hashed_password: &hashed_password,
    };

    let updated_user = storage
        .change_password(&user, &values)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail
        .register(AuditEntry::ChangePassword(&user))
        .await;

    let token = generate_token(&jwt_keys, &updated_user)?;

    Ok(Success::ok(token))
}

/// Delete a user
///
/// Request:
/// ```sh
/// curl -v -XDELETE \
///     -H 'Authorization: Bearer tokentokentoken' \
///     http://localhost:6000/api/users/<uuid>
/// ```
pub async fn delete<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(user_id): PathParameters<Uuid>,
) -> Result<Success<&'static str>, Error> {
    current_user.role.is_allowed(Role::Admin)?;

    let user = fetch_user(&storage, &user_id).await?;

    storage
        .delete_user(&user)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail.register(AuditEntry::DeleteUser(&user)).await;

    Ok(Success::<&'static str>::no_content())
}

/// Fetch a user from storage
async fn fetch_user<S: Storage>(storage: &S, user_id: &Uuid) -> Result<User, Error> {
    storage
        .find_single_user_by_id(user_id)
        .await
        .map_err(Error::internal_server_error)?
        .map_or_else(|| Err(Error::not_found("User not found")), Ok)
}
