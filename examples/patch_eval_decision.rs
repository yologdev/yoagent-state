use serde_json::json;
use yoagent_state::{
    ActorRef, ArtifactRef, ExpectedEffect, MemoryEventStore, NodeId, PatchId, PatchStatus,
    Precondition, StatePatch, YoAgentState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let actor = ActorRef::agent("yoyo-evolve");

    let failure = NodeId::new("failure_17");
    state
        .record_failure(
            actor.clone(),
            failure.clone(),
            "tool_retry_survives_timeout fails",
            "Retry state is lost when timeout cancels the future.",
        )
        .await?;

    let patch_id = PatchId::new("patch_42");
    let mut patch = StatePatch::new(
        patch_id.clone(),
        "Persist retry state across timeout",
        "Add RetryState and keep attempt count outside the cancelled future.",
        actor.clone(),
    );
    patch.evidence.push(failure);
    patch.preconditions.push(Precondition::TestStillFailing {
        name: "tool_retry_survives_timeout".to_string(),
    });
    patch.expected_effects.push(ExpectedEffect::TestPasses {
        name: "tool_retry_survives_timeout".to_string(),
    });
    patch.artifacts.push(
        ArtifactRef::new("git.diff", "file://.yoyo/artifacts/patch_42.diff")
            .with_summary("Fix retry persistence and add timeout regression test")
            .with_metadata(json!({
                "base_commit": "abc123",
                "files_changed": [
                    "crates/yoagent-runtime/src/tool.rs",
                    "crates/yoagent-runtime/tests/retry.rs"
                ]
            })),
    );

    state.propose_patch(patch).await?;
    state
        .record_eval_result(
            actor,
            NodeId::new("eval_55"),
            patch_id.clone(),
            "cargo test tool_retry_survives_timeout",
            true,
        )
        .await?;
    state
        .record_decision(
            ActorRef::user("yuanhao"),
            NodeId::new("decision_9"),
            patch_id.clone(),
            true,
            "Eval passed; approve promotion",
        )
        .await?;
    state
        .update_patch_status(
            patch_id.clone(),
            PatchStatus::Approved,
            Some("Eval passed".to_string()),
        )
        .await?;
    state
        .update_patch_status(
            patch_id.clone(),
            PatchStatus::Promoted,
            Some("Promoted as commit def456".to_string()),
        )
        .await?;

    print!(
        "{}",
        state.lineage(NodeId::new(patch_id.0)).await.to_markdown()
    );
    Ok(())
}
