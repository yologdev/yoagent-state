//! The sink adapter must emit GASP-conformant logs: paired ops, valid causal
//! roots, failures with ids, closed runs, and run-scoped correlation.

use serde_json::json;
use yoagent_state::{
    ActorRef, EventStore, MemoryEventStore, NodeId, RunId, YoAgentModelCalled,
    YoAgentModelFinished, YoAgentRunFinished, YoAgentRunStarted, YoAgentState, YoAgentStateAdapter,
    YoAgentStateSink, YoAgentToolCalled, YoAgentToolFinished,
};

#[tokio::test]
async fn adapter_emits_conformant_log() {
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();
    let adapter = YoAgentStateAdapter::new(state.clone(), ActorRef::agent("yoyo"));
    let run = RunId::new("run_a");

    adapter
        .on_run_started(YoAgentRunStarted {
            run_id: run.clone(),
            task: "task".into(),
            metadata: json!({}),
        })
        .await
        .unwrap();
    adapter
        .on_model_called(YoAgentModelCalled {
            run_id: run.clone(),
            model: "claude-opus-4-6".into(),
            prompt_summary: "p".into(),
        })
        .await
        .unwrap();
    adapter
        .on_model_finished(YoAgentModelFinished {
            run_id: run.clone(),
            model: "claude-opus-4-6".into(),
            output_summary: "o".into(),
        })
        .await
        .unwrap();
    adapter
        .on_tool_called(YoAgentToolCalled {
            run_id: run.clone(),
            tool: "cargo test".into(),
            input_summary: "i".into(),
        })
        .await
        .unwrap();
    adapter
        .on_tool_finished(YoAgentToolFinished {
            run_id: run.clone(),
            tool: "cargo test".into(),
            output_summary: "2 failed".into(),
            success: false,
        })
        .await
        .unwrap();
    // a SUCCESSFUL tool finish must not emit a failure
    adapter
        .on_tool_called(YoAgentToolCalled {
            run_id: run.clone(),
            tool: "cargo build".into(),
            input_summary: "i".into(),
        })
        .await
        .unwrap();
    adapter
        .on_tool_finished(YoAgentToolFinished {
            run_id: run.clone(),
            tool: "cargo build".into(),
            output_summary: "ok".into(),
            success: true,
        })
        .await
        .unwrap();

    // run-transition validation propagates through the sink
    let err = adapter
        .on_run_started(YoAgentRunStarted {
            run_id: RunId::new("run_b"),
            task: "task".into(),
            metadata: json!({}),
        })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("already open"), "{err}");

    adapter
        .on_run_finished(YoAgentRunFinished {
            run_id: run.clone(),
            outcome: "reverted".into(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    let events = state.store().scan().await.unwrap();

    // conformance properties the GASP checker enforces:
    let mut seen_ids = std::collections::HashSet::new();
    for event in &events {
        // check 5: valid roots, earlier-reference causation. In an adapter
        // log every ops event is paired, so ops roots are NOT allowed here
        // (stricter than the checker's maintenance-root allowance).
        match &event.causation_id {
            Some(cause) => assert!(
                seen_ids.contains(cause.as_str()),
                "{} cites a later/unknown event",
                event.kind
            ),
            None => assert!(
                event.kind.ends_with(".started") || event.kind.ends_with(".created"),
                "bad root kind: {}",
                event.kind
            ),
        }
        seen_ids.insert(event.id.as_str().to_string());

        // check 7 half: every ops event chains to a domain event
        if event.kind == "state.ops_applied" {
            let cause = event.causation_id.as_ref().expect("paired ops");
            let domain = events.iter().find(|e| &e.id == cause).unwrap();
            assert_ne!(domain.kind, "state.ops_applied");
        }

        // run-scoped correlation on every event
        assert_eq!(
            event.correlation_id.as_deref(),
            Some("run_a"),
            "{} lacks run correlation",
            event.kind
        );
    }

    // exactly one failure (the successful tool emitted none), with the
    // 0.4.0 payload shape pinned: {id, failure_id, title, summary}
    let failures: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "failure.observed")
        .collect();
    assert_eq!(failures.len(), 1, "success=true must not emit a failure");
    let failure = failures[0];
    for key in ["id", "failure_id", "title", "summary"] {
        assert!(
            failure.payload.get(key).is_some(),
            "failure payload lacks `{key}`"
        );
    }
    assert_eq!(failure.payload["title"], json!("tool cargo test failed"));

    // entity-creating events, plus run.started/run.finished (which this impl
    // chooses to pair), each have a paired ops event
    for kind in ["failure.observed", "run.started", "run.finished"] {
        let domain = events.iter().find(|e| e.kind == kind).unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.kind == "state.ops_applied"
                    && e.causation_id.as_ref() == Some(&domain.id)),
            "{kind} has no ops pair"
        );
    }

    // the folded run node is closed, not perpetually "started"
    let node = state.get_node(NodeId::new("run_a")).await.unwrap();
    assert_eq!(node.props["status"], json!("finished"));
    assert_eq!(node.props["outcome"], json!("reverted"));

    // model/tool call nodes exist and are produced_by the run
    let produced = state
        .incoming(NodeId::new("run_a"), Some("produced_by"))
        .await;
    assert_eq!(produced.len(), 3, "model_call + 2 tool_call nodes");
}
