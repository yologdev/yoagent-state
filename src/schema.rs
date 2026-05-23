use crate::{Node, PackId, Relation, StateError};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    pub id: PackId,
    pub name: String,
    pub version: String,
    pub object_types: BTreeMap<String, ObjectType>,
    pub relation_types: BTreeMap<String, RelationType>,
    pub policies: Vec<String>,
    pub prompts: Vec<String>,
    pub settings: JsonValue,
}

impl Pack {
    pub fn new(id: PackId, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            version: version.into(),
            object_types: BTreeMap::new(),
            relation_types: BTreeMap::new(),
            policies: Vec::new(),
            prompts: Vec::new(),
            settings: JsonValue::Object(Default::default()),
        }
    }

    pub fn add_object_type(mut self, object_type: ObjectType) -> Self {
        self.object_types
            .insert(object_type.kind.clone(), object_type);
        self
    }

    pub fn add_relation_type(mut self, relation_type: RelationType) -> Self {
        self.relation_types
            .insert(relation_type.rel.clone(), relation_type);
        self
    }

    pub fn validate_node(&self, node: &Node) -> Result<(), StateError> {
        let Some(object_type) = self.object_types.get(&node.kind) else {
            return Ok(());
        };

        for required in &object_type.required_props {
            if node.props.get(required).is_none_or(|value| value.is_null()) {
                return Err(StateError::Validation(format!(
                    "node {} of kind {} is missing required prop {}",
                    node.id, node.kind, required
                )));
            }
        }

        Ok(())
    }

    pub fn validate_relation(
        &self,
        relation: &Relation,
        from: &Node,
        to: &Node,
    ) -> Result<(), StateError> {
        let Some(relation_type) = self.relation_types.get(&relation.rel) else {
            return Ok(());
        };

        if !relation_type.from_kinds.is_empty() && !relation_type.from_kinds.contains(&from.kind) {
            return Err(StateError::Validation(format!(
                "relation {} cannot start from kind {}",
                relation.rel, from.kind
            )));
        }

        if !relation_type.to_kinds.is_empty() && !relation_type.to_kinds.contains(&to.kind) {
            return Err(StateError::Validation(format!(
                "relation {} cannot point to kind {}",
                relation.rel, to.kind
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectType {
    pub kind: String,
    pub required_props: BTreeSet<String>,
    pub description: Option<String>,
}

impl ObjectType {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            required_props: BTreeSet::new(),
            description: None,
        }
    }

    pub fn require(mut self, prop: impl Into<String>) -> Self {
        self.required_props.insert(prop.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationType {
    pub rel: String,
    pub from_kinds: BTreeSet<String>,
    pub to_kinds: BTreeSet<String>,
    pub description: Option<String>,
}

impl RelationType {
    pub fn new(rel: impl Into<String>) -> Self {
        Self {
            rel: rel.into(),
            from_kinds: BTreeSet::new(),
            to_kinds: BTreeSet::new(),
            description: None,
        }
    }

    pub fn from_kind(mut self, kind: impl Into<String>) -> Self {
        self.from_kinds.insert(kind.into());
        self
    }

    pub fn to_kind(mut self, kind: impl Into<String>) -> Self {
        self.to_kinds.insert(kind.into());
        self
    }
}
