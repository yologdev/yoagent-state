//! A dangling op must not make an append-only log permanently unreadable.
//!
//! Regression for the failure that took down a live GASP store: a shell
//! harness emitted `session-start` on one binary and `session-end` on a newer
//! one, ~70 minutes and several processes apart. The second emitted an
//! `UpdateNode` for a node the first had never created.
//!
//! The caller had guarded the call and logged the error exactly as designed —
//! but the events were appended before the failure surfaced, and catching a
//! `Result` cannot un-append. From that moment every session failed to open
//! the store, so the agent recorded nothing while continuing to run green.
//!
//! Neither repair was legal. Appending a corrective `CreateNode` is never
//! reached, because the fold dies first. Inserting one before the bad op works
//! but rewrites published history, permanently failing the append-only check —
//! the one property the format exists to guarantee.

use serde_json::json;
use yoagent_state::{
    ActorRef, Event, Graph, NodeId, StateOp, replay, replay_strict, replay_with_diagnostics,
};

fn ops_event(ops: Vec<StateOp>) -> Event {
    Event::new(
        ActorRef::agent("test"),
        yoagent_state::STATE_OPS_APPLIED,
        json!(ops),
    )
}

/// The exact production shape: an update for a node nobody created.
fn log_with_a_dangling_update() -> Vec<Event> {
    vec![
        ops_event(vec![StateOp::CreateNode {
            id: NodeId::from("task_real"),
            kind: "task".into(),
            props: json!({"status": "open"}),
        }]),
        // The op that bricked the store.
        ops_event(vec![StateOp::UpdateNode {
            id: NodeId::from("task_run_day180_session"),
            props: json!({"status": "closed"}),
        }]),
        // Everything after it was unreachable.
        ops_event(vec![StateOp::CreateNode {
            id: NodeId::from("task_after"),
            kind: "task".into(),
            props: json!({"status": "open"}),
        }]),
    ]
}

#[test]
fn a_dangling_update_does_not_make_the_log_unreadable() {
    let graph = replay(&log_with_a_dangling_update()).expect("the log must remain readable");

    assert!(
        graph.nodes.contains_key(&NodeId::from("task_real")),
        "events before the dangling op must survive"
    );
    assert!(
        graph.nodes.contains_key(&NodeId::from("task_after")),
        "events AFTER the dangling op must be reached — this is the whole bug: the fold \
         died before them, so an append-only log could never be read again"
    );
}

/// Survivable, but never silent: masking real corruption is the failure mode
/// this must not trade for.
#[test]
fn the_skip_is_reported_with_enough_detail_to_locate_it() {
    let (_graph, skipped) =
        replay_with_diagnostics(&log_with_a_dangling_update()).expect("readable");

    assert_eq!(skipped.len(), 1, "exactly the one bad op, got {skipped:?}");
    let s = &skipped[0];
    assert_eq!(s.op, "UpdateNode");
    assert_eq!(
        s.node.as_ref(),
        Some(&NodeId::from("task_run_day180_session")),
        "the report must name the node, so an operator can find the writer"
    );
    assert!(
        s.reason.contains("node not found"),
        "the reason must be the error the strict path would have given, got {:?}",
        s.reason
    );
}

/// A validator should still reject a malformed log. That distinction is the
/// point: `load` was a reader wearing a validator's failure mode.
#[test]
fn strict_replay_still_rejects_the_same_log() {
    let err = replay_strict(&log_with_a_dangling_update())
        .expect_err("a validator must still reject this");
    assert!(err.to_string().contains("node not found"), "got {err}");
}

/// All four node-referencing ops, not just `UpdateNode`.
///
/// The issue named two; `MarkStale` and `AttachArtifact` abort identically,
/// and `AttachArtifact` is the likeliest in practice because artifacts get
/// attached opportunistically by whichever process happens to have one.
#[test]
fn every_node_referencing_op_survives_a_missing_target() {
    let missing = NodeId::from("never_created");
    let events = vec![ops_event(vec![
        StateOp::UpdateNode {
            id: missing.clone(),
            props: json!({"a": 1}),
        },
        StateOp::TombstoneNode {
            id: missing.clone(),
            reason: "gone".into(),
        },
        StateOp::MarkStale {
            id: missing.clone(),
            reason: "stale".into(),
        },
        StateOp::AttachArtifact {
            id: missing.clone(),
            artifact: yoagent_state::ArtifactRef::new("f", "artifacts/x.txt"),
        },
        // Reached only if none of the four aborted.
        StateOp::CreateNode {
            id: NodeId::from("sentinel"),
            kind: "task".into(),
            props: json!({}),
        },
    ])];

    let (graph, skipped) = replay_with_diagnostics(&events).expect("readable");
    assert!(
        graph.nodes.contains_key(&NodeId::from("sentinel")),
        "the op after four dangling references must still be applied"
    );
    let kinds: Vec<&str> = skipped.iter().map(|s| s.op).collect();
    assert_eq!(
        kinds,
        vec!["UpdateNode", "TombstoneNode", "MarkStale", "AttachArtifact"],
        "all four must be reported, each with its own entry"
    );
    assert_eq!(
        skipped.iter().map(|s| s.index).collect::<Vec<_>>(),
        vec![0, 1, 2, 3],
        "the index locates the op within its batch, so a repair tool need not \
         search for the node id"
    );
}

/// A healthy log must fold exactly as before. The migration risk of leniency
/// is that it quietly changes graphs that were already correct.
#[test]
fn a_healthy_log_folds_identically_under_both_paths() {
    let events = vec![
        ops_event(vec![StateOp::CreateNode {
            id: NodeId::from("a"),
            kind: "task".into(),
            props: json!({"status": "open"}),
        }]),
        ops_event(vec![StateOp::UpdateNode {
            id: NodeId::from("a"),
            props: json!({"status": "done"}),
        }]),
    ];

    let (lenient, skipped): (Graph, _) = replay_with_diagnostics(&events).expect("readable");
    let strict = replay_strict(&events).expect("healthy log folds strictly too");

    assert!(skipped.is_empty(), "nothing to skip in a healthy log");
    // Whole-graph equality, not three hand-picked fields. `Graph` derives
    // `PartialEq`, and the narrow version of this test let a lenient fold
    // tombstone every node, invent a relation and blank every `kind` while
    // staying green — it claimed "folds identically" and checked one node's
    // props.
    assert_eq!(
        lenient, strict,
        "the lenient reader must not change a graph that was already correct"
    );
}

/// A skip must be observable through the **front door**.
///
/// 0.5.1 made the fold survivable and advertised "not silent" — but
/// `YoAgentState::load` and `fork_events_at` both call `replay`, which
/// discards the report. So the one system whose store was bricked survived the
/// brick and was told nothing: green while incomplete, which is the property
/// that made the original incident damaging.
///
/// The API existing is not the same as the API being reachable. This pins the
/// reachable path, not the one nobody calls.
#[tokio::test]
async fn a_skip_is_observable_through_load() {
    use yoagent_state::{EventStore, MemoryEventStore, YoAgentState};

    let store = MemoryEventStore::new();
    store
        .append(log_with_a_dangling_update())
        .await
        .expect("append");

    let (state, skipped) = YoAgentState::load_with_diagnostics(store)
        .await
        .expect("a dangling op must not fail the load");

    assert_eq!(
        skipped.len(),
        1,
        "load must report the skip, not just survive it: {skipped:?}"
    );
    assert_eq!(skipped[0].op, "UpdateNode");

    // And the store is genuinely usable, not merely openable.
    let graph = state.graph().await;
    assert!(
        graph.nodes.contains_key(&NodeId::from("task_after")),
        "events after the dangling op must be present in the loaded graph"
    );
}

/// A reader's `version` must match what a live writer holds.
///
/// This was unpinned and got it backwards. `version` is a `Graph` field, and
/// GASP check 2 compares a snapshot to a fold of its log prefix by whole-graph
/// equality — so if a reader counts an op the writer did not, every snapshot
/// fails to verify, reporting non-conformance for a store every runtime can
/// restore. That is the inversion this whole change exists to prevent.
///
/// Strict `apply_ops` increments only *after* `apply_op` succeeds, so a writer
/// that hits a dangling op does not count it. Measured on the production shape
/// before the fix: live 0, replay 1.
#[tokio::test]
async fn a_reader_and_a_live_writer_agree_on_version() {
    use yoagent_state::{EventStore, MemoryEventStore, YoAgentState};

    let store = MemoryEventStore::new();
    let state = YoAgentState::load(store.clone()).await.unwrap();

    // The real write path: append, then fold strictly. The error is swallowed
    // exactly as the production harness swallowed it.
    let _ = state
        .record_event(ops_event(vec![StateOp::UpdateNode {
            id: NodeId::from("ghost"),
            props: json!({"a": 1}),
        }]))
        .await;

    let live = state.graph().await;
    let events = store.scan().await.unwrap();
    let (replayed, skipped) = replay_with_diagnostics(&events).expect("readable");

    assert_eq!(skipped.len(), 1, "the fixture must actually skip something");
    assert_eq!(
        replayed.version, live.version,
        "a reader that counts the skipped op disagrees with the writer that \
         sealed the snapshot — live={}, replay={}",
        live.version, replayed.version
    );
}
