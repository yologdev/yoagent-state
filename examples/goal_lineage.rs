use serde_json::json;
use yoagent_state::{
    ActorRef, ArtifactRef, ExpectedEffect, Goal, GoalId, GoalStatus, MemoryEventStore, NodeId,
    PatchId, PatchStatus, StatePatch, Task, TaskId, TaskStatus, YoAgentState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let actor = ActorRef::agent("yoyo-evolve");
    let state = YoAgentState::load(MemoryEventStore::new()).await?;

    let goal = Goal {
        status: GoalStatus::InProgress,
        ..Goal::new(
            GoalId::new("goal_retry_reliability"),
            "Make retry behavior reliable",
            "Retry attempts should survive timeout cancellation.",
            actor.clone(),
        )
    };
    state.record_goal(goal).await?;

    let task = Task {
        id: TaskId::new("task_retry_timeout"),
        title: "Fix timeout retry state".to_string(),
        summary: "Investigate and patch retry state loss.".to_string(),
        status: TaskStatus::InProgress,
        goal: Some(GoalId::new("goal_retry_reliability")),
        created_by: actor.clone(),
        metadata: json!({}),
    };
    state.record_task(task).await?;

    let failure = NodeId::new("failure_retry_timeout");
    state
        .record_failure(
            actor.clone(),
            failure.clone(),
            "retry timeout loses state",
            "The next attempt starts from zero after cancellation.",
        )
        .await?;
    state
        .link(
            actor.clone(),
            failure.clone(),
            yoagent_state::REL_BLOCKS,
            NodeId::new("goal_retry_reliability"),
        )
        .await?;

    let patch_id = PatchId::new("patch_retry_state");
    let mut patch = StatePatch::new(
        patch_id.clone(),
        "Persist retry state",
        "Keep attempt count outside the cancelled future.",
        actor.clone(),
    );
    patch.evidence.push(failure);
    patch.expected_effects.push(ExpectedEffect::TestPasses {
        name: "tool_retry_survives_timeout".to_string(),
    });
    patch.artifacts.push(ArtifactRef::new(
        "git.diff",
        "file://.yoyo/artifacts/patch_retry_state.diff",
    ));
    state.propose_patch(patch).await?;
    state
        .link(
            actor.clone(),
            NodeId::new(patch_id.0.clone()),
            yoagent_state::REL_ADVANCES,
            NodeId::new("goal_retry_reliability"),
        )
        .await?;
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
        .update_patch_status(
            patch_id,
            PatchStatus::Promoted,
            Some("eval passed".to_string()),
        )
        .await?;

    print!(
        "{}",
        state
            .lineage(NodeId::new("goal_retry_reliability"))
            .await
            .to_markdown()
    );
    Ok(())
}
