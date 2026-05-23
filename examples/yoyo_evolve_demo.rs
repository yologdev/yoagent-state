use serde_json::json;
use yoagent_state::{
    ActorRef, ExpectedEffect, NodeId, PatchId, PatchStatus, Precondition, StatePatch, YoAgentState,
    changed_file_ops, diff_artifact, parse_git_name_status, project_ref,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(yoagent_state::MemoryEventStore::new()).await?;
    let actor = ActorRef::agent("yoyo-evolve");

    let failure = NodeId::new("failure_retry_timeout");
    state
        .record_failure(
            actor.clone(),
            failure.clone(),
            "Retry eval fails after timeout",
            "The retry attempt count is reset when the timeout cancels the future.",
        )
        .await?;

    let changed = parse_git_name_status(
        "M crates/yoagent-runtime/src/tool.rs\nA crates/yoagent-runtime/tests/retry.rs\n",
    );
    let patch_id = PatchId::new("patch_retry_timeout");
    let mut patch = StatePatch::new(
        patch_id.clone(),
        "Persist retry state across timeout",
        "Move attempt count into RetryState and keep it outside the cancelled future.",
        actor.clone(),
    );
    patch.base_project_ref = Some(project_ref(
        "yoagent",
        Some("main".to_string()),
        Some("abc123".to_string()),
        None,
    ));
    patch.evidence.push(failure);
    patch
        .preconditions
        .push(Precondition::ProjectCommitIs("abc123".to_string()));
    patch.expected_effects.push(ExpectedEffect::TestPasses {
        name: "tool_retry_survives_timeout".to_string(),
    });
    patch.artifacts.push(diff_artifact(
        ".yoyo/artifacts/patch_retry_timeout.diff",
        "Persist retry state and add timeout regression test",
        "abc123",
        &changed,
    ));
    patch
        .ops
        .extend(changed_file_ops(patch_id.clone(), &changed));

    state.propose_patch(patch).await?;
    state
        .record_eval_result(
            actor,
            NodeId::new("eval_retry_timeout"),
            patch_id.clone(),
            "cargo test tool_retry_survives_timeout",
            true,
        )
        .await?;
    state
        .record_decision(
            ActorRef::user("yuanhao"),
            NodeId::new("decision_promote_retry_timeout"),
            patch_id.clone(),
            true,
            "Regression test passed",
        )
        .await?;
    state
        .update_patch_status(
            patch_id.clone(),
            PatchStatus::Promoted,
            Some("Promoted as commit def456".to_string()),
        )
        .await?;

    let lineage = state.lineage(NodeId::new(patch_id.0)).await;
    println!("{}", lineage.to_markdown());
    println!(
        "changed_files={}",
        json!(changed.iter().map(|file| &file.path).collect::<Vec<_>>())
    );
    Ok(())
}
