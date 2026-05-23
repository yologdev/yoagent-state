use serde_json::json;
use yoagent_state::{ActorRef, MemoryEventStore, NodeId, StateOp, YoAgentState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let actor = ActorRef::agent("example");

    let failure = NodeId::new("failure_retry_timeout");
    let hypothesis = NodeId::new("hypothesis_retry_state_lost");

    state
        .apply_ops(
            actor,
            vec![
                StateOp::CreateNode {
                    id: failure.clone(),
                    kind: "failure".to_string(),
                    props: json!({
                        "title": "Retry state is lost after timeout",
                        "summary": "The next retry starts from attempt zero."
                    }),
                },
                StateOp::CreateNode {
                    id: hypothesis.clone(),
                    kind: "hypothesis".to_string(),
                    props: json!({
                        "title": "Attempt count is scoped to the cancelled future"
                    }),
                },
                StateOp::CreateRelation {
                    from: hypothesis.clone(),
                    rel: "explains".to_string(),
                    to: failure,
                    props: json!({}),
                },
            ],
        )
        .await?;

    print!("{}", state.lineage(hypothesis).await.to_markdown());
    Ok(())
}
