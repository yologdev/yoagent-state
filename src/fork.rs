use crate::{Event, EventId, ForkId, Graph, StateError, replay};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForkSnapshot {
    pub id: ForkId,
    pub parent_event: Option<EventId>,
    pub events: Vec<Event>,
    pub graph: Graph,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphDiff {
    pub added_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub changed_nodes: Vec<String>,
    pub added_relations: Vec<String>,
    pub removed_relations: Vec<String>,
}

pub fn fork_events_at(
    events: &[Event],
    id: ForkId,
    parent_event: Option<EventId>,
) -> Result<ForkSnapshot, StateError> {
    let cutoff = match &parent_event {
        Some(parent_event) => {
            events
                .iter()
                .position(|event| &event.id == parent_event)
                .ok_or_else(|| StateError::EventNotFound(parent_event.clone()))?
                + 1
        }
        None => events.len(),
    };
    let forked_events = events.iter().take(cutoff).cloned().collect::<Vec<_>>();
    let graph = replay(&forked_events)?;
    Ok(ForkSnapshot {
        id,
        parent_event,
        events: forked_events,
        graph,
    })
}

pub fn diff_graphs(left: &Graph, right: &Graph) -> GraphDiff {
    let left_nodes = left
        .nodes
        .keys()
        .map(|id| id.0.clone())
        .collect::<BTreeSet<_>>();
    let right_nodes = right
        .nodes
        .keys()
        .map(|id| id.0.clone())
        .collect::<BTreeSet<_>>();

    let changed_nodes = left
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            right
                .nodes
                .get(id)
                .filter(|right_node| *right_node != node)
                .map(|_| id.0.clone())
        })
        .collect();

    let left_relations = left
        .relations
        .iter()
        .map(|rel| format!("{}:{}:{}", rel.from, rel.rel, rel.to))
        .collect::<BTreeSet<_>>();
    let right_relations = right
        .relations
        .iter()
        .map(|rel| format!("{}:{}:{}", rel.from, rel.rel, rel.to))
        .collect::<BTreeSet<_>>();

    GraphDiff {
        added_nodes: right_nodes.difference(&left_nodes).cloned().collect(),
        removed_nodes: left_nodes.difference(&right_nodes).cloned().collect(),
        changed_nodes,
        added_relations: right_relations
            .difference(&left_relations)
            .cloned()
            .collect(),
        removed_relations: left_relations
            .difference(&right_relations)
            .cloned()
            .collect(),
    }
}
