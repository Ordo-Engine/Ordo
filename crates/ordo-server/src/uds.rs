//! Unix Domain Socket support

use std::path::Path;
use std::sync::Arc;

use ordo_core::rule::RuleExecutor;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tracing::info;

use ordo_proto::ordo_service_server::OrdoServiceServer;

use crate::grpc::OrdoGrpcService;
use crate::rate_limiter::RateLimiter;
use crate::store::RuleStore;
use crate::tenant::TenantManager;

/// Start the gRPC server over Unix Domain Socket
pub async fn start_uds_server(
    uds_path: &Path,
    store: Arc<tokio::sync::RwLock<RuleStore>>,
    executor: Arc<RuleExecutor>,
    default_tenant: String,
    tenant_manager: Arc<TenantManager>,
    rate_limiter: Arc<RateLimiter>,
    multi_tenancy_enabled: bool,
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
    let grpc_service = OrdoGrpcService::new(
        store,
        executor,
        default_tenant,
        tenant_manager,
        rate_limiter,
        multi_tenancy_enabled,
    );

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
    use crate::tenant::{TenantDefaults, TenantManager};
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_uds_server_start_stop() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let store = Arc::new(tokio::sync::RwLock::new(RuleStore::new()));
        let executor = Arc::new(RuleExecutor::new());
        let defaults = TenantDefaults {
            default_qps_limit: Some(1000),
            default_burst_limit: Some(100),
            default_timeout_ms: 100,
        };
        let tenant_manager = Arc::new(TenantManager::new(None, defaults).await.unwrap());
        tenant_manager.ensure_default("default").await.unwrap();
        let rate_limiter = Arc::new(RateLimiter::new());

        // Start server in background
        let socket_path_clone = socket_path.clone();
        let store_clone = store.clone();
        let executor_clone = executor.clone();
        let tenant_manager_clone = tenant_manager.clone();
        let rate_limiter_clone = rate_limiter.clone();
        let server_handle = tokio::spawn(async move {
            start_uds_server(
                &socket_path_clone,
                store_clone,
                executor_clone,
                "default".to_string(),
                tenant_manager_clone,
                rate_limiter_clone,
                false,
            )
            .await
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
