use crate::{EventId, StateError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub const CURRENT_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorRef {
    pub kind: String,
    pub id: String,
}

impl ActorRef {
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }

    pub fn agent(id: impl Into<String>) -> Self {
        Self::new("agent", id)
    }

    pub fn user(id: impl Into<String>) -> Self {
        Self::new("user", id)
    }

    pub fn system(id: impl Into<String>) -> Self {
        Self::new("system", id)
    }

    pub fn tool(id: impl Into<String>) -> Self {
        Self::new("tool", id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub schema_version: u32,
    pub ts_ms: i64,
    pub actor: ActorRef,
    pub kind: String,
    pub payload: JsonValue,
    pub causation_id: Option<EventId>,
    pub correlation_id: Option<String>,
}

impl Event {
    pub fn new(actor: ActorRef, kind: impl Into<String>, payload: JsonValue) -> Self {
        Self {
            id: EventId::generate(),
            schema_version: CURRENT_EVENT_SCHEMA_VERSION,
            ts_ms: now_ms(),
            actor,
            kind: kind.into(),
            payload,
            causation_id: None,
            correlation_id: None,
        }
    }

    pub fn with_causation(mut self, event_id: EventId) -> Self {
        self.causation_id = Some(event_id);
        self
    }

    pub fn with_correlation(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    pub fn payload_as<T>(&self) -> Result<T, StateError>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_value(self.payload.clone()).map_err(|source| {
            StateError::InvalidEventPayload {
                kind: self.kind.clone(),
                source,
            }
        })
    }
}

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
