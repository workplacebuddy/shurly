#![forbid(unsafe_code)]
#![forbid(clippy::missing_docs_in_private_items)]
#![warn(clippy::pedantic)]
// easier to use when using the functions as callback of foreign functions
#![allow(clippy::needless_pass_by_value)]
// types are used on their and are easier to read with a complete name
#![allow(clippy::module_name_repetitions)]
#![doc = include_str!("../README.md")]

use std::net::SocketAddr;

use anyhow::Result;
use axum::Extension;
use axum::Router;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::prelude::*;

use crate::api::router;
use crate::api::JwtKeys;
use crate::database::Database;
use crate::database::DatabaseConfig;
use crate::users::ensure_initial_user;
use crate::utils::env_var_or_else;

mod api;
mod database;
mod destinations;
mod graceful_shutdown;
mod notes;
mod password;
mod root;
#[cfg(test)]
mod tests;
mod users;
mod utils;

/// Default `RUST_LOG` value
const DEFAULT_RUST_LOG: &str = "shurly=debug,tower_http=debug";

/// Default address Shurly binds to
const DEFAULT_ADDRESS: &str = "0.0.0.0:7000";

#[tokio::main]
async fn main() -> Result<()> {
    setup_environment();
    setup_tracing();

    let app = setup_app(DatabaseConfig::DetectConfig).await?;

    let address = setup_address()?;
    tracing::info!("Listening on {}", address);

    let listener = TcpListener::bind(address).await?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(graceful_shutdown::handler())
    .await?;

    Ok(())
}

/// Create and setup the app with its dependencies
///
/// # Errors
///
/// Will return `Err` if any of its dependencies fail to load:
/// - Database connection
/// - Initial user setup
pub async fn setup_app(config: DatabaseConfig) -> Result<Router> {
    let database = Database::from_config(config).await;

    ensure_initial_user(&database).await?;

    Ok(create_router(database))
}

/// Create the router for Shurly
fn create_router(database: Database) -> Router {
    let jwt_keys = setup_jwt_keys();

    Router::new()
        .nest("/api", router())
        .fallback(root::root)
        .layer(TraceLayer::new_for_http())
        .layer(Extension(database))
        .layer(Extension(jwt_keys))
}

/// Setup the environment (variables) in which Shurly runs
fn setup_environment() {
    dotenvy::dotenv().ok();
}

/// Setup the tracing subscriber for logging
fn setup_tracing() {
    use tracing_subscriber::fmt;
    use tracing_subscriber::registry;
    use tracing_subscriber::EnvFilter;

    registry()
        .with(EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_RUST_LOG.into()),
        ))
        .with(fmt::layer())
        .init();
}

/// Setup the JWT keys for encoding/decoding
fn setup_jwt_keys() -> JwtKeys {
    use crate::password::generate;

    let jwt_secret = env_var_or_else("JWT_SECRET", || {
        let jwt_secret = generate();
        tracing::info!("`JWT_SECRET` is not set, generating temporary one: {jwt_secret}");
        jwt_secret
    });

    JwtKeys::new(jwt_secret.as_bytes())
}

/// Setup the address Shurly will bind to
fn setup_address() -> Result<SocketAddr> {
    let mut address =
        env_var_or_else("ADDRESS", || String::from(DEFAULT_ADDRESS)).parse::<SocketAddr>()?;

    // optional override of just the port
    if let Ok(port) = std::env::var("PORT") {
        // only check non-empty strings
        if !port.is_empty() {
            let port = port.parse::<u16>()?;

            address.set_port(port);
        }
    }

    Ok(address)
}
