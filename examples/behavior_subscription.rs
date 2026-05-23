use serde_json::json;
use yoagent_state::{
    ActorRef, BehaviorContext, BehaviorId, Event, EventPattern, FnBehavior, MemoryEventStore,
    NodeId, StateOp, YoAgentRuntime, YoAgentState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let mut runtime = YoAgentRuntime::new(state.clone());

    runtime.register_behavior(FnBehavior::new(
        BehaviorId::new("behavior_failure_to_task"),
        EventPattern::Kind("failure.observed".to_string()),
        |_ctx: BehaviorContext, event: Event| async move {
            let title = event
                .payload
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or("Investigate failure");
            Ok(vec![StateOp::CreateNode {
                id: NodeId::new("task_from_failure"),
                kind: "task".to_string(),
                props: json!({
                    "title": format!("Investigate: {title}"),
                    "status": "Open",
                    "created_by_behavior": "behavior_failure_to_task",
                }),
            }])
        },
    ));

    runtime
        .emit_event(Event::new(
            ActorRef::agent("demo"),
            "failure.observed",
            json!({ "title": "retry timeout loses state" }),
        ))
        .await?;

    print!(
        "{}",
        state
            .lineage(NodeId::new("task_from_failure"))
            .await
            .to_markdown()
    );
    Ok(())
}
