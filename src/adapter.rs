use crate::{ActorRef, Event, EventId, EventStore, RunId, StateError, YoAgentState};
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
        self.record("run.started", event).await
    }

    async fn on_run_finished(&self, event: YoAgentRunFinished) -> Result<EventId, StateError> {
        self.record("run.finished", event).await
    }

    async fn on_model_called(&self, event: YoAgentModelCalled) -> Result<EventId, StateError> {
        self.record("model.called", event).await
    }

    async fn on_model_finished(&self, event: YoAgentModelFinished) -> Result<EventId, StateError> {
        self.record("model.finished", event).await
    }

    async fn on_tool_called(&self, event: YoAgentToolCalled) -> Result<EventId, StateError> {
        self.record("tool.called", event).await
    }

    async fn on_tool_finished(&self, event: YoAgentToolFinished) -> Result<EventId, StateError> {
        if !event.success {
            self.state
                .record_event(Event::new(
                    self.actor.clone(),
                    "failure.observed",
                    json!({
                        "run_id": event.run_id,
                        "tool": event.tool,
                        "output_summary": event.output_summary,
                    }),
                ))
                .await?;
            return self.record("tool.finished", event).await;
        }

        self.record("tool.finished", event).await
    }
}
