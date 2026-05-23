use serde_json::json;
use yoagent_state::{
    ActorRef, ForkId, MemoryEventStore, NodeId, StateOp, YoAgentState, diff_graphs,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let first = state
        .apply_ops(
            ActorRef::agent("demo"),
            vec![StateOp::CreateNode {
                id: NodeId::new("goal_1"),
                kind: "goal".to_string(),
                props: json!({ "title": "Improve retry reliability" }),
            }],
        )
        .await?;
    state
        .apply_ops(
            ActorRef::agent("demo"),
            vec![StateOp::CreateNode {
                id: NodeId::new("task_1"),
                kind: "task".to_string(),
                props: json!({ "title": "Investigate timeout" }),
            }],
        )
        .await?;

    let fork = state
        .fork_at_event(ForkId::new("fork_before_task"), Some(first))
        .await?;
    let diff = diff_graphs(&fork.graph, &state.graph().await);
    println!("fork_nodes={}", fork.graph.nodes.len());
    println!("current_nodes={}", state.graph().await.nodes.len());
    println!("added_nodes={:?}", diff.added_nodes);
    Ok(())
}
