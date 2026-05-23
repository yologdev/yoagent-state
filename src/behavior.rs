use crate::{BehaviorId, Event, GraphSnapshot, StateError, StateOp};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventPattern {
    Any,
    Kind(String),
    KindPrefix(String),
    StateOpsApplied,
    PatchStatusChanged,
}

impl EventPattern {
    pub fn matches(&self, event: &Event) -> bool {
        match self {
            EventPattern::Any => true,
            EventPattern::Kind(kind) => event.kind == *kind,
            EventPattern::KindPrefix(prefix) => event.kind.starts_with(prefix),
            EventPattern::StateOpsApplied => event.kind == crate::STATE_OPS_APPLIED,
            EventPattern::PatchStatusChanged => event.kind == "patch.status_changed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BehaviorContext {
    pub graph: GraphSnapshot,
    pub replaying: bool,
}

#[async_trait]
pub trait Behavior: Send + Sync {
    fn id(&self) -> BehaviorId;
    fn pattern(&self) -> EventPattern;
    async fn handle(&self, ctx: BehaviorContext, event: &Event)
    -> Result<Vec<StateOp>, StateError>;
}

pub struct FnBehavior<F> {
    id: BehaviorId,
    pattern: EventPattern,
    f: F,
}

impl<F> FnBehavior<F> {
    pub fn new(id: BehaviorId, pattern: EventPattern, f: F) -> Self {
        Self { id, pattern, f }
    }
}

#[async_trait]
impl<F, Fut> Behavior for FnBehavior<F>
where
    F: Send + Sync + Fn(BehaviorContext, Event) -> Fut,
    Fut: Send + std::future::Future<Output = Result<Vec<StateOp>, StateError>>,
{
    fn id(&self) -> BehaviorId {
        self.id.clone()
    }

    fn pattern(&self) -> EventPattern {
        self.pattern.clone()
    }

    async fn handle(
        &self,
        ctx: BehaviorContext,
        event: &Event,
    ) -> Result<Vec<StateOp>, StateError> {
        (self.f)(ctx, event.clone()).await
    }
}
