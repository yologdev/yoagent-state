use crate::{EventId, NodeId};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("event not found: {0}")]
    EventNotFound(EventId),

    #[error("node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("invalid event payload for {kind}: {source}")]
    InvalidEventPayload {
        kind: String,
        source: serde_json::Error,
    },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("store error: {0}")]
    Store(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("policy denied: {0}")]
    PolicyDenied(String),
}
