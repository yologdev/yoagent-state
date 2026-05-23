use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn generate() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::new_v4().simple()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

id_type!(EventId, "event");
id_type!(NodeId, "node");
id_type!(PatchId, "patch");
id_type!(RunId, "run");
id_type!(GoalId, "goal");
id_type!(TaskId, "task");
id_type!(ObservationId, "observation");
id_type!(HypothesisId, "hypothesis");
id_type!(EvalId, "eval");
id_type!(DecisionId, "decision");
id_type!(ArtifactId, "artifact");
id_type!(FrameId, "frame");
id_type!(ForkId, "fork");
id_type!(BehaviorId, "behavior");
id_type!(PolicyId, "policy");
id_type!(PackId, "pack");
id_type!(ViewId, "view");
