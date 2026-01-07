//! gRPC integration tests

use ordo_proto::ordo_service_client::OrdoServiceClient;
use ordo_proto::ordo_service_server::OrdoServiceServer;
use ordo_proto::{
    EvalRequest, ExecuteRequest, GetRuleSetRequest, HealthRequest, ListRuleSetsRequest,
};
use tonic::transport::{Channel, Server};

mod common;
use common::{create_test_store, wait_for_server};

/// Start a test gRPC server and return a client
async fn setup_grpc_server() -> OrdoServiceClient<Channel> {
    let store = create_test_store().await;

    // Find available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    // Create gRPC service
    let grpc_service = common::create_grpc_service(store);

    // Start server in background
    let addr_clone = addr;
    tokio::spawn(async move {
        Server::builder()
            .add_service(OrdoServiceServer::new(grpc_service))
            .serve(addr_clone)
            .await
            .unwrap();
    });

    // Wait for server to start
    wait_for_server(&format!("http://{}", addr)).await;

    // Create client
    OrdoServiceClient::connect(format!("http://{}", addr))
        .await
        .unwrap()
}

#[tokio::test]
async fn test_grpc_health_check() {
    let mut client = setup_grpc_server().await;

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
async fn test_grpc_execute_ruleset() {
    let mut client = setup_grpc_server().await;

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

    let trace = result.trace.unwrap();
    assert!(!trace.path.is_empty());
    assert!(!trace.steps.is_empty());
}

#[tokio::test]
async fn test_grpc_execute_with_low_value() {
    let mut client = setup_grpc_server().await;

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
    assert!(result.trace.is_none());
}

#[tokio::test]
async fn test_grpc_execute_not_found() {
    let mut client = setup_grpc_server().await;

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
async fn test_grpc_execute_invalid_json() {
    let mut client = setup_grpc_server().await;

    let result = client
        .execute(ExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            input_json: "not valid json".to_string(),
            include_trace: false,
        })
        .await;

    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_grpc_list_rulesets() {
    let mut client = setup_grpc_server().await;

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
async fn test_grpc_list_rulesets_with_prefix() {
    let mut client = setup_grpc_server().await;

    // With matching prefix
    let response = client
        .list_rule_sets(ListRuleSetsRequest {
            name_prefix: "test".to_string(),
            limit: 0,
        })
        .await
        .unwrap();

    assert_eq!(response.into_inner().total_count, 1);

    // With non-matching prefix
    let response = client
        .list_rule_sets(ListRuleSetsRequest {
            name_prefix: "nonexistent".to_string(),
            limit: 0,
        })
        .await
        .unwrap();

    assert_eq!(response.into_inner().total_count, 0);
}

#[tokio::test]
async fn test_grpc_get_ruleset() {
    let mut client = setup_grpc_server().await;

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
async fn test_grpc_get_ruleset_not_found() {
    let mut client = setup_grpc_server().await;

    let result = client
        .get_rule_set(GetRuleSetRequest {
            name: "nonexistent".to_string(),
        })
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_grpc_eval_expression() {
    let mut client = setup_grpc_server().await;

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
async fn test_grpc_eval_arithmetic() {
    let mut client = setup_grpc_server().await;

    let response = client
        .eval(EvalRequest {
            expression: "price * quantity".to_string(),
            context_json: r#"{"price": 10.5, "quantity": 3}"#.to_string(),
        })
        .await
        .unwrap();

    let result = response.into_inner();
    assert_eq!(result.result_json, "31.5");
}

#[tokio::test]
async fn test_grpc_eval_invalid_expression() {
    let mut client = setup_grpc_server().await;

    let result = client
        .eval(EvalRequest {
            expression: "(((".to_string(),
            context_json: r#"{}"#.to_string(),
        })
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_grpc_concurrent_requests() {
    let mut client = setup_grpc_server().await;

    // Execute multiple requests concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let mut client_clone = client.clone();
        handles.push(tokio::spawn(async move {
            client_clone
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
