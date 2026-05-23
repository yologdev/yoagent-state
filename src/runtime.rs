use crate::{
    ActorRef, ApprovalRequest, ApprovalStatus, Behavior, BehaviorContext, Event, EventId,
    EventPattern, EventStore, GraphSnapshot, Node, NodeId, Pack, Policy, PolicyAction,
    PolicyDecision, PolicyId, StateError, StateOp, YoAgentState,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct YoAgentRuntime<S: EventStore> {
    state: YoAgentState<S>,
    packs: BTreeMap<String, Pack>,
    behaviors: Vec<Arc<dyn Behavior>>,
    policies: Vec<Policy>,
}

impl<S: EventStore> YoAgentRuntime<S> {
    pub fn new(state: YoAgentState<S>) -> Self {
        Self {
            state,
            packs: BTreeMap::new(),
            behaviors: Vec::new(),
            policies: Vec::new(),
        }
    }

    pub fn state(&self) -> YoAgentState<S> {
        self.state.clone()
    }

    pub fn register_pack(&mut self, pack: Pack) {
        self.packs.insert(pack.id.0.clone(), pack);
    }

    pub fn register_behavior<B>(&mut self, behavior: B)
    where
        B: Behavior + 'static,
    {
        self.behaviors.push(Arc::new(behavior));
    }

    pub fn register_policy(&mut self, policy: Policy) {
        self.policies.push(policy);
    }

    pub async fn emit_event(&self, event: Event) -> Result<EventId, StateError> {
        let event_id = self.state.record_event(event.clone()).await?;
        self.run_behaviors(&event).await?;
        Ok(event_id)
    }

    pub async fn create_typed_node(
        &self,
        actor: ActorRef,
        id: NodeId,
        kind: impl Into<String>,
        props: serde_json::Value,
    ) -> Result<EventId, StateError> {
        self.ensure_allowed(PolicyAction::CreateNode, Some(id.clone()), actor.clone())
            .await?;
        let node = Node::new(id.clone(), kind.into(), props.clone());
        for pack in self.packs.values() {
            pack.validate_node(&node)?;
        }
        self.state
            .apply_ops(
                actor,
                vec![StateOp::CreateNode {
                    id,
                    kind: node.kind,
                    props,
                }],
            )
            .await
    }

    pub async fn create_typed_relation(
        &self,
        actor: ActorRef,
        from: NodeId,
        rel: impl Into<String>,
        to: NodeId,
        props: serde_json::Value,
    ) -> Result<EventId, StateError> {
        self.ensure_allowed(
            PolicyAction::CreateRelation,
            Some(from.clone()),
            actor.clone(),
        )
        .await?;
        let rel = rel.into();
        let graph = self.state.graph().await;
        let from_node = graph
            .get_node(&from)
            .ok_or_else(|| StateError::NodeNotFound(from.clone()))?;
        let to_node = graph
            .get_node(&to)
            .ok_or_else(|| StateError::NodeNotFound(to.clone()))?;
        let relation = crate::Relation::new(from.clone(), rel.clone(), to.clone(), props.clone());
        for pack in self.packs.values() {
            pack.validate_relation(&relation, from_node, to_node)?;
        }
        self.state
            .apply_ops(
                actor,
                vec![StateOp::CreateRelation {
                    from,
                    rel,
                    to,
                    props,
                }],
            )
            .await
    }

    pub async fn request_approval(
        &self,
        policy_id: PolicyId,
        action: PolicyAction,
        target: Option<NodeId>,
        requested_by: ActorRef,
        reason: Option<String>,
    ) -> Result<EventId, StateError> {
        let request = ApprovalRequest {
            id: NodeId::generate(),
            policy_id,
            action,
            target,
            requested_by: requested_by.clone(),
            status: ApprovalStatus::Pending,
            reason,
            metadata: json!({}),
        };
        self.state
            .apply_ops(
                requested_by,
                vec![StateOp::CreateNode {
                    id: request.id.clone(),
                    kind: "approval_request".to_string(),
                    props: serde_json::to_value(request)?,
                }],
            )
            .await
    }

    pub async fn approve_request(
        &self,
        actor: ActorRef,
        request_id: NodeId,
        reason: Option<String>,
    ) -> Result<EventId, StateError> {
        self.state
            .apply_ops(
                actor,
                vec![StateOp::UpdateNode {
                    id: request_id,
                    props: json!({
                        "status": ApprovalStatus::Approved,
                        "approval_reason": reason,
                    }),
                }],
            )
            .await
    }

    pub async fn reject_request(
        &self,
        actor: ActorRef,
        request_id: NodeId,
        reason: Option<String>,
    ) -> Result<EventId, StateError> {
        self.state
            .apply_ops(
                actor,
                vec![StateOp::UpdateNode {
                    id: request_id,
                    props: json!({
                        "status": ApprovalStatus::Rejected,
                        "rejection_reason": reason,
                    }),
                }],
            )
            .await
    }

    pub async fn graph(&self) -> GraphSnapshot {
        self.state.graph().await
    }

    async fn ensure_allowed(
        &self,
        action: PolicyAction,
        target: Option<NodeId>,
        actor: ActorRef,
    ) -> Result<(), StateError> {
        for policy in &self.policies {
            if policy.action != action {
                continue;
            }
            match policy.decision {
                PolicyDecision::Allow => {}
                PolicyDecision::Deny => {
                    return Err(StateError::PolicyDenied(
                        policy
                            .reason
                            .clone()
                            .unwrap_or_else(|| policy.title.clone()),
                    ));
                }
                PolicyDecision::RequireApproval => {
                    self.request_approval(
                        policy.id.clone(),
                        action,
                        target,
                        actor,
                        policy.reason.clone(),
                    )
                    .await?;
                    return Err(StateError::PolicyDenied(format!(
                        "approval required by policy {}",
                        policy.title
                    )));
                }
            }
        }
        Ok(())
    }

    async fn run_behaviors(&self, event: &Event) -> Result<(), StateError> {
        for behavior in &self.behaviors {
            let pattern: EventPattern = behavior.pattern();
            if !pattern.matches(event) {
                continue;
            }
            let ops = behavior
                .handle(
                    BehaviorContext {
                        graph: self.state.graph().await,
                        replaying: false,
                    },
                    event,
                )
                .await?;
            if !ops.is_empty() {
                self.state.apply_ops(event.actor.clone(), ops).await?;
            }
        }
        Ok(())
    }
}
