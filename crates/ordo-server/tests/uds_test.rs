#![cfg(unix)]
//! Unix Domain Socket integration tests

use std::time::Duration;

use ordo_proto::ordo_service_client::OrdoServiceClient;
use ordo_proto::ordo_service_server::OrdoServiceServer;
use ordo_proto::{
    EvalRequest, ExecuteRequest, GetRuleSetRequest, HealthRequest, ListRuleSetsRequest,
};
use tempfile::tempdir;
use tokio::net::UnixStream;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Endpoint, Server, Uri};
use tower::service_fn;

mod common;
use common::{create_grpc_service, create_test_store};

/// Create a UDS client connected to the given socket path
async fn create_uds_client(
    socket_path: &std::path::Path,
) -> OrdoServiceClient<tonic::transport::Channel> {
    let socket_path = socket_path.to_owned();

    // Create a channel using UDS
    let channel = Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            let path = socket_path.clone();
            async move { UnixStream::connect(path).await }
        }))
        .await
        .unwrap();

    OrdoServiceClient::new(channel)
}

/// Start a UDS server and return socket path
async fn setup_uds_server() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("ordo.sock");

    let store = create_test_store().await;
    let grpc_service = create_grpc_service(store);

    let uds = tokio::net::UnixListener::bind(&socket_path).unwrap();
    let uds_stream = UnixListenerStream::new(uds);

    // Start server in background
    tokio::spawn(async move {
        Server::builder()
            .add_service(OrdoServiceServer::new(grpc_service))
            .serve_with_incoming(uds_stream)
            .await
            .unwrap();
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    (temp_dir, socket_path)
}

#[tokio::test]
async fn test_uds_health_check() {
    let (_temp_dir, socket_path) = setup_uds_server().await;
    let mut client = create_uds_client(&socket_path).await;

    let response = client
        .health(HealthRequest {
            service: String::new(),
        })
        .await
        .unwrap();

    let health = response.into_inner();
    assert_eq!(health.status, 1); // SERVING
    assert_eq!(health.ruleset_count, 1);
}

#[tokio::test]
async fn test_uds_execute_ruleset() {
    let (_temp_dir, socket_path) = setup_uds_server().await;
    let mut client = create_uds_client(&socket_path).await;

    // Test with value > 50 (should go to HIGH)
    let response = client
        .execute(ExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            input_json: r#"{"value": 75}"#.to_string(),
            include_trace: true,
        })
        .await
        .unwrap();

    let result = response.into_inner();
    assert_eq!(result.code, "HIGH");
    assert!(result.trace.is_some());
}

#[tokio::test]
async fn test_uds_execute_with_low_value() {
    let (_temp_dir, socket_path) = setup_uds_server().await;
    let mut client = create_uds_client(&socket_path).await;

    // Test with value <= 50 (should go to LOW)
    let response = client
        .execute(ExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            input_json: r#"{"value": 25}"#.to_string(),
            include_trace: false,
        })
        .await
        .unwrap();

    let result = response.into_inner();
    assert_eq!(result.code, "LOW");
}

#[tokio::test]
async fn test_uds_execute_not_found() {
    let (_temp_dir, socket_path) = setup_uds_server().await;
    let mut client = create_uds_client(&socket_path).await;

    let result = client
        .execute(ExecuteRequest {
            ruleset_name: "nonexistent".to_string(),
            input_json: r#"{}"#.to_string(),
            include_trace: false,
        })
        .await;

    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_uds_list_rulesets() {
    let (_temp_dir, socket_path) = setup_uds_server().await;
    let mut client = create_uds_client(&socket_path).await;

    let response = client
        .list_rule_sets(ListRuleSetsRequest {
            name_prefix: String::new(),
            limit: 0,
        })
        .await
        .unwrap();

    let list = response.into_inner();
    assert_eq!(list.total_count, 1);
    assert_eq!(list.rulesets[0].name, "test_rule");
}

#[tokio::test]
async fn test_uds_get_ruleset() {
    let (_temp_dir, socket_path) = setup_uds_server().await;
    let mut client = create_uds_client(&socket_path).await;

    let response = client
        .get_rule_set(GetRuleSetRequest {
            name: "test_rule".to_string(),
        })
        .await
        .unwrap();

    let ruleset = response.into_inner();
    assert!(!ruleset.ruleset_json.is_empty());
    assert_eq!(ruleset.step_count, 3);
}

#[tokio::test]
async fn test_uds_eval_expression() {
    let (_temp_dir, socket_path) = setup_uds_server().await;
    let mut client = create_uds_client(&socket_path).await;

    let response = client
        .eval(EvalRequest {
            expression: "age >= 18 && status == \"active\"".to_string(),
            context_json: r#"{"age": 25, "status": "active"}"#.to_string(),
        })
        .await
        .unwrap();

    let result = response.into_inner();
    assert_eq!(result.result_json, "true");
}

#[tokio::test]
async fn test_uds_concurrent_requests() {
    let (_temp_dir, socket_path) = setup_uds_server().await;

    // Execute multiple requests concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let socket_path_clone = socket_path.clone();
        handles.push(tokio::spawn(async move {
            let mut client = create_uds_client(&socket_path_clone).await;
            client
                .execute(ExecuteRequest {
                    ruleset_name: "test_rule".to_string(),
                    input_json: format!(r#"{{"value": {}}}"#, i * 10),
                    include_trace: false,
                })
                .await
        }));
    }

    // Wait for all requests to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_uds_socket_cleanup() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("test_cleanup.sock");

    // Create socket
    let _uds = tokio::net::UnixListener::bind(&socket_path).unwrap();
    assert!(socket_path.exists());

    // Cleanup
    std::fs::remove_file(&socket_path).unwrap();
    assert!(!socket_path.exists());
}
