//! Unix Domain Socket support

use std::path::Path;
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tracing::info;

use ordo_proto::ordo_service_server::OrdoServiceServer;

use crate::grpc::OrdoGrpcService;
use crate::store::RuleStore;

/// Start the gRPC server over Unix Domain Socket
pub async fn start_uds_server(
    uds_path: &Path,
    store: Arc<tokio::sync::RwLock<RuleStore>>,
    default_tenant: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Remove existing socket file if it exists
    if uds_path.exists() {
        std::fs::remove_file(uds_path)?;
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = uds_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create Unix listener
    let uds = UnixListener::bind(uds_path)?;
    let uds_stream = UnixListenerStream::new(uds);

    info!("UDS server listening on {:?}", uds_path);

    // Create gRPC service
    let grpc_service = OrdoGrpcService::new(store, default_tenant);

    // Start server
    Server::builder()
        .add_service(OrdoServiceServer::new(grpc_service))
        .serve_with_incoming(uds_stream)
        .await?;

    Ok(())
}

/// Cleanup UDS socket file
#[allow(dead_code)]
pub fn cleanup_uds(uds_path: &Path) {
    if uds_path.exists() {
        if let Err(e) = std::fs::remove_file(uds_path) {
            tracing::warn!("Failed to remove UDS socket file: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_uds_server_start_stop() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let store = Arc::new(tokio::sync::RwLock::new(RuleStore::new()));

        // Start server in background
        let socket_path_clone = socket_path.clone();
        let store_clone = store.clone();
        let server_handle = tokio::spawn(async move {
            start_uds_server(&socket_path_clone, store_clone, "default".to_string()).await
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify socket file exists
        assert!(socket_path.exists());

        // Abort server
        server_handle.abort();

        // Cleanup
        cleanup_uds(&socket_path);
        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn test_cleanup_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("nonexistent.sock");

        // Should not panic
        cleanup_uds(&socket_path);
    }
}
