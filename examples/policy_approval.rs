use serde_json::json;
use yoagent_state::{
    ActorRef, MemoryEventStore, NodeId, Policy, PolicyAction, PolicyId, StateError, YoAgentRuntime,
    YoAgentState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let mut runtime = YoAgentRuntime::new(state.clone());
    runtime.register_policy(Policy::require_approval(
        PolicyId::new("policy_create_node_review"),
        "Creating graph nodes requires review",
        PolicyAction::CreateNode,
    ));

    let result = runtime
        .create_typed_node(
            ActorRef::agent("demo"),
            NodeId::new("sensitive_node"),
            "memory",
            json!({ "title": "Potentially sensitive memory" }),
        )
        .await;

    match result {
        Err(StateError::PolicyDenied(message)) => println!("blocked: {message}"),
        other => println!("unexpected: {other:?}"),
    }

    let approvals = state
        .graph()
        .await
        .nodes
        .values()
        .filter(|node| node.kind == "approval_request")
        .count();
    println!("approval_requests={approvals}");
    Ok(())
}
