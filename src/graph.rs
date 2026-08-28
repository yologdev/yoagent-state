use crate::{ArtifactRef, NodeId, StateError, StateOp};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: String,
    pub props: JsonValue,
    pub stale: bool,
    pub tombstoned: bool,
    pub artifacts: Vec<ArtifactRef>,
}

impl Node {
    pub fn new(id: NodeId, kind: impl Into<String>, props: JsonValue) -> Self {
        Self {
            id,
            kind: kind.into(),
            props,
            stale: false,
            tombstoned: false,
            artifacts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub from: NodeId,
    pub rel: String,
    pub to: NodeId,
    pub props: JsonValue,
}

impl Relation {
    pub fn new(from: NodeId, rel: impl Into<String>, to: NodeId, props: JsonValue) -> Self {
        Self {
            from,
            rel: rel.into(),
            to,
            props,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: HashMap<NodeId, Node>,
    pub relations: Vec<Relation>,
    pub version: u64,
}

pub type GraphSnapshot = Graph;

/// An op that referenced something the graph does not contain, skipped rather
/// than aborting the fold.
///
/// Returned by [`Graph::apply_ops_lenient`] and
/// [`crate::projector::replay_with_diagnostics`]. A non-empty list means the
/// log is malformed but readable — surface it rather than dropping it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedOp {
    /// Position within the op batch this op came from.
    ///
    /// **This does not identify the event.** One `state.ops_applied` event
    /// carries many ops, and the event's identity is not recorded here — five
    /// skips from five different events all report the same index. The node id
    /// below is the more selective locator today. Carrying an `EventId` is
    /// tracked for 0.6.0, where the struct can gain a field.
    pub index: usize,
    /// The op variant, e.g. `"UpdateNode"`. A string rather than an enum is a
    /// known wart — it forecloses `Deserialize` on this type and gives callers
    /// no exhaustiveness. Tracked for 0.6.0.
    pub op: &'static str,
    /// The node the op referenced, when it named one.
    pub node: Option<NodeId>,
    /// The error the strict path would have returned.
    pub reason: String,
}

fn op_kind(op: &StateOp) -> &'static str {
    match op {
        StateOp::CreateNode { .. } => "CreateNode",
        StateOp::UpdateNode { .. } => "UpdateNode",
        StateOp::TombstoneNode { .. } => "TombstoneNode",
        StateOp::CreateRelation { .. } => "CreateRelation",
        StateOp::DeleteRelation { .. } => "DeleteRelation",
        StateOp::MarkStale { .. } => "MarkStale",
        StateOp::AttachArtifact { .. } => "AttachArtifact",
    }
}

/// The pre-existing node this op requires.
///
/// Not "the node an op names": `CreateNode` names one and creates it, so it can
/// never dangle, and the relation ops name two and are infallible.
///
/// Matched exhaustively on purpose. A `_` arm would let a future `StateOp` that
/// requires an existing node compile clean and report `node: None` in every
/// diagnostic — losing the operator's only handle. That is the exact rot that
/// made the original bug report name two ops when four were affected: the set
/// was read, not enumerated.
fn missing_node(op: &StateOp) -> Option<NodeId> {
    match op {
        StateOp::UpdateNode { id, .. }
        | StateOp::TombstoneNode { id, .. }
        | StateOp::MarkStale { id, .. }
        | StateOp::AttachArtifact { id, .. } => Some(id.clone()),
        StateOp::CreateNode { .. }
        | StateOp::CreateRelation { .. }
        | StateOp::DeleteRelation { .. } => None,
    }
}

impl Graph {
    pub fn apply_ops(&mut self, ops: &[StateOp]) -> Result<(), StateError> {
        for op in ops {
            self.apply_op(op)?;
            self.version += 1;
        }
        Ok(())
    }

    /// Apply ops, skipping any that reference a node that does not exist, and
    /// returning what was skipped.
    ///
    /// # Why this exists
    ///
    /// [`apply_ops`](Self::apply_ops) aborts the whole fold on the first
    /// dangling reference. On an append-only log that is unrecoverable: the
    /// offending op cannot be removed, and no op appended afterwards is ever
    /// reached, so the store becomes permanently unreadable. Inserting a
    /// corrective op *before* it repairs the fold but rewrites published
    /// history, which is the one property the format exists to guarantee.
    ///
    /// A dangling `UpdateNode` is semantically a no-op — it modifies nothing
    /// that exists. Refusing to read the rest of the log because of it
    /// discards information for no gain.
    ///
    /// # Not silent
    ///
    /// Every skip is returned. Callers are expected to surface them: a skip
    /// means the log says something the graph cannot represent, which is worth
    /// a human's attention even though it is survivable. Discarding this
    /// return value turns a recoverable store into a quietly wrong one.
    ///
    /// Use [`apply_ops`](Self::apply_ops) when validating rather than reading —
    /// a validator *should* reject a malformed log.
    #[must_use = "the skipped ops are the only signal that the log is malformed; \
                  dropping them is what turns a recoverable store into a quiet one"]
    pub fn apply_ops_lenient(&mut self, ops: &[StateOp]) -> Vec<SkippedOp> {
        let mut skipped = Vec::new();
        for (index, op) in ops.iter().enumerate() {
            if let Err(err) = self.apply_op(op) {
                skipped.push(SkippedOp {
                    index,
                    op: op_kind(op),
                    node: missing_node(op),
                    reason: err.to_string(),
                });
                // Do NOT advance on a skip.
                //
                // `version` must match what a *live* runtime holds, because
                // that is what seals snapshots, and gasp check 2 compares a
                // snapshot to a fold of its log prefix by whole-`Graph`
                // equality — `version` included. Strict `apply_ops` increments
                // only after `apply_op` succeeds, so a writer that hits a
                // dangling op does not count it. Measured on the production
                // shape (one event, one dangling `UpdateNode`):
                //
                //     live.version   = 0   (strict, aborted before increment)
                //     replay.version = 1   (lenient, had it counted the skip)
                //
                // Counting the skip made every reader disagree with the writer
                // and would fail snapshot verification the day a snapshot
                // emitter ships — reporting non-conformance for a store every
                // runtime can restore, which is the inversion this whole change
                // exists to prevent.
                //
                // This aligns the single-op batch, which is the shape seen in
                // the wild. A multi-op batch still diverges, because strict
                // abandons the ops *after* the failure while lenient applies
                // them; closing that needs the write path to stop appending
                // before it folds.
                continue;
            }
            self.version += 1;
        }
        skipped
    }

    pub fn apply_op(&mut self, op: &StateOp) -> Result<(), StateError> {
        match op {
            StateOp::CreateNode { id, kind, props } => {
                self.nodes.insert(
                    id.clone(),
                    Node::new(id.clone(), kind.clone(), props.clone()),
                );
            }
            StateOp::UpdateNode { id, props } => {
                let node = self
                    .nodes
                    .get_mut(id)
                    .ok_or_else(|| StateError::NodeNotFound(id.clone()))?;
                merge_json(&mut node.props, props.clone());
            }
            StateOp::TombstoneNode { id, reason } => {
                let node = self
                    .nodes
                    .get_mut(id)
                    .ok_or_else(|| StateError::NodeNotFound(id.clone()))?;
                node.tombstoned = true;
                merge_json(
                    &mut node.props,
                    serde_json::json!({ "tombstone_reason": reason }),
                );
            }
            StateOp::CreateRelation {
                from,
                rel,
                to,
                props,
            } => {
                self.relations.push(Relation::new(
                    from.clone(),
                    rel.clone(),
                    to.clone(),
                    props.clone(),
                ));
            }
            StateOp::DeleteRelation { from, rel, to } => {
                self.relations
                    .retain(|r| &r.from != from || &r.rel != rel || &r.to != to);
            }
            StateOp::MarkStale { id, reason } => {
                let node = self
                    .nodes
                    .get_mut(id)
                    .ok_or_else(|| StateError::NodeNotFound(id.clone()))?;
                node.stale = true;
                merge_json(
                    &mut node.props,
                    serde_json::json!({ "stale_reason": reason }),
                );
            }
            StateOp::AttachArtifact { id, artifact } => {
                let node = self
                    .nodes
                    .get_mut(id)
                    .ok_or_else(|| StateError::NodeNotFound(id.clone()))?;
                node.artifacts.push(artifact.clone());
            }
        }

        Ok(())
    }

    pub fn get_node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn outgoing(&self, id: &NodeId, rel: Option<&str>) -> Vec<Relation> {
        self.relations
            .iter()
            .filter(|r| &r.from == id && rel.is_none_or(|expected| r.rel == expected))
            .cloned()
            .collect()
    }

    pub fn incoming(&self, id: &NodeId, rel: Option<&str>) -> Vec<Relation> {
        self.relations
            .iter()
            .filter(|r| &r.to == id && rel.is_none_or(|expected| r.rel == expected))
            .cloned()
            .collect()
    }

    pub fn related(&self, id: &NodeId) -> Vec<Relation> {
        self.relations
            .iter()
            .filter(|r| &r.from == id || &r.to == id)
            .cloned()
            .collect()
    }
}

fn merge_json(target: &mut JsonValue, patch: JsonValue) {
    match (target, patch) {
        (JsonValue::Object(target), JsonValue::Object(patch)) => {
            for (key, value) in patch {
                merge_json(target.entry(key).or_insert(JsonValue::Null), value);
            }
        }
        (slot, value) => *slot = value,
    }
}

pub fn props() -> JsonValue {
    JsonValue::Object(Map::new())
}
