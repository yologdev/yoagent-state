use crate::{ArtifactRef, Event, EventId, ForkId, ForkSnapshot, GraphSnapshot, StateError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, events: Vec<Event>) -> Result<Vec<EventId>, StateError>;
    async fn scan(&self) -> Result<Vec<Event>, StateError>;
    async fn scan_after(&self, event_id: Option<EventId>) -> Result<Vec<Event>, StateError>;
}

#[async_trait]
pub trait SnapshotStore: Send + Sync {
    async fn save_snapshot(&self, snapshot: GraphSnapshot) -> Result<(), StateError>;
    async fn load_snapshot(&self) -> Result<Option<GraphSnapshot>, StateError>;
}

#[async_trait]
pub trait ForkStore: Send + Sync {
    async fn save_fork(&self, fork: ForkSnapshot) -> Result<(), StateError>;
    async fn load_fork(&self, fork_id: ForkId) -> Result<Option<ForkSnapshot>, StateError>;
}

#[async_trait]
pub trait IndexStore: Send + Sync {
    async fn put_index(&self, key: String, value: serde_json::Value) -> Result<(), StateError>;
    async fn get_index(&self, key: String) -> Result<Option<serde_json::Value>, StateError>;
}

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    async fn put_artifact(
        &self,
        artifact: ArtifactRef,
        bytes: Vec<u8>,
    ) -> Result<ArtifactRef, StateError>;
    async fn get_artifact(&self, artifact: ArtifactRef) -> Result<Option<Vec<u8>>, StateError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexRecord {
    key: String,
    value: serde_json::Value,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryEventStore {
    events: Arc<Mutex<Vec<Event>>>,
}

impl MemoryEventStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EventStore for MemoryEventStore {
    async fn append(&self, events: Vec<Event>) -> Result<Vec<EventId>, StateError> {
        let ids = events.iter().map(|event| event.id.clone()).collect();
        self.events
            .lock()
            .map_err(|err| StateError::Store(err.to_string()))?
            .extend(events);
        Ok(ids)
    }

    async fn scan(&self) -> Result<Vec<Event>, StateError> {
        Ok(self
            .events
            .lock()
            .map_err(|err| StateError::Store(err.to_string()))?
            .clone())
    }

    async fn scan_after(&self, event_id: Option<EventId>) -> Result<Vec<Event>, StateError> {
        scan_after_events(self.scan().await?, event_id)
    }
}

#[derive(Debug, Clone)]
pub struct JsonlEventStore {
    path: PathBuf,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl JsonlEventStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn snapshot_path(&self) -> PathBuf {
        self.path.with_extension("snapshot.json")
    }

    fn index_path(&self) -> PathBuf {
        self.path.with_extension("index.jsonl")
    }

    fn fork_path(&self, fork_id: &ForkId) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("forks")
            .join(format!("{}.json", fork_id.0))
    }
}

#[async_trait]
impl EventStore for JsonlEventStore {
    async fn append(&self, events: Vec<Event>) -> Result<Vec<EventId>, StateError> {
        let _guard = self.lock.lock().await;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut existing = if self.path.exists() {
            tokio::fs::read_to_string(&self.path).await?
        } else {
            String::new()
        };
        let ids = events.iter().map(|event| event.id.clone()).collect();
        for event in events {
            existing.push_str(&serde_json::to_string(&event)?);
            existing.push('\n');
        }
        tokio::fs::write(&self.path, existing).await?;
        Ok(ids)
    }

    async fn scan(&self) -> Result<Vec<Event>, StateError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let raw = tokio::fs::read_to_string(&self.path).await?;
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(StateError::from))
            .collect()
    }

    async fn scan_after(&self, event_id: Option<EventId>) -> Result<Vec<Event>, StateError> {
        scan_after_events(self.scan().await?, event_id)
    }
}

#[async_trait]
impl SnapshotStore for JsonlEventStore {
    async fn save_snapshot(&self, snapshot: GraphSnapshot) -> Result<(), StateError> {
        let path = self.snapshot_path();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, serde_json::to_vec_pretty(&snapshot)?).await?;
        Ok(())
    }

    async fn load_snapshot(&self) -> Result<Option<GraphSnapshot>, StateError> {
        let path = self.snapshot_path();
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&tokio::fs::read(path).await?)?))
    }
}

#[async_trait]
impl ForkStore for JsonlEventStore {
    async fn save_fork(&self, fork: ForkSnapshot) -> Result<(), StateError> {
        let path = self.fork_path(&fork.id);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, serde_json::to_vec_pretty(&fork)?).await?;
        Ok(())
    }

    async fn load_fork(&self, fork_id: ForkId) -> Result<Option<ForkSnapshot>, StateError> {
        let path = self.fork_path(&fork_id);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&tokio::fs::read(path).await?)?))
    }
}

#[async_trait]
impl IndexStore for JsonlEventStore {
    async fn put_index(&self, key: String, value: serde_json::Value) -> Result<(), StateError> {
        let path = self.index_path();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut existing = if path.exists() {
            tokio::fs::read_to_string(&path).await?
        } else {
            String::new()
        };
        existing.push_str(&serde_json::to_string(&IndexRecord { key, value })?);
        existing.push('\n');
        tokio::fs::write(path, existing).await?;
        Ok(())
    }

    async fn get_index(&self, key: String) -> Result<Option<serde_json::Value>, StateError> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(None);
        }
        let mut found = None;
        for line in tokio::fs::read_to_string(path).await?.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let record: IndexRecord = serde_json::from_str(line)?;
            if record.key == key {
                found = Some(record.value);
            }
        }
        Ok(found)
    }
}

pub(crate) fn scan_after_events(
    events: Vec<Event>,
    event_id: Option<EventId>,
) -> Result<Vec<Event>, StateError> {
    let Some(event_id) = event_id else {
        return Ok(events);
    };

    let position = events
        .iter()
        .position(|event| event.id == event_id)
        .ok_or(StateError::EventNotFound(event_id))?;
    Ok(events.into_iter().skip(position + 1).collect())
}
