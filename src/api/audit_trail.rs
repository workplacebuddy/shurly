//! Audit trail service

use std::net::IpAddr;

use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::Extension;
use axum::RequestPartsExt;
use axum_client_ip::InsecureClientIp;

use crate::storage::AuditEntry;
use crate::storage::Storage;

use super::CurrentUser;
use super::Error;

/// Audit trail service
pub struct AuditTrail<S: Storage> {
    /// Storage in where the trail is saved
    storage: S,

    /// The current user for the audit trail
    current_user: CurrentUser<S>,

    /// The IP address associated with the audit trail
    ip_address: Option<IpAddr>,
}

impl<S: Storage> AuditTrail<S> {
    /// Register an entry on the audit trail
    pub async fn register(&self, entry: AuditEntry<'_>) {
        let result = self
            .storage
            .register_audit_trail(&self.current_user, &entry, self.ip_address.as_ref())
            .await;

        if let Err(err) = result {
            tracing::error!("Could register audit trail entry: {err}");
        }
    }
}

#[async_trait]
impl<B, S: Storage> FromRequestParts<B> for AuditTrail<S>
where
    B: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &B) -> Result<Self, Self::Rejection> {
        let Extension(storage) = parts
            .extract::<Extension<S>>()
            .await
            .map_err(|_| Error::internal_server_error("Could not get a storage pool"))?;

        let current_user = CurrentUser::from_request_parts(parts, state).await?;

        let ip_address = Option::<InsecureClientIp>::from_request_parts(parts, state)
            .await
            .map_err(|_| Error::internal_server_error("Missing address"))?
            .map(|i| i.0);

        Ok(AuditTrail {
            storage,
            current_user,
            ip_address,
        })
    }
}
