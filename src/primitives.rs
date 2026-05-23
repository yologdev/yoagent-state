use crate::{
    ActorRef, ArtifactRef, DecisionId, EvalId, FrameId, GoalId, HypothesisId, NodeId,
    ObservationId, ProjectRef, RunId, TaskId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

pub const KIND_GOAL: &str = "goal";
pub const KIND_TASK: &str = "task";
pub const KIND_RUN: &str = "run";
pub const KIND_OBSERVATION: &str = "observation";
pub const KIND_FAILURE: &str = "failure";
pub const KIND_HYPOTHESIS: &str = "hypothesis";
pub const KIND_PATCH: &str = "patch";
pub const KIND_EVAL: &str = "eval";
pub const KIND_DECISION: &str = "decision";
pub const KIND_PROJECT_SNAPSHOT: &str = "project_snapshot";
pub const KIND_MODEL_CALL: &str = "model_call";
pub const KIND_TOOL_CALL: &str = "tool_call";
pub const KIND_FRAME: &str = "frame";

pub const REL_SERVES: &str = "serves";
pub const REL_BLOCKS: &str = "blocks";
pub const REL_ADVANCES: &str = "advances";
pub const REL_OBSERVES: &str = "observes";
pub const REL_EXPLAINS: &str = "explains";
pub const REL_ADDRESSES: &str = "addresses";
pub const REL_MODIFIES: &str = "modifies";
pub const REL_VALIDATED_BY: &str = "validated_by";
pub const REL_APPROVED_BY: &str = "approved_by";
pub const REL_REJECTED_BY: &str = "rejected_by";
pub const REL_PRODUCED_BY: &str = "produced_by";
pub const REL_DERIVED_FROM: &str = "derived_from";
pub const REL_DEPENDS_ON: &str = "depends_on";
pub const REL_SUPERSEDES: &str = "supersedes";
pub const REL_CONTAINED_IN_FRAME: &str = "contained_in_frame";
pub const REL_FORKED_FROM: &str = "forked_from";
pub const REL_REFERENCES: &str = "references";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalStatus {
    Open,
    InProgress,
    Satisfied,
    Abandoned,
    Blocked,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Open,
    InProgress,
    Done,
    Blocked,
    Abandoned,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalStatus {
    Started,
    Passed,
    Failed,
    Error,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionStatus {
    Pending,
    Approved,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BehaviorStatus {
    Enabled,
    Disabled,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyStatus {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForkStatus {
    Open,
    Merged,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub id: GoalId,
    pub title: String,
    pub summary: String,
    pub status: GoalStatus,
    pub owner: ActorRef,
    pub metadata: JsonValue,
}

impl Goal {
    pub fn new(
        id: GoalId,
        title: impl Into<String>,
        summary: impl Into<String>,
        owner: ActorRef,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            summary: summary.into(),
            status: GoalStatus::Open,
            owner,
            metadata: json!({}),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub summary: String,
    pub status: TaskStatus,
    pub goal: Option<GoalId>,
    pub created_by: ActorRef,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub title: String,
    pub summary: String,
    pub observed_in: Option<RunId>,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub title: String,
    pub summary: String,
    pub confidence: Option<f64>,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalResult {
    pub id: EvalId,
    pub command: String,
    pub status: EvalStatus,
    pub score: Option<f64>,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub id: DecisionId,
    pub status: DecisionStatus,
    pub reason: String,
    pub decided_by: ActorRef,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub id: NodeId,
    pub project: ProjectRef,
    pub artifacts: Vec<ArtifactRef>,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCall {
    pub id: NodeId,
    pub run_id: RunId,
    pub model: String,
    pub prompt_summary: String,
    pub output_summary: Option<String>,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: NodeId,
    pub run_id: RunId,
    pub tool: String,
    pub input_summary: String,
    pub output_summary: Option<String>,
    pub success: Option<bool>,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub id: FrameId,
    pub title: String,
    pub summary: String,
    pub parent: Option<FrameId>,
    pub metadata: JsonValue,
}
