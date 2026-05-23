use serde_json::json;
use yoagent_state::{
    ActorRef, MemoryEventStore, NodeId, ObjectType, Pack, PackId, RelationType, YoAgentRuntime,
    YoAgentState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let mut runtime = YoAgentRuntime::new(state.clone());
    runtime.register_pack(
        Pack::new(PackId::new("pack_lineage"), "lineage", "0.1.0")
            .add_object_type(ObjectType::new("goal").require("title"))
            .add_object_type(ObjectType::new("task").require("title"))
            .add_relation_type(
                RelationType::new("serves")
                    .from_kind("task")
                    .to_kind("goal"),
            ),
    );

    runtime
        .create_typed_node(
            ActorRef::agent("demo"),
            NodeId::new("goal_1"),
            "goal",
            json!({ "title": "Improve retry reliability" }),
        )
        .await?;
    runtime
        .create_typed_node(
            ActorRef::agent("demo"),
            NodeId::new("task_1"),
            "task",
            json!({ "title": "Investigate timeout" }),
        )
        .await?;
    runtime
        .create_typed_relation(
            ActorRef::agent("demo"),
            NodeId::new("task_1"),
            "serves",
            NodeId::new("goal_1"),
            json!({}),
        )
        .await?;

    println!("{}", serde_json::to_string_pretty(&state.graph().await)?);
    Ok(())
}
