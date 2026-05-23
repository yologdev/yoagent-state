use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use yoagent_state::{
    ActorRef, ArtifactRef, Event, EventStore, ExpectedEffect, JsonlEventStore, MemoryEventStore,
    NodeId, PatchId, PatchStatus, Precondition, StateOp, StatePatch, YoAgentState,
    changed_file_ops, parse_git_name_status, replay,
};

#[tokio::test]
async fn appends_events_and_scans_after() {
    let store = MemoryEventStore::new();
    let first = Event::new(ActorRef::agent("test"), "run.started", json!({}));
    let second = Event::new(ActorRef::agent("test"), "run.finished", json!({}));
    let first_id = first.id.clone();

    store.append(vec![first, second.clone()]).await.unwrap();

    let all = store.scan().await.unwrap();
    assert_eq!(all.len(), 2);
    let after = store.scan_after(Some(first_id)).await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, second.id);
}

#[tokio::test]
async fn applies_state_ops_to_graph() {
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();
    let node = NodeId::new("failure_1");

    state
        .apply_ops(
            ActorRef::agent("test"),
            vec![
                StateOp::CreateNode {
                    id: node.clone(),
                    kind: "failure".to_string(),
                    props: json!({ "title": "timeout" }),
                },
                StateOp::UpdateNode {
                    id: node.clone(),
                    props: json!({ "severity": "high" }),
                },
            ],
        )
        .await
        .unwrap();

    let graph = state.graph().await;
    let stored = graph.get_node(&node).unwrap();
    assert_eq!(stored.kind, "failure");
    assert_eq!(stored.props["title"], "timeout");
    assert_eq!(stored.props["severity"], "high");
    assert_eq!(graph.version, 2);
}

#[tokio::test]
async fn replays_event_log_into_graph() {
    let store = MemoryEventStore::new();
    let node = NodeId::new("task_1");
    let ops = vec![StateOp::CreateNode {
        id: node.clone(),
        kind: "task".to_string(),
        props: json!({ "title": "write docs" }),
    }];

    store
        .append(vec![Event::new(
            ActorRef::agent("test"),
            yoagent_state::STATE_OPS_APPLIED,
            serde_json::to_value(ops).unwrap(),
        )])
        .await
        .unwrap();

    let replayed = replay(&store.scan().await.unwrap()).unwrap();
    assert!(replayed.get_node(&node).is_some());

    let loaded = YoAgentState::load(store).await.unwrap();
    assert!(loaded.graph().await.get_node(&node).is_some());
}

#[tokio::test]
async fn patch_status_transitions_update_patch_node() {
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();
    let patch_id = PatchId::new("patch_1");
    let mut patch = StatePatch::new(
        patch_id.clone(),
        "Fix timeout retry",
        "Persist retry state",
        ActorRef::agent("test"),
    );
    patch.preconditions.push(Precondition::StateVersionIs(0));
    patch.expected_effects.push(ExpectedEffect::TestPasses {
        name: "retry_timeout".to_string(),
    });

    state.propose_patch(patch).await.unwrap();
    state
        .update_patch_status(patch_id.clone(), PatchStatus::Evaluated, None)
        .await
        .unwrap();
    state
        .update_patch_status(
            patch_id.clone(),
            PatchStatus::Approved,
            Some("test passed".to_string()),
        )
        .await
        .unwrap();

    let node = state
        .get_node(NodeId::new(patch_id.0))
        .await
        .expect("patch node");
    assert_eq!(node.props["status"], "Approved");
    assert_eq!(node.props["status_reason"], "test passed");
}

#[tokio::test]
async fn lineage_query_explains_failure_patch_eval_chain() {
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();
    let actor = ActorRef::agent("test");
    let failure = NodeId::new("failure_1");
    let patch_id = PatchId::new("patch_1");

    state
        .record_failure(actor.clone(), failure.clone(), "retry fails", "timeout")
        .await
        .unwrap();

    let mut patch = StatePatch::new(
        patch_id.clone(),
        "Fix retry",
        "Persist attempt count",
        actor.clone(),
    );
    patch.evidence.push(failure.clone());
    patch
        .artifacts
        .push(ArtifactRef::new("git.diff", "file://patch.diff"));
    state.propose_patch(patch).await.unwrap();
    state
        .record_eval_result(
            actor,
            NodeId::new("eval_1"),
            patch_id.clone(),
            "cargo test",
            true,
        )
        .await
        .unwrap();
    state
        .record_decision(
            ActorRef::user("reviewer"),
            NodeId::new("decision_1"),
            patch_id.clone(),
            true,
            "eval passed",
        )
        .await
        .unwrap();

    let lineage = state.lineage(NodeId::new(patch_id.0)).await;
    assert!(lineage.outgoing.iter().any(|rel| rel.rel == "addresses"));
    assert!(lineage.outgoing.iter().any(|rel| rel.rel == "validated_by"));
    assert!(lineage.outgoing.iter().any(|rel| rel.rel == "approved_by"));
    assert_eq!(lineage.root.unwrap().artifacts.len(), 1);
}

#[tokio::test]
async fn jsonl_store_survives_reload() {
    let path = std::env::temp_dir().join(format!(
        "yoagent-state-{}.jsonl",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let node = NodeId::new("persisted");

    let state = YoAgentState::load(JsonlEventStore::new(&path))
        .await
        .unwrap();
    state
        .apply_ops(
            ActorRef::agent("test"),
            vec![StateOp::CreateNode {
                id: node.clone(),
                kind: "observation".to_string(),
                props: json!({ "title": "survives reload" }),
            }],
        )
        .await
        .unwrap();

    let reloaded = YoAgentState::load(JsonlEventStore::new(&path))
        .await
        .unwrap();
    assert!(reloaded.graph().await.get_node(&node).is_some());

    let _ = tokio::fs::remove_file(path).await;
}

#[test]
fn observer_turns_changed_files_into_patch_relations() {
    let files = parse_git_name_status("M src/lib.rs\nA tests/state_flow.rs\n");
    let ops = changed_file_ops(PatchId::new("patch_1"), &files);

    assert_eq!(files.len(), 2);
    assert_eq!(ops.len(), 4);
}
