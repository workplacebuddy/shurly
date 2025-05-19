//! Audit trail service

use std::net::IpAddr;

use axum::Extension;
use axum::RequestPartsExt;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::client_ip::ClientIp;
use crate::database::AuditEntry;
use crate::database::Database;

use super::CurrentUser;
use super::Error;

/// Audit trail service
pub struct AuditTrail {
    /// Database in where the trail is saved
    database: Database,

    /// The current user for the audit trail
    current_user: CurrentUser,

    /// The IP address associated with the audit trail
    ip_address: Option<IpAddr>,
}

impl AuditTrail {
    /// Register an entry on the audit trail
    pub async fn register(&self, entry: AuditEntry<'_>) {
        let result = self
            .database
            .register_audit_trail(&self.current_user, &entry, self.ip_address.as_ref())
            .await;

        if let Err(err) = result {
            tracing::error!("Could register audit trail entry: {err}");
        }
    }
}

impl<B> FromRequestParts<B> for AuditTrail
where
    B: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &B) -> Result<Self, Self::Rejection> {
        let Extension(database) = parts
            .extract::<Extension<Database>>()
            .await
            .map_err(|_| Error::internal_server_error("Could not get a database pool"))?;

        let current_user = CurrentUser::from_request_parts(parts, state).await?;

        let ip_address = Option::<ClientIp>::from_request_parts(parts, state)
            .await
            .map_err(|_| Error::internal_server_error("Missing address"))?
            .map(|client_ip| client_ip.ip_address.0);

        Ok(AuditTrail {
            database,
            current_user,
            ip_address,
        })
    }
}
