use anyhow::Result;
use chrono::naive::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::password::generate;
use crate::password::hash;
use crate::storage::CreateUserValues;
use crate::storage::Storage;
use crate::utils::env_var_or_else;

/// User roles
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Manage users/destinations/notes
    Admin,
    /// Manage destinations/notes
    Manager,
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: Uuid,
    pub session_id: Uuid,
    pub username: String,
    pub hashed_password: String,
    pub role: Role,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}

pub async fn ensure_initial_user<S: Storage>(storage: &S) -> Result<()> {
    let user = storage.find_any_single_user().await?;

    if user.is_none() {
        let username = env_var_or_else("INITIAL_USERNAME", || {
            let initial_username = Uuid::new_v4().to_string();
            tracing::info!(
                "`INITIAL_USERNAME` not set, generating new username: {initial_username}"
            );
            initial_username
        });

        let password = env_var_or_else("INITIAL_PASSWORD", || {
            let initial_password = generate();
            tracing::info!(
                "`INITIAL_PASSWORD` not set, generating new password: {initial_password}"
            );
            initial_password
        });

        let hashed_password = hash(&password);

        let values = CreateUserValues {
            session_id: &Uuid::new_v4(),
            role: Role::Admin,
            username: &username,
            hashed_password: &hashed_password,
        };

        storage.create_user(&values).await?;
    }

    Ok(())
}
