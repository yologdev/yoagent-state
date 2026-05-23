use crate::{Graph, Node, NodeId, Relation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lineage {
    pub root: Option<Node>,
    pub incoming: Vec<Relation>,
    pub outgoing: Vec<Relation>,
    pub related_nodes: Vec<Node>,
}

impl Lineage {
    pub fn from_graph(graph: &Graph, id: &NodeId) -> Self {
        let incoming = graph.incoming(id, None);
        let outgoing = graph.outgoing(id, None);
        let mut related = BTreeMap::new();

        for rel in incoming.iter().chain(outgoing.iter()) {
            for node_id in [&rel.from, &rel.to] {
                if node_id != id
                    && let Some(node) = graph.nodes.get(node_id)
                {
                    related.insert(node_id.clone(), node.clone());
                }
            }
        }

        Self {
            root: graph.nodes.get(id).cloned(),
            incoming,
            outgoing,
            related_nodes: related.into_values().collect(),
        }
    }

    pub fn to_markdown(&self) -> String {
        let Some(root) = &self.root else {
            return "Node not found.\n".to_string();
        };

        let title = root
            .props
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or(root.id.as_str());
        let status = root
            .props
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");

        let mut lines = vec![
            format!("# {}", title),
            String::new(),
            format!("- id: {}", root.id),
            format!("- kind: {}", root.kind),
            format!("- status: {}", status),
        ];

        if !root.artifacts.is_empty() {
            lines.push(String::new());
            lines.push("## Artifacts".to_string());
            for artifact in &root.artifacts {
                lines.push(format!("- {}: {}", artifact.kind, artifact.uri));
            }
        }

        if !self.outgoing.is_empty() {
            lines.push(String::new());
            lines.push("## Outgoing".to_string());
            for rel in &self.outgoing {
                lines.push(format!("- {} -> {}", rel.rel, rel.to));
            }
        }

        if !self.incoming.is_empty() {
            lines.push(String::new());
            lines.push("## Incoming".to_string());
            for rel in &self.incoming {
                lines.push(format!("- {} <- {}", rel.rel, rel.from));
            }
        }

        lines.push(String::new());
        lines.join("\n")
    }
}
