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
    /// Position within the op batch this op came from, so a repair tool can
    /// locate the event rather than searching for the node id, which may
    /// appear many times in a long log.
    pub index: usize,
    /// The op variant, e.g. `"UpdateNode"`.
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

/// The node an op names, for ops that name one.
fn missing_node(op: &StateOp) -> Option<NodeId> {
    match op {
        StateOp::UpdateNode { id, .. }
        | StateOp::TombstoneNode { id, .. }
        | StateOp::MarkStale { id, .. }
        | StateOp::AttachArtifact { id, .. } => Some(id.clone()),
        _ => None,
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
            }
            // The version advances either way: it counts ops seen, not ops
            // that changed something, and two readers of the same log must
            // agree on it.
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
