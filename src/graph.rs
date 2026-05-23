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

impl Graph {
    pub fn apply_ops(&mut self, ops: &[StateOp]) -> Result<(), StateError> {
        for op in ops {
            self.apply_op(op)?;
            self.version += 1;
        }
        Ok(())
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
