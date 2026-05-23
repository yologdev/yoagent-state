//! Simple but effective state and lineage for long-running agents.
//!
//! `yoagent-state` stores append-only events and derives a small semantic graph
//! for patches, evals, decisions, artifacts, and project references. Git and the
//! filesystem remain the source of truth for concrete project changes.

pub mod adapter;
pub mod artifact;
pub mod error;
pub mod event;
pub mod graph;
pub mod ids;
pub mod observer;
pub mod patch;
pub mod projector;
pub mod query;
pub mod state;
pub mod store;

pub use adapter::*;
pub use artifact::*;
pub use error::*;
pub use event::*;
pub use graph::*;
pub use ids::*;
pub use observer::*;
pub use patch::*;
pub use projector::*;
pub use query::*;
pub use state::*;
pub use store::*;
