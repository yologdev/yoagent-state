//! Run auto-chaining and the GASP pairing rule, tested store-agnostically.

use serde_json::json;
use yoagent_state::{
    ActorRef, ArtifactRef, Decision, DecisionId, DecisionStatus, EvalId, EvalResult, EvalStatus,
    Event, EventStore, Frame, FrameId, Goal, GoalId, GoalStatus, Hypothesis, HypothesisId,
    MemoryEventStore, ModelCall, NodeId, Observation, ObservationId, PatchId, PatchStatus, RunId,
    StatePatch, Task, TaskId, TaskStatus, ToolCall, YoAgentState,
};

fn actor() -> ActorRef {
    ActorRef::agent("t")
}

async fn fresh() -> YoAgentState<MemoryEventStore> {
    YoAgentState::load(MemoryEventStore::new()).await.unwrap()
}

#[tokio::test]
async fn events_inside_a_run_chain_to_run_started() {
    let state = fresh().await;
    state
        .record_run_started(actor(), RunId::new("run_1"), "task")
        .await
        .unwrap();

    let events = state.store().scan().await.unwrap();
    let run_started_id = events
        .iter()
        .find(|e| e.kind == "run.started")
        .unwrap()
        .id
        .clone();

    // null-causation event inside the run chains to run.started
    state
        .record_goal(Goal::new(GoalId::new("g"), "t", "s", actor()))
        .await
        .unwrap();
    let events = state.store().scan().await.unwrap();
    let goal_created = events.iter().find(|e| e.kind == "goal.created").unwrap();
    assert_eq!(goal_created.causation_id.as_ref(), Some(&run_started_id));

    // an explicit causation_id is never overwritten
    let explicit = Event::new(actor(), "observation.created", json!({"id": "o1"}))
        .with_causation(goal_created.id.clone());
    let goal_created_id = goal_created.id.clone();
    state.record_event(explicit).await.unwrap();
    let events = state.store().scan().await.unwrap();
    let obs = events
        .iter()
        .find(|e| e.kind == "observation.created")
        .unwrap();
    assert_eq!(obs.causation_id.as_ref(), Some(&goal_created_id));

    // run.finished itself chains to run.started, and clears the slot
    state
        .record_run_finished(actor(), RunId::new("run_1"), "done")
        .await
        .unwrap();
    let events = state.store().scan().await.unwrap();
    let finished = events.iter().find(|e| e.kind == "run.finished").unwrap();
    assert_eq!(finished.causation_id.as_ref(), Some(&run_started_id));

    // after the run, unattributed events are roots again
    state
        .record_goal(Goal::new(GoalId::new("g2"), "t", "s", actor()))
        .await
        .unwrap();
    let events = state.store().scan().await.unwrap();
    let after = events
        .iter()
        .filter(|e| e.kind == "goal.created")
        .last()
        .unwrap();
    assert_eq!(after.causation_id, None);
}

#[tokio::test]
async fn run_transitions_are_validated() {
    let state = fresh().await;
    state
        .record_run_started(actor(), RunId::new("run_1"), "task")
        .await
        .unwrap();

    // double start
    let err = state
        .record_run_started(actor(), RunId::new("run_2"), "task")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("already open"), "{err}");

    // finishing a run that is not the open one
    let err = state
        .record_run_finished(actor(), RunId::new("run_2"), "done")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("open run is run_1"), "{err}");

    state
        .record_run_finished(actor(), RunId::new("run_1"), "done")
        .await
        .unwrap();

    // finishing with nothing open
    let err = state
        .record_run_finished(actor(), RunId::new("run_1"), "done")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("no run is open"), "{err}");

    // and a new run can start after a clean finish
    state
        .record_run_started(actor(), RunId::new("run_2"), "task")
        .await
        .unwrap();
}

/// The pairing-rule invariant across EVERY typed helper: each
/// `state.ops_applied` event carries a causation_id naming an earlier
/// non-ops event in the log. A helper regressing to plain `apply_ops`
/// (with no run open to auto-chain it) breaks this.
#[tokio::test]
async fn every_helper_pairs_ops_with_a_domain_event() {
    let state = fresh().await;
    let a = actor();

    state
        .record_run_started(a.clone(), RunId::new("run_1"), "task")
        .await
        .unwrap();
    state
        .record_goal(Goal::new(GoalId::new("g"), "t", "s", a.clone()))
        .await
        .unwrap();
    state
        .update_goal_status(GoalId::new("g"), GoalStatus::InProgress, None)
        .await
        .unwrap();
    state
        .record_task(Task {
            id: TaskId::new("t1"),
            title: "t".into(),
            summary: "s".into(),
            status: TaskStatus::Open,
            goal: Some(GoalId::new("g")),
            created_by: a.clone(),
            metadata: json!({}),
        })
        .await
        .unwrap();
    state
        .update_task_status(TaskId::new("t1"), TaskStatus::Done, None)
        .await
        .unwrap();
    state
        .record_observation(
            a.clone(),
            Observation {
                id: ObservationId::new("o1"),
                title: "t".into(),
                summary: "s".into(),
                observed_in: Some(RunId::new("run_1")),
                metadata: json!({}),
            },
        )
        .await
        .unwrap();
    state
        .record_failure(a.clone(), NodeId::new("f1"), "t", "s")
        .await
        .unwrap();
    state
        .record_hypothesis(
            a.clone(),
            Hypothesis {
                id: HypothesisId::new("h1"),
                title: "t".into(),
                summary: "s".into(),
                confidence: Some(0.5),
                metadata: json!({}),
            },
            Some(NodeId::new("f1")),
        )
        .await
        .unwrap();
    state
        .propose_patch(StatePatch::new(PatchId::new("p1"), "t", "s", a.clone()))
        .await
        .unwrap();
    state
        .record_eval(
            a.clone(),
            EvalResult {
                id: EvalId::new("e1"),
                command: "cmd".into(),
                status: EvalStatus::Passed,
                score: Some(1.0),
                metadata: json!({}),
            },
            Some(PatchId::new("p1")),
        )
        .await
        .unwrap();
    state
        .record_decision_node(
            a.clone(),
            Decision {
                id: DecisionId::new("d1"),
                status: DecisionStatus::Approved,
                reason: "r".into(),
                decided_by: a.clone(),
                metadata: json!({}),
            },
            Some(NodeId::new("p1")),
        )
        .await
        .unwrap();
    state
        .update_patch_status(PatchId::new("p1"), PatchStatus::Promoted, None)
        .await
        .unwrap();
    state
        .record_model_call(
            a.clone(),
            ModelCall {
                id: NodeId::new("m1"),
                run_id: RunId::new("run_1"),
                model: "model".into(),
                prompt_summary: "p".into(),
                output_summary: None,
                metadata: json!({}),
            },
        )
        .await
        .unwrap();
    state
        .record_tool_call(
            a.clone(),
            ToolCall {
                id: NodeId::new("tc1"),
                run_id: RunId::new("run_1"),
                tool: "tool".into(),
                input_summary: "i".into(),
                output_summary: None,
                success: Some(true),
                metadata: json!({}),
            },
        )
        .await
        .unwrap();
    state
        .record_frame(
            a.clone(),
            Frame {
                id: FrameId::new("fr1"),
                title: "t".into(),
                summary: "s".into(),
                parent: None,
                metadata: json!({}),
            },
        )
        .await
        .unwrap();
    state
        .attach_artifact(
            NodeId::new("p1"),
            ArtifactRef::new("git-commit", "abc1234"),
        )
        .await
        .unwrap();
    state
        .record_run_finished(a, RunId::new("run_1"), "done")
        .await
        .unwrap();

    let events = state.store().scan().await.unwrap();
    let mut ops_count = 0;
    for (i, event) in events.iter().enumerate() {
        if event.kind != "state.ops_applied" {
            continue;
        }
        ops_count += 1;
        let cause = event
            .causation_id
            .as_ref()
            .unwrap_or_else(|| panic!("ops event at index {i} has no causation"));
        let domain = events[..i]
            .iter()
            .find(|e| &e.id == cause)
            .unwrap_or_else(|| panic!("ops event at index {i} chained to a missing event"));
        assert_ne!(
            domain.kind, "state.ops_applied",
            "ops event at index {i} chained to another ops event"
        );
    }
    assert!(ops_count >= 15, "expected ops from every helper, saw {ops_count}");
}
