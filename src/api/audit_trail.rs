//! Audit trail service

use std::net::IpAddr;

use axum::async_trait;
use axum::extract::FromRequest;
use axum::extract::RequestParts;
use axum::Extension;
use axum_client_ip::ClientIp;

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
impl<B, S: Storage> FromRequest<B> for AuditTrail<S>
where
    B: Send,
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(storage) = req
            .extract::<Extension<S>>()
            .await
            .map_err(|_| Error::internal_server_error("Could not get a storage pool"))?;

        let current_user = CurrentUser::from_request(req).await?;

        let ip_address = Option::<ClientIp>::from_request(req)
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
