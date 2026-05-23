use crate::{
    ActorRef, ArtifactRef, Decision, DecisionStatus, EvalResult, Event, EventId, EventStore,
    ForkId, ForkSnapshot, Frame, Goal, GoalId, GoalStatus, Graph, GraphDiff, GraphSnapshot,
    Hypothesis, Lineage, ModelCall, NodeId, Observation, PatchId, PatchStatus, ProjectSnapshot,
    RunId, StateError, StateOp, StatePatch, Task, TaskId, TaskStatus, ToolCall, diff_graphs,
    fork_events_at, project_event, replay,
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
        run_id: RunId,
        task: impl Into<String>,
    ) -> Result<EventId, StateError> {
        let task = task.into();
        self.record_event(Event::new(
            actor.clone(),
            "run.started",
            json!({ "run_id": run_id, "task": task }),
        ))
        .await?;
        self.apply_ops(
            actor,
            vec![StateOp::CreateNode {
                id: NodeId::new(run_id.0),
                kind: crate::KIND_RUN.to_string(),
                props: json!({ "task": task, "status": "started" }),
            }],
        )
        .await
    }

    pub async fn record_run_finished(
        &self,
        actor: ActorRef,
        run_id: RunId,
        outcome: impl Into<String>,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor,
            "run.finished",
            json!({ "run_id": run_id, "outcome": outcome.into() }),
        ))
        .await
    }

    pub async fn record_goal(&self, goal: Goal) -> Result<EventId, StateError> {
        let actor = goal.owner.clone();
        self.record_event(Event::new(
            actor.clone(),
            "goal.created",
            serde_json::to_value(&goal)?,
        ))
        .await?;
        self.apply_ops(
            actor,
            vec![StateOp::CreateNode {
                id: NodeId::new(goal.id.0),
                kind: crate::KIND_GOAL.to_string(),
                props: json!({
                    "title": goal.title,
                    "summary": goal.summary,
                    "status": goal.status,
                    "owner": goal.owner,
                    "metadata": goal.metadata,
                }),
            }],
        )
        .await
    }

    pub async fn update_goal_status(
        &self,
        goal_id: GoalId,
        status: GoalStatus,
        reason: Option<String>,
    ) -> Result<EventId, StateError> {
        let actor = ActorRef::system("yoagent-state");
        self.record_event(Event::new(
            actor.clone(),
            "goal.status_changed",
            json!({ "goal_id": goal_id, "status": status, "reason": reason }),
        ))
        .await?;
        self.apply_ops(
            actor,
            vec![StateOp::UpdateNode {
                id: NodeId::new(goal_id.0),
                props: json!({ "status": status, "status_reason": reason }),
            }],
        )
        .await
    }

    pub async fn record_task(&self, task: Task) -> Result<EventId, StateError> {
        let actor = task.created_by.clone();
        self.record_event(Event::new(
            actor.clone(),
            "task.created",
            serde_json::to_value(&task)?,
        ))
        .await?;
        let task_id = NodeId::new(task.id.0);
        let mut ops = vec![StateOp::CreateNode {
            id: task_id.clone(),
            kind: crate::KIND_TASK.to_string(),
            props: json!({
                "title": task.title,
                "summary": task.summary,
                "status": task.status,
                "metadata": task.metadata,
            }),
        }];
        if let Some(goal) = task.goal {
            ops.push(StateOp::CreateRelation {
                from: task_id,
                rel: crate::REL_SERVES.to_string(),
                to: NodeId::new(goal.0),
                props: json!({}),
            });
        }
        self.apply_ops(actor, ops).await
    }

    pub async fn update_task_status(
        &self,
        task_id: TaskId,
        status: TaskStatus,
        reason: Option<String>,
    ) -> Result<EventId, StateError> {
        let actor = ActorRef::system("yoagent-state");
        self.record_event(Event::new(
            actor.clone(),
            "task.status_changed",
            json!({ "task_id": task_id, "status": status, "reason": reason }),
        ))
        .await?;
        self.apply_ops(
            actor,
            vec![StateOp::UpdateNode {
                id: NodeId::new(task_id.0),
                props: json!({ "status": status, "status_reason": reason }),
            }],
        )
        .await
    }

    pub async fn record_observation(
        &self,
        actor: ActorRef,
        observation: Observation,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor.clone(),
            "observation.created",
            serde_json::to_value(&observation)?,
        ))
        .await?;
        let observation_id = NodeId::new(observation.id.0);
        let mut ops = vec![StateOp::CreateNode {
            id: observation_id.clone(),
            kind: crate::KIND_OBSERVATION.to_string(),
            props: json!({
                "title": observation.title,
                "summary": observation.summary,
                "metadata": observation.metadata,
            }),
        }];
        if let Some(run_id) = observation.observed_in {
            ops.push(StateOp::CreateRelation {
                from: observation_id,
                rel: "observed_in".to_string(),
                to: NodeId::new(run_id.0),
                props: json!({}),
            });
        }
        self.apply_ops(actor, ops).await
    }

    pub async fn record_hypothesis(
        &self,
        actor: ActorRef,
        hypothesis: Hypothesis,
        explains: Option<NodeId>,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor.clone(),
            "hypothesis.created",
            serde_json::to_value(&hypothesis)?,
        ))
        .await?;
        let hypothesis_id = NodeId::new(hypothesis.id.0);
        let mut ops = vec![StateOp::CreateNode {
            id: hypothesis_id.clone(),
            kind: crate::KIND_HYPOTHESIS.to_string(),
            props: json!({
                "title": hypothesis.title,
                "summary": hypothesis.summary,
                "confidence": hypothesis.confidence,
                "metadata": hypothesis.metadata,
            }),
        }];
        if let Some(target) = explains {
            ops.push(StateOp::CreateRelation {
                from: hypothesis_id,
                rel: crate::REL_EXPLAINS.to_string(),
                to: target,
                props: json!({}),
            });
        }
        self.apply_ops(actor, ops).await
    }

    pub async fn record_failure(
        &self,
        actor: ActorRef,
        failure_id: NodeId,
        title: impl Into<String>,
        summary: impl Into<String>,
    ) -> Result<EventId, StateError> {
        let title = title.into();
        let summary = summary.into();
        self.record_event(Event::new(
            actor.clone(),
            "failure.observed",
            json!({
                "failure_id": failure_id,
                "title": title,
                "summary": summary,
            }),
        ))
        .await?;
        self.apply_ops(
            actor,
            vec![StateOp::CreateNode {
                id: failure_id,
                kind: "failure".to_string(),
                props: json!({ "title": title, "summary": summary }),
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

    pub async fn record_eval(
        &self,
        actor: ActorRef,
        eval: EvalResult,
        patch_id: Option<PatchId>,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor.clone(),
            "eval.finished",
            serde_json::to_value(&eval)?,
        ))
        .await?;
        let eval_node_id = NodeId::new(eval.id.0);
        let mut ops = vec![StateOp::CreateNode {
            id: eval_node_id.clone(),
            kind: crate::KIND_EVAL.to_string(),
            props: json!({
                "command": eval.command,
                "status": eval.status,
                "score": eval.score,
                "metadata": eval.metadata,
            }),
        }];
        if let Some(patch_id) = patch_id {
            ops.push(StateOp::CreateRelation {
                from: NodeId::new(patch_id.0),
                rel: crate::REL_VALIDATED_BY.to_string(),
                to: eval_node_id,
                props: json!({}),
            });
        }
        self.apply_ops(actor, ops).await
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

    pub async fn record_decision_node(
        &self,
        actor: ActorRef,
        decision: Decision,
        target: Option<NodeId>,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor.clone(),
            "decision.created",
            serde_json::to_value(&decision)?,
        ))
        .await?;
        let decision_node_id = NodeId::new(decision.id.0);
        let mut ops = vec![StateOp::CreateNode {
            id: decision_node_id.clone(),
            kind: crate::KIND_DECISION.to_string(),
            props: json!({
                "status": decision.status,
                "reason": decision.reason,
                "decided_by": decision.decided_by,
                "metadata": decision.metadata,
            }),
        }];
        if let Some(target) = target {
            let rel = match decision.status {
                DecisionStatus::Approved => crate::REL_APPROVED_BY,
                DecisionStatus::Rejected => crate::REL_REJECTED_BY,
                _ => crate::REL_REFERENCES,
            };
            ops.push(StateOp::CreateRelation {
                from: target,
                rel: rel.to_string(),
                to: decision_node_id,
                props: json!({}),
            });
        }
        self.apply_ops(actor, ops).await
    }

    pub async fn record_project_snapshot(
        &self,
        actor: ActorRef,
        snapshot: ProjectSnapshot,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor.clone(),
            "project.snapshot_recorded",
            serde_json::to_value(&snapshot)?,
        ))
        .await?;
        let mut ops = vec![StateOp::CreateNode {
            id: snapshot.id.clone(),
            kind: crate::KIND_PROJECT_SNAPSHOT.to_string(),
            props: json!({
                "project": snapshot.project,
                "metadata": snapshot.metadata,
            }),
        }];
        for artifact in snapshot.artifacts {
            ops.push(StateOp::AttachArtifact {
                id: snapshot.id.clone(),
                artifact,
            });
        }
        self.apply_ops(actor, ops).await
    }

    pub async fn record_model_call(
        &self,
        actor: ActorRef,
        call: ModelCall,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor.clone(),
            "model.called",
            serde_json::to_value(&call)?,
        ))
        .await?;
        self.apply_ops(
            actor,
            vec![
                StateOp::CreateNode {
                    id: call.id.clone(),
                    kind: crate::KIND_MODEL_CALL.to_string(),
                    props: json!({
                        "run_id": call.run_id,
                        "model": call.model,
                        "prompt_summary": call.prompt_summary,
                        "output_summary": call.output_summary,
                        "metadata": call.metadata,
                    }),
                },
                StateOp::CreateRelation {
                    from: call.id,
                    rel: crate::REL_PRODUCED_BY.to_string(),
                    to: NodeId::new(call.run_id.0),
                    props: json!({}),
                },
            ],
        )
        .await
    }

    pub async fn record_tool_call(
        &self,
        actor: ActorRef,
        call: ToolCall,
    ) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor.clone(),
            "tool.called",
            serde_json::to_value(&call)?,
        ))
        .await?;
        self.apply_ops(
            actor,
            vec![
                StateOp::CreateNode {
                    id: call.id.clone(),
                    kind: crate::KIND_TOOL_CALL.to_string(),
                    props: json!({
                        "run_id": call.run_id,
                        "tool": call.tool,
                        "input_summary": call.input_summary,
                        "output_summary": call.output_summary,
                        "success": call.success,
                        "metadata": call.metadata,
                    }),
                },
                StateOp::CreateRelation {
                    from: call.id,
                    rel: crate::REL_PRODUCED_BY.to_string(),
                    to: NodeId::new(call.run_id.0),
                    props: json!({}),
                },
            ],
        )
        .await
    }

    pub async fn record_frame(&self, actor: ActorRef, frame: Frame) -> Result<EventId, StateError> {
        self.record_event(Event::new(
            actor.clone(),
            "frame.created",
            serde_json::to_value(&frame)?,
        ))
        .await?;
        let frame_node_id = NodeId::new(frame.id.0);
        let mut ops = vec![StateOp::CreateNode {
            id: frame_node_id.clone(),
            kind: crate::KIND_FRAME.to_string(),
            props: json!({
                "title": frame.title,
                "summary": frame.summary,
                "metadata": frame.metadata,
            }),
        }];
        if let Some(parent) = frame.parent {
            ops.push(StateOp::CreateRelation {
                from: frame_node_id,
                rel: crate::REL_CONTAINED_IN_FRAME.to_string(),
                to: NodeId::new(parent.0),
                props: json!({}),
            });
        }
        self.apply_ops(actor, ops).await
    }

    pub async fn link(
        &self,
        actor: ActorRef,
        from: NodeId,
        rel: impl Into<String>,
        to: NodeId,
    ) -> Result<EventId, StateError> {
        self.apply_ops(
            actor,
            vec![StateOp::CreateRelation {
                from,
                rel: rel.into(),
                to,
                props: json!({}),
            }],
        )
        .await
    }

    pub async fn fork_at_event(
        &self,
        fork_id: ForkId,
        parent_event: Option<EventId>,
    ) -> Result<ForkSnapshot, StateError> {
        let events = self.store.scan().await?;
        fork_events_at(&events, fork_id, parent_event)
    }

    pub async fn diff_with(&self, other: &Graph) -> GraphDiff {
        let current = self.graph().await;
        diff_graphs(&current, other)
    }
}
