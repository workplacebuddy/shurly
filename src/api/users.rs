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

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub role: Role,
}

impl UserResponse {
    fn from_user(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: user.role,
        }
    }

    fn from_user_multiple(mut users: Vec<User>) -> Vec<Self> {
        users.drain(..).map(Self::from_user).collect::<Vec<Self>>()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginForm {
    username: String,
    password: String,
}

pub async fn token<S: Storage>(
    Extension(jwt_keys): Extension<JwtKeys>,
    Extension(storage): Extension<S>,
    Form(form): Form<LoginForm>,
) -> Result<Success<Token>, Error> {
    const ERROR_MESSAGE: &str = "Invalid user";

    let user = storage
        .find_single_user_by_username(&form.username)
        .await
        .map_err(Error::internal_server_error)?;

    if let Some(user) = user {
        if verify(&user.hashed_password, &form.password) {
            let token = generate_token(&jwt_keys, &user)?;

            Ok(Success::ok(token))
        } else {
            Err(Error::bad_request(ERROR_MESSAGE))
        }
    } else {
        Err(Error::bad_request(ERROR_MESSAGE))
    }
}

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

pub async fn single<S: Storage>(
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(user_id): PathParameters<Uuid>,
) -> Result<Success<UserResponse>, Error> {
    current_user.role.is_allowed(Role::Admin)?;

    get_user(&storage, &user_id)
        .await
        .map(|user| Success::ok(UserResponse::from_user(user)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserForm {
    role: Role,
    username: String,
    password: Option<String>,
}

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
        if user.deleted_at.is_some() {
            Err(Error::bad_request("User already exists and is deleted"))
        } else {
            Err(Error::bad_request("User already exists"))
        }
    } else {
        let password = form.password.unwrap_or_else(generate);
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

        Ok(Success::created(UserResponse::from_user(user)))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordForm {
    current_password: String,
    password: Option<String>,
}

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
        get_user(&storage, user_id).await?
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

pub async fn delete<S: Storage>(
    audit_trail: AuditTrail<S>,
    Extension(storage): Extension<S>,
    current_user: CurrentUser<S>,
    PathParameters(user_id): PathParameters<Uuid>,
) -> Result<Success<&'static str>, Error> {
    current_user.role.is_allowed(Role::Admin)?;

    let user = get_user(&storage, &user_id).await?;

    storage
        .delete_user(&user)
        .await
        .map_err(Error::internal_server_error)?;

    audit_trail.register(AuditEntry::DeleteUser(&user)).await;

    Ok(Success::<&'static str>::no_content())
}

async fn get_user<S: Storage>(storage: &S, user_id: &Uuid) -> Result<User, Error> {
    storage
        .find_single_user_by_id(user_id)
        .await
        .map_err(Error::internal_server_error)?
        .map_or_else(|| Err(Error::not_found("User not found")), Ok)
}
