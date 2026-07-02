//! Emit a complete GASP agent repo — goal → run → patch → eval → decision →
//! promotion — through `GitEventStore`, then validate it with the GASP
//! conformance checker:
//!
//! ```sh
//! cargo run --example gasp_agent_repo -- /tmp/gasp-demo-repo
//! conformance-check /tmp/gasp-demo-repo   # from the gasp workspace
//! ```

use std::process::Command;
use yoagent_state::{
    init_agent_repo, ActorRef, Decision, DecisionId, DecisionStatus, EvalId, EvalResult,
    EvalStatus, Goal, GoalId, NodeId, PatchId, PatchStatus, RunId, StatePatch, ToolCall,
    YoAgentState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .expect("usage: gasp_agent_repo <target-dir>");

    let store = init_agent_repo(&root, "demo-agent", "worker-demo")?;
    let git = |args: &[&str]| {
        assert!(Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .status()
            .unwrap()
            .success());
    };
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init demo agent repo"]);

    let state = YoAgentState::load(store.clone()).await?;
    let actor = ActorRef::agent("demo");

    state
        .record_goal(Goal::new(
            GoalId::new("goal_retry"),
            "Make retry reliable",
            "retries drop state after timeout",
            actor.clone(),
        ))
        .await?;

    state
        .record_run_started(actor.clone(), RunId::new("run_1"), "fix retry skill")
        .await?;

    let patch_id = state
        .propose_patch(StatePatch::new(
            PatchId::new("patch_1"),
            "persist retry counter",
            "counter survives timeouts",
            actor.clone(),
        ))
        .await?;
    state
        .link(
            actor.clone(),
            NodeId::new("patch_1"),
            "advances",
            NodeId::new("goal_retry"),
        )
        .await?;

    state
        .record_tool_call(
            ActorRef::tool("cargo"),
            ToolCall {
                id: NodeId::new("tool_1"),
                run_id: RunId::new("run_1"),
                tool: "cargo test".into(),
                input_summary: "cargo test retry".into(),
                output_summary: Some("ok, 1 passed".into()),
                success: Some(true),
                metadata: serde_json::json!({}),
            },
        )
        .await?;

    state
        .record_eval(
            actor.clone(),
            EvalResult {
                id: EvalId::new("eval_1"),
                command: "cargo test retry".into(),
                status: EvalStatus::Passed,
                score: Some(1.0),
                metadata: serde_json::json!({}),
            },
            Some(patch_id.clone()),
        )
        .await?;

    state
        .record_decision_node(
            actor.clone(),
            Decision {
                id: DecisionId::new("decision_1"),
                status: DecisionStatus::Approved,
                reason: "eval passed; promote".into(),
                decided_by: actor.clone(),
                metadata: serde_json::json!({}),
            },
            Some(NodeId::new("patch_1")),
        )
        .await?;
    state
        .update_patch_status(patch_id, PatchStatus::Promoted, Some("promoted".into()))
        .await?;

    state
        .record_run_finished(actor, RunId::new("run_1"), "promoted")
        .await?;

    let sha = store.commit_run("run_1", "goal_retry", "promoted", &[])?;
    println!("agent repo at {root}, boundary commit {sha:?}");
    println!(
        "{}",
        state.lineage(NodeId::new("goal_retry")).await.to_markdown()
    );
    Ok(())
}
