//! Simple but effective state and lineage for long-running agents.
//!
//! `yoagent-state` stores append-only events and derives a small semantic graph
//! for patches, evals, decisions, artifacts, and project references. Git and the
//! filesystem remain the source of truth for concrete project changes.

pub mod adapter;
pub mod artifact;
pub mod behavior;
pub mod error;
pub mod event;
pub mod fork;
pub mod git_store;
pub mod graph;
pub mod ids;
pub mod observer;
pub mod patch;
pub mod policy;
pub mod primitives;
pub mod projector;
pub mod query;
pub mod runtime;
pub mod schema;
pub mod state;
pub mod store;

pub use adapter::*;
pub use artifact::*;
pub use behavior::*;
pub use error::*;
pub use event::*;
pub use fork::*;
pub use git_store::*;
pub use graph::*;
pub use ids::*;
pub use observer::*;
pub use patch::*;
pub use policy::*;
pub use primitives::*;
pub use projector::*;
pub use query::*;
pub use runtime::*;
pub use schema::*;
pub use state::*;
pub use store::*;
