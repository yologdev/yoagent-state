use crate::{
    ActorRef, ArtifactRef, Event, EventId, EventStore, Graph, GraphSnapshot, Lineage, NodeId,
    PatchId, PatchStatus, StateError, StateOp, StatePatch, project_event, replay,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct YoAgentState<S: EventStore> {
    store: Arc<S>,
    graph: Arc<RwLock<Graph>>,
}

impl<S: EventStore> Clone for YoAgentState<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            graph: self.graph.clone(),
        }
    }
}

impl<S: EventStore> YoAgentState<S> {
    pub async fn load(store: S) -> Result<Self, StateError> {
        let events = store.scan().await?;
        let graph = replay(&events)?;
        Ok(Self {
            store: Arc::new(store),
            graph: Arc::new(RwLock::new(graph)),
        })
    }

    pub fn store(&self) -> Arc<S> {
        self.store.clone()
    }

    pub async fn record_event(&self, event: Event) -> Result<EventId, StateError> {
        let ids = self.store.append(vec![event.clone()]).await?;
        {
            let mut graph = self.graph.write().await;
            project_event(&mut graph, &event)?;
        }
        Ok(ids[0].clone())
    }

    pub async fn apply_ops(
        &self,
        actor: ActorRef,
        ops: Vec<StateOp>,
    ) -> Result<EventId, StateError> {
        let event = Event::new(actor, crate::STATE_OPS_APPLIED, serde_json::to_value(ops)?);
        self.record_event(event).await
    }

    pub async fn propose_patch(&self, patch: StatePatch) -> Result<PatchId, StateError> {
        let patch_id = patch.id.clone();
        let actor = patch.created_by.clone();
        self.record_event(Event::new(
            actor.clone(),
            "patch.proposed",
            serde_json::to_value(&patch)?,
        ))
        .await?;

        let patch_node_id = NodeId::new(patch.id.0.clone());
        let mut ops = vec![StateOp::CreateNode {
            id: patch_node_id.clone(),
            kind: "patch".to_string(),
            props: json!({
                "title": patch.title,
                "summary": patch.summary,
                "status": patch.status,
                "base_state_version": patch.base_state_version,
                "preconditions": patch.preconditions,
                "expected_effects": patch.expected_effects,
                "base_project_ref": patch.base_project_ref,
            }),
        }];

        for evidence in patch.evidence {
            ops.push(StateOp::CreateRelation {
                from: patch_node_id.clone(),
                rel: "addresses".to_string(),
                to: evidence,
                props: json!({}),
            });
        }

        for artifact in patch.artifacts {
            ops.push(StateOp::AttachArtifact {
                id: patch_node_id.clone(),
                artifact,
            });
        }

        ops.extend(patch.ops);
        self.apply_ops(actor, ops).await?;
        Ok(patch_id)
    }

    pub async fn update_patch_status(
        &self,
        patch_id: PatchId,
        status: PatchStatus,
        reason: Option<String>,
    ) -> Result<EventId, StateError> {
        let actor = ActorRef::system("yoagent-state");
        let event = Event::new(
            actor.clone(),
            "patch.status_changed",
            json!({
                "patch_id": patch_id,
                "status": status,
                "reason": reason,
            }),
        );
        self.record_event(event).await?;

        self.apply_ops(
            actor,
            vec![StateOp::UpdateNode {
                id: NodeId::new(patch_id.0),
                props: json!({
                    "status": status,
                    "status_reason": reason,
                }),
            }],
        )
        .await
    }

    pub async fn attach_artifact(
        &self,
        node_id: NodeId,
        artifact: ArtifactRef,
    ) -> Result<EventId, StateError> {
        let actor = ActorRef::system("yoagent-state");
        self.record_event(Event::new(
            actor.clone(),
            "artifact.attached",
            json!({
                "node_id": node_id,
                "artifact": artifact,
            }),
        ))
        .await?;

        self.apply_ops(
            actor,
            vec![StateOp::AttachArtifact {
                id: node_id,
                artifact,
            }],
        )
        .await
    }

    pub async fn graph(&self) -> GraphSnapshot {
        self.graph.read().await.clone()
    }

    pub async fn get_node(&self, node_id: NodeId) -> Option<crate::Node> {
        self.graph.read().await.get_node(&node_id).cloned()
    }

    pub async fn outgoing(&self, node_id: NodeId, rel: Option<&str>) -> Vec<crate::Relation> {
        self.graph.read().await.outgoing(&node_id, rel)
    }

    pub async fn incoming(&self, node_id: NodeId, rel: Option<&str>) -> Vec<crate::Relation> {
        self.graph.read().await.incoming(&node_id, rel)
    }

    pub async fn related(&self, node_id: NodeId) -> Vec<crate::Relation> {
        self.graph.read().await.related(&node_id)
    }

    pub async fn patches_for_failure(&self, failure_id: NodeId) -> Vec<crate::Node> {
        let graph = self.graph.read().await;
        graph
            .incoming(&failure_id, Some("addresses"))
            .into_iter()
            .filter_map(|rel| graph.get_node(&rel.from).cloned())
            .filter(|node| node.kind == "patch")
            .collect()
    }

    pub async fn evals_for_patch(&self, patch_id: PatchId) -> Vec<crate::Node> {
        let patch_node_id = NodeId::new(patch_id.0);
        let graph = self.graph.read().await;
        graph
            .outgoing(&patch_node_id, Some("validated_by"))
            .into_iter()
            .filter_map(|rel| graph.get_node(&rel.to).cloned())
            .collect()
    }

    pub async fn lineage(&self, node_id: NodeId) -> Lineage {
        let graph = self.graph.read().await;
        Lineage::from_graph(&graph, &node_id)
    }

    pub async fn record_run_started(
        &self,
        actor: ActorRef,
        run_id: crate::RunId,
        task: impl Into<String>,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor,
            "run.started",
            json!({ "run_id": run_id, "task": task.into() }),
        ))
        .await
    }

    pub async fn record_run_finished(
        &self,
        actor: ActorRef,
        run_id: crate::RunId,
        outcome: impl Into<String>,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor,
            "run.finished",
            json!({ "run_id": run_id, "outcome": outcome.into() }),
        ))
        .await
    }

    pub async fn record_failure(
        &self,
        actor: ActorRef,
        failure_id: NodeId,
        title: impl Into<String>,
        summary: impl Into<String>,
    ) -> Result<EventId, StateError> {
        self.apply_ops(
            actor,
            vec![StateOp::CreateNode {
                id: failure_id,
                kind: "failure".to_string(),
                props: json!({ "title": title.into(), "summary": summary.into() }),
            }],
        )
        .await
    }

    pub async fn record_eval_result(
        &self,
        actor: ActorRef,
        eval_id: NodeId,
        patch_id: PatchId,
        command: impl Into<String>,
        passed: bool,
    ) -> Result<EventId, StateError> {
        let patch_node_id = NodeId::new(patch_id.0);
        self.apply_ops(
            actor,
            vec![
                StateOp::CreateNode {
                    id: eval_id.clone(),
                    kind: "eval".to_string(),
                    props: json!({ "command": command.into(), "passed": passed }),
                },
                StateOp::CreateRelation {
                    from: patch_node_id,
                    rel: "validated_by".to_string(),
                    to: eval_id,
                    props: json!({ "passed": passed }),
                },
            ],
        )
        .await
    }

    pub async fn record_decision(
        &self,
        actor: ActorRef,
        decision_id: NodeId,
        patch_id: PatchId,
        approved: bool,
        reason: impl Into<String>,
    ) -> Result<EventId, StateError> {
        let patch_node_id = NodeId::new(patch_id.0);
        let rel = if approved {
            "approved_by"
        } else {
            "rejected_by"
        };

        self.apply_ops(
            actor,
            vec![
                StateOp::CreateNode {
                    id: decision_id.clone(),
                    kind: "decision".to_string(),
                    props: json!({ "approved": approved, "reason": reason.into() }),
                },
                StateOp::CreateRelation {
                    from: patch_node_id,
                    rel: rel.to_string(),
                    to: decision_id,
                    props: json!({}),
                },
            ],
        )
        .await
    }
}
