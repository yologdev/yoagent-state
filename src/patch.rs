use crate::{ActorRef, ArtifactRef, NodeId, PatchId};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StateOp {
    CreateNode {
        id: NodeId,
        kind: String,
        props: JsonValue,
    },
    UpdateNode {
        id: NodeId,
        props: JsonValue,
    },
    TombstoneNode {
        id: NodeId,
        reason: String,
    },
    CreateRelation {
        from: NodeId,
        rel: String,
        to: NodeId,
        props: JsonValue,
    },
    DeleteRelation {
        from: NodeId,
        rel: String,
        to: NodeId,
    },
    MarkStale {
        id: NodeId,
        reason: String,
    },
    AttachArtifact {
        id: NodeId,
        artifact: ArtifactRef,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatePatch {
    pub id: PatchId,
    pub title: String,
    pub summary: String,
    pub base_state_version: u64,
    pub base_project_ref: Option<ProjectRef>,
    pub ops: Vec<StateOp>,
    pub preconditions: Vec<Precondition>,
    pub expected_effects: Vec<ExpectedEffect>,
    pub evidence: Vec<NodeId>,
    pub artifacts: Vec<ArtifactRef>,
    pub status: PatchStatus,
    pub created_by: ActorRef,
}

impl StatePatch {
    pub fn new(
        id: PatchId,
        title: impl Into<String>,
        summary: impl Into<String>,
        created_by: ActorRef,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            summary: summary.into(),
            base_state_version: 0,
            base_project_ref: None,
            ops: Vec::new(),
            preconditions: Vec::new(),
            expected_effects: Vec::new(),
            evidence: Vec::new(),
            artifacts: Vec::new(),
            status: PatchStatus::Proposed,
            created_by,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchStatus {
    Proposed,
    AppliedInFork,
    Evaluated,
    Approved,
    Rejected,
    Promoted,
    Stale,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub repo: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub worktree: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Precondition {
    StateVersionIs(u64),
    ProjectCommitIs(String),
    NodeExists(NodeId),
    NodeDoesNotExist(NodeId),
    TestStillFailing { name: String },
    FileHashIs { path: String, hash: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExpectedEffect {
    TestPasses { name: String },
    EvalScoreAtLeast { metric: String, value: f64 },
    FailureResolved { failure: NodeId },
    NoRegression { metric: String },
}
