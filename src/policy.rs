use crate::{ActorRef, NodeId, PolicyId};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyAction {
    CreateNode,
    CreateRelation,
    PromotePatch,
    MutatePrompt,
    MutateToolConfig,
    MutateMemory,
    PromoteProjectPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    RequireApproval,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub id: PolicyId,
    pub title: String,
    pub action: PolicyAction,
    pub decision: PolicyDecision,
    pub reason: Option<String>,
}

impl Policy {
    pub fn require_approval(id: PolicyId, title: impl Into<String>, action: PolicyAction) -> Self {
        Self {
            id,
            title: title.into(),
            action,
            decision: PolicyDecision::RequireApproval,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: NodeId,
    pub policy_id: PolicyId,
    pub action: PolicyAction,
    pub target: Option<NodeId>,
    pub requested_by: ActorRef,
    pub status: ApprovalStatus,
    pub reason: Option<String>,
    pub metadata: JsonValue,
}
