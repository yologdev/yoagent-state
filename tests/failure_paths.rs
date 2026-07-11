//! Error-path coverage for run transitions: what happens when the SECOND
//! append of a paired helper fails, and how the open-run marker behaves.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use serde_json::json;
use yoagent_state::{
    ActorRef, Event, EventId, EventStore, MemoryEventStore, RunId, StateError, YoAgentState,
};

/// Wraps MemoryEventStore and fails every append once `fail_after` appends
/// have succeeded (then keeps failing until `heal` is called).
#[derive(Debug, Clone)]
struct FailingStore {
    inner: MemoryEventStore,
    appends: Arc<AtomicUsize>,
    fail_after: Arc<AtomicUsize>,
}

impl FailingStore {
    fn new(fail_after: usize) -> Self {
        Self {
            inner: MemoryEventStore::new(),
            appends: Arc::new(AtomicUsize::new(0)),
            fail_after: Arc::new(AtomicUsize::new(fail_after)),
        }
    }

    fn heal(&self) {
        self.fail_after.store(usize::MAX, Ordering::SeqCst);
    }
}

#[async_trait]
impl EventStore for FailingStore {
    async fn append(&self, events: Vec<Event>) -> Result<Vec<EventId>, StateError> {
        if self.appends.load(Ordering::SeqCst) >= self.fail_after.load(Ordering::SeqCst) {
            return Err(StateError::Store("injected append failure".into()));
        }
        self.appends.fetch_add(1, Ordering::SeqCst);
        self.inner.append(events).await
    }

    async fn scan(&self) -> Result<Vec<Event>, StateError> {
        self.inner.scan().await
    }

    async fn scan_after(&self, event_id: Option<EventId>) -> Result<Vec<Event>, StateError> {
        self.inner.scan_after(event_id).await
    }
}

fn actor() -> ActorRef {
    ActorRef::agent("t")
}

#[tokio::test]
async fn failed_run_start_rolls_back_the_marker() {
    // fail the SECOND append: run.started lands, its ops pair does not
    let store = FailingStore::new(1);
    let state = YoAgentState::load(store.clone()).await.unwrap();

    let err = state
        .record_run_started(actor(), RunId::new("run_x"), "task")
        .await;
    assert!(err.is_err(), "ops-pair failure must propagate");
    store.heal();

    // the marker was rolled back: a subsequent event is a root with no
    // correlation, NOT chained to the phantom run
    state
        .record_event(Event::new(
            actor(),
            "observation.created",
            json!({"id": "o1"}),
        ))
        .await
        .unwrap();
    let events = state.store().scan().await.unwrap();
    let obs = events
        .iter()
        .find(|e| e.kind == "observation.created")
        .unwrap();
    assert_eq!(obs.causation_id, None);
    assert_eq!(obs.correlation_id, None);

    // the unpaired run.started stays in the append-only log (documented)
    assert_eq!(events.iter().filter(|e| e.kind == "run.started").count(), 1);

    // and the run can be started cleanly afterwards
    state
        .record_run_started(actor(), RunId::new("run_x"), "task")
        .await
        .unwrap();
}

#[tokio::test]
async fn failed_run_finish_keeps_the_run_open_for_retry() {
    let store = FailingStore::new(3); // run.started + ops + run.finished land
    let state = YoAgentState::load(store.clone()).await.unwrap();
    state
        .record_run_started(actor(), RunId::new("run_x"), "task")
        .await
        .unwrap();

    // the finish's ops append fails -> Err, and the run stays open
    let err = state
        .record_run_finished(actor(), RunId::new("run_x"), "done")
        .await;
    assert!(err.is_err());
    store.heal();

    // retry succeeds (appending a fresh run.finished — documented semantics)
    state
        .record_run_finished(actor(), RunId::new("run_x"), "done")
        .await
        .unwrap();
    let events = state.store().scan().await.unwrap();
    assert_eq!(
        events.iter().filter(|e| e.kind == "run.finished").count(),
        2
    );

    // after the successful finish the slot is clear: finishing again errors
    let err = state
        .record_run_finished(actor(), RunId::new("run_x"), "done")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no run is open"));
}

#[tokio::test]
async fn resume_open_run_recovers_the_marker_across_processes() {
    let store = MemoryEventStore::new();
    let state = YoAgentState::load(store.clone()).await.unwrap();
    state
        .record_run_started(actor(), RunId::new("run_x"), "task")
        .await
        .unwrap();

    // "new process": fresh load, marker gone — resume recovers it
    let state2 = YoAgentState::load((*state.store()).clone()).await.unwrap();
    let resumed = state2.resume_open_run().await.unwrap();
    assert_eq!(resumed, Some(RunId::new("run_x")));

    // events in the resumed process chain + correlate to the original run
    state2
        .record_event(Event::new(
            actor(),
            "observation.created",
            json!({"id": "o1"}),
        ))
        .await
        .unwrap();
    let events = state2.store().scan().await.unwrap();
    let started = events.iter().find(|e| e.kind == "run.started").unwrap();
    let obs = events
        .iter()
        .find(|e| e.kind == "observation.created")
        .unwrap();
    assert_eq!(obs.causation_id.as_ref(), Some(&started.id));
    assert_eq!(obs.correlation_id.as_deref(), Some("run_x"));

    // and the resumed run can be finished
    state2
        .record_run_finished(actor(), RunId::new("run_x"), "done")
        .await
        .unwrap();

    // a third process now finds nothing open
    let state3 = YoAgentState::load((*state2.store()).clone()).await.unwrap();
    assert_eq!(state3.resume_open_run().await.unwrap(), None);
}
