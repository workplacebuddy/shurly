//! Graceful shutdown

use tokio::signal;

/// Handler for graceful shutdown
///
/// Will listen to Ctrl+C to and initiate a shutdown
pub async fn handler() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Valid CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Valid terminate handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Terminate signal received, starting graceful shutdown");
}
