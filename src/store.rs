use crate::{Event, EventId, StateError};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, events: Vec<Event>) -> Result<Vec<EventId>, StateError>;
    async fn scan(&self) -> Result<Vec<Event>, StateError>;
    async fn scan_after(&self, event_id: Option<EventId>) -> Result<Vec<Event>, StateError>;
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

fn scan_after_events(
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
