//! In-process sink for callback-driven agents — the integration contract as a
//! typed interface. `YoAgentStateAdapter` routes the entity-creating callbacks
//! (run start/finish, model/tool calls, tool failures) through the paired
//! `record_*` helpers and records the non-entity-creating finish events
//! (`model.finished`, `tool.finished`) raw, auto-chained and correlated to the
//! open run — so a log emitted through the sink is GASP-conformant: runs
//! open/close (with validation), tool and model calls become paired nodes
//! chained to the run, and failures carry ids and pairs.
//!
//! Contract: callbacks between `on_run_started` and `on_run_finished` belong
//! to that run; calling other callbacks with no run open produces events that
//! root at raw kinds (invalid roots under conformance check 5) and is
//! non-conformant. Known limitations: the run structs' `metadata` fields are
//! not persisted by the paired helpers, and the `*_finished` callbacks do not
//! update the call nodes' `output_summary`/`success` — call outcomes live in
//! the raw events, not the folded graph.

use crate::{
    ActorRef, Event, EventId, EventStore, ModelCall, NodeId, RunId, StateError, ToolCall,
    YoAgentState,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoAgentRunStarted {
    pub run_id: RunId,
    pub task: String,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoAgentRunFinished {
    pub run_id: RunId,
    pub outcome: String,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoAgentModelCalled {
    pub run_id: RunId,
    pub model: String,
    pub prompt_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoAgentModelFinished {
    pub run_id: RunId,
    pub model: String,
    pub output_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoAgentToolCalled {
    pub run_id: RunId,
    pub tool: String,
    pub input_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoAgentToolFinished {
    pub run_id: RunId,
    pub tool: String,
    pub output_summary: String,
    pub success: bool,
}

#[async_trait]
pub trait YoAgentStateSink: Send + Sync {
    async fn on_run_started(&self, event: YoAgentRunStarted) -> Result<EventId, StateError>;
    async fn on_run_finished(&self, event: YoAgentRunFinished) -> Result<EventId, StateError>;
    async fn on_model_called(&self, event: YoAgentModelCalled) -> Result<EventId, StateError>;
    async fn on_model_finished(&self, event: YoAgentModelFinished) -> Result<EventId, StateError>;
    async fn on_tool_called(&self, event: YoAgentToolCalled) -> Result<EventId, StateError>;
    async fn on_tool_finished(&self, event: YoAgentToolFinished) -> Result<EventId, StateError>;
}

#[derive(Debug, Clone)]
pub struct YoAgentStateAdapter<S: EventStore> {
    state: YoAgentState<S>,
    actor: ActorRef,
}

impl<S: EventStore> YoAgentStateAdapter<S> {
    pub fn new(state: YoAgentState<S>, actor: ActorRef) -> Self {
        Self { state, actor }
    }

    async fn record<T: Serialize + Send + Sync>(
        &self,
        kind: &'static str,
        event: T,
    ) -> Result<EventId, StateError> {
        self.state
            .record_event(Event::new(
                self.actor.clone(),
                kind,
                serde_json::to_value(event)?,
            ))
            .await
    }
}

#[async_trait]
impl<S: EventStore> YoAgentStateSink for YoAgentStateAdapter<S> {
    async fn on_run_started(&self, event: YoAgentRunStarted) -> Result<EventId, StateError> {
        self.state
            .record_run_started(self.actor.clone(), event.run_id, event.task)
            .await
    }

    async fn on_run_finished(&self, event: YoAgentRunFinished) -> Result<EventId, StateError> {
        self.state
            .record_run_finished(self.actor.clone(), event.run_id, event.outcome)
            .await
    }

    async fn on_model_called(&self, event: YoAgentModelCalled) -> Result<EventId, StateError> {
        self.state
            .record_model_call(
                self.actor.clone(),
                ModelCall {
                    id: NodeId::generate(),
                    run_id: event.run_id,
                    model: event.model,
                    prompt_summary: event.prompt_summary,
                    output_summary: None,
                    metadata: json!({}),
                },
            )
            .await
    }

    async fn on_model_finished(&self, event: YoAgentModelFinished) -> Result<EventId, StateError> {
        // Not entity-creating: recorded raw, auto-chained to the open run.
        self.record("model.finished", event).await
    }

    async fn on_tool_called(&self, event: YoAgentToolCalled) -> Result<EventId, StateError> {
        self.state
            .record_tool_call(
                self.actor.clone(),
                ToolCall {
                    id: NodeId::generate(),
                    run_id: event.run_id,
                    tool: event.tool,
                    input_summary: event.input_summary,
                    output_summary: None,
                    success: None,
                    metadata: json!({}),
                },
            )
            .await
    }

    async fn on_tool_finished(&self, event: YoAgentToolFinished) -> Result<EventId, StateError> {
        if !event.success {
            // Paired failure with a generated id, per the pairing rule.
            self.state
                .record_failure(
                    self.actor.clone(),
                    NodeId::generate(),
                    format!("tool {} failed", event.tool),
                    event.output_summary.clone(),
                )
                .await?;
        }
        self.record("tool.finished", event).await
    }
}
