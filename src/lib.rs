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

/// This crate's version, baked in at *its* compile time.
///
/// Reports what a consumer actually **linked**, not what a lockfile resolved —
/// stronger than either, and it needs no build script. Added because the GASP
/// conformance checker must state which fold produced a verdict: it certifies
/// a store by folding it, so a verdict is only meaningful against a named fold
/// version, and 0.4 and 0.5 fold differently.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
