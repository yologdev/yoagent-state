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
    /// The open run, if any: its `RunId` and its `run.started` event id.
    /// Shared across clones — at most one run may be open per state handle
    /// and all its clones (enforced by `record_run_started` /
    /// `record_run_finished`). In-memory only: `load` does not recover an
    /// open run from the log, so a process restarted mid-run must start a
    /// new run before recording chained events.
    current_run: Arc<RwLock<Option<(RunId, EventId)>>>,
}

impl<S: EventStore> Clone for YoAgentState<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            graph: self.graph.clone(),
            current_run: self.current_run.clone(),
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
            current_run: Arc::new(RwLock::new(None)),
        })
    }

    pub fn store(&self) -> Arc<S> {
        self.store.clone()
    }

    /// Append one event. Events recorded without an explicit `causation_id`
    /// while a run is open (between `record_run_started` and
    /// `record_run_finished`) are chained to the run's start event, and
    /// events without an explicit `correlation_id` are correlated to the
    /// run's id. Provided all activity happens inside runs within a single
    /// process lifetime, this keeps the causation graph rooted at
    /// `*.created` / `*.started` — events recorded with no open run become
    /// roots of whatever kind they are and carry no run correlation, and the
    /// open-run marker is not recovered by `load`.
    pub async fn record_event(&self, mut event: Event) -> Result<EventId, StateError> {
        if event.causation_id.is_none() || event.correlation_id.is_none() {
            if let Some((run_id, run_event)) = self.current_run.read().await.clone() {
                if event.causation_id.is_none() && event.kind != "run.started" {
                    event.causation_id = Some(run_event);
                }
                if event.correlation_id.is_none() {
                    event.correlation_id = Some(run_id.0);
                }
            }
        }
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
        self.apply_ops_caused_by(actor, ops, None).await
    }

    /// Append a `state.ops_applied` event carrying the domain event that caused
    /// it, per the GASP pairing rule: ops that materialize a domain event set
    /// `causation_id` to that event's id, keeping the audit layer and the
    /// folded graph mechanically linked.
    pub async fn apply_ops_caused_by(
        &self,
        actor: ActorRef,
        ops: Vec<StateOp>,
        caused_by: Option<EventId>,
    ) -> Result<EventId, StateError> {
        let mut event = Event::new(actor, crate::STATE_OPS_APPLIED, serde_json::to_value(ops)?);
        event.causation_id = caused_by;
        self.record_event(event).await
    }

    pub async fn propose_patch(&self, patch: StatePatch) -> Result<PatchId, StateError> {
        let patch_id = patch.id.clone();
        let actor = patch.created_by.clone();
        let caused_by = self
            .record_event(Event::new(
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
        self.apply_ops_caused_by(actor, ops, Some(caused_by))
            .await?;
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
        let caused_by = self.record_event(event).await?;

        self.apply_ops_caused_by(
            actor,
            vec![StateOp::UpdateNode {
                id: NodeId::new(patch_id.0),
                props: json!({
                    "status": status,
                    "status_reason": reason,
                }),
            }],
            Some(caused_by),
        )
        .await
    }

    pub async fn attach_artifact(
        &self,
        node_id: NodeId,
        artifact: ArtifactRef,
    ) -> Result<EventId, StateError> {
        let actor = ActorRef::system("yoagent-state");
        let caused_by = self
            .record_event(Event::new(
                actor.clone(),
                "artifact.attached",
                json!({
                    "node_id": node_id,
                    "artifact": artifact,
                }),
            ))
            .await?;

        self.apply_ops_caused_by(
            actor,
            vec![StateOp::AttachArtifact {
                id: node_id,
                artifact,
            }],
            Some(caused_by),
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
        if let Some((open_run, _)) = self.current_run.read().await.as_ref() {
            return Err(StateError::Validation(format!(
                "run {open_run} is already open; finish it before starting {run_id}"
            )));
        }
        let task = task.into();
        let caused_by = self
            .record_event(
                Event::new(
                    actor.clone(),
                    "run.started",
                    json!({ "run_id": run_id, "task": task }),
                )
                .with_correlation(run_id.0.clone()),
            )
            .await?;
        // Open the run before the ops pair so the pair itself picks up the
        // run correlation. On ops failure, roll back the open-run MARKER only
        // (the run.started event stays in the log as a valid unpaired root)
        // so later events don't chain or correlate to a run whose node was
        // never created. Clones recording concurrently in this narrow window
        // may still chain to it — benign: the cited event exists in the log.
        *self.current_run.write().await = Some((run_id.clone(), caused_by.clone()));
        let result = self
            .apply_ops_caused_by(
                actor,
                vec![StateOp::CreateNode {
                    id: NodeId::new(run_id.0),
                    kind: crate::KIND_RUN.to_string(),
                    props: json!({ "task": task, "status": "started" }),
                }],
                Some(caused_by),
            )
            .await;
        if result.is_err() {
            *self.current_run.write().await = None;
        }
        result
    }

    pub async fn record_run_finished(
        &self,
        actor: ActorRef,
        run_id: RunId,
        outcome: impl Into<String>,
    ) -> Result<EventId, StateError> {
        match self.current_run.read().await.as_ref() {
            Some((open_run, _)) if *open_run == run_id => {}
            Some((open_run, _)) => {
                return Err(StateError::Validation(format!(
                    "cannot finish {run_id}: the open run is {open_run}"
                )));
            }
            None => {
                return Err(StateError::Validation(format!(
                    "cannot finish {run_id}: no run is open"
                )));
            }
        }
        let outcome = outcome.into();
        // On failure the open-run marker stays set, so finish can be retried;
        // a retry appends a fresh run.finished domain event (the earlier one
        // remains in the append-only log, unpaired).
        let caused_by = self
            .record_event(Event::new(
                actor.clone(),
                "run.finished",
                json!({ "run_id": run_id, "outcome": outcome }),
            ))
            .await?;
        // Pair the finish so the folded run node doesn't stay "started" forever.
        let event_id = self
            .apply_ops_caused_by(
                actor,
                vec![StateOp::UpdateNode {
                    id: NodeId::new(run_id.0.clone()),
                    props: json!({ "status": "finished", "outcome": outcome }),
                }],
                Some(caused_by),
            )
            .await?;
        *self.current_run.write().await = None;
        Ok(event_id)
    }

    pub async fn record_goal(&self, goal: Goal) -> Result<EventId, StateError> {
        let actor = goal.owner.clone();
        let caused_by = self
            .record_event(Event::new(
                actor.clone(),
                "goal.created",
                serde_json::to_value(&goal)?,
            ))
            .await?;
        self.apply_ops_caused_by(
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
            Some(caused_by),
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
        let caused_by = self
            .record_event(Event::new(
                actor.clone(),
                "goal.status_changed",
                json!({ "goal_id": goal_id, "status": status, "reason": reason }),
            ))
            .await?;
        self.apply_ops_caused_by(
            actor,
            vec![StateOp::UpdateNode {
                id: NodeId::new(goal_id.0),
                props: json!({ "status": status, "status_reason": reason }),
            }],
            Some(caused_by),
        )
        .await
    }

    pub async fn record_task(&self, task: Task) -> Result<EventId, StateError> {
        let actor = task.created_by.clone();
        let caused_by = self
            .record_event(Event::new(
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
        self.apply_ops_caused_by(actor, ops, Some(caused_by)).await
    }

    pub async fn update_task_status(
        &self,
        task_id: TaskId,
        status: TaskStatus,
        reason: Option<String>,
    ) -> Result<EventId, StateError> {
        let actor = ActorRef::system("yoagent-state");
        let caused_by = self
            .record_event(Event::new(
                actor.clone(),
                "task.status_changed",
                json!({ "task_id": task_id, "status": status, "reason": reason }),
            ))
            .await?;
        self.apply_ops_caused_by(
            actor,
            vec![StateOp::UpdateNode {
                id: NodeId::new(task_id.0),
                props: json!({ "status": status, "status_reason": reason }),
            }],
            Some(caused_by),
        )
        .await
    }

    pub async fn record_observation(
        &self,
        actor: ActorRef,
        observation: Observation,
    ) -> Result<EventId, StateError> {
        let caused_by = self
            .record_event(Event::new(
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
                rel: crate::REL_OBSERVES.to_string(),
                to: NodeId::new(run_id.0),
                props: json!({}),
            });
        }
        self.apply_ops_caused_by(actor, ops, Some(caused_by)).await
    }

    pub async fn record_hypothesis(
        &self,
        actor: ActorRef,
        hypothesis: Hypothesis,
        explains: Option<NodeId>,
    ) -> Result<EventId, StateError> {
        let caused_by = self
            .record_event(Event::new(
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
        self.apply_ops_caused_by(actor, ops, Some(caused_by)).await
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
        let caused_by = self
            .record_event(Event::new(
                actor.clone(),
                "failure.observed",
                json!({
                    "id": failure_id,
                    "failure_id": failure_id,
                    "title": title,
                    "summary": summary,
                }),
            ))
            .await?;
        self.apply_ops_caused_by(
            actor,
            vec![StateOp::CreateNode {
                id: failure_id,
                kind: "failure".to_string(),
                props: json!({ "title": title, "summary": summary }),
            }],
            Some(caused_by),
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
        let caused_by = self
            .record_event(Event::new(
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
        self.apply_ops_caused_by(actor, ops, Some(caused_by)).await
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
        let caused_by = self
            .record_event(Event::new(
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
        self.apply_ops_caused_by(actor, ops, Some(caused_by)).await
    }

    pub async fn record_project_snapshot(
        &self,
        actor: ActorRef,
        snapshot: ProjectSnapshot,
    ) -> Result<EventId, StateError> {
        let caused_by = self
            .record_event(Event::new(
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
        self.apply_ops_caused_by(actor, ops, Some(caused_by)).await
    }

    pub async fn record_model_call(
        &self,
        actor: ActorRef,
        call: ModelCall,
    ) -> Result<EventId, StateError> {
        let caused_by = self
            .record_event(Event::new(
                actor.clone(),
                "model.called",
                serde_json::to_value(&call)?,
            ))
            .await?;
        self.apply_ops_caused_by(
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
            Some(caused_by),
        )
        .await
    }

    pub async fn record_tool_call(
        &self,
        actor: ActorRef,
        call: ToolCall,
    ) -> Result<EventId, StateError> {
        let caused_by = self
            .record_event(Event::new(
                actor.clone(),
                "tool.called",
                serde_json::to_value(&call)?,
            ))
            .await?;
        self.apply_ops_caused_by(
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
            Some(caused_by),
        )
        .await
    }

    pub async fn record_frame(&self, actor: ActorRef, frame: Frame) -> Result<EventId, StateError> {
        let caused_by = self
            .record_event(Event::new(
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
        self.apply_ops_caused_by(actor, ops, Some(caused_by)).await
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
