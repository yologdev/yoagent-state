//! Git-backed event store satisfying the GASP store contract:
//!
//! 1. **Durable append, boundary commit.** Every `append` opens the log in
//!    append mode, writes, flushes, and fsyncs — a crash mid-run loses nothing.
//!    `commit_run` makes the one closing git commit per run boundary.
//! 2. **The lease lives inside the append path.** `append` refuses to write
//!    unless this worker holds the cross-process lease at `.agent/lease`
//!    (local-only, gitignored), so a second writer is structurally excluded.

use crate::store::scan_after_events;
use crate::{Event, EventId, EventStore, StateError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub const DEFAULT_EVENTS_PATH: &str = "state/events.jsonl";
pub const LEASE_PATH: &str = ".agent/lease";
const GITIGNORED: &[&str] = &[".agent/lease", ".agent/HEAD"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Lease {
    worker_id: String,
    pid: u32,
    expires_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct GitEventStore {
    repo_root: PathBuf,
    events_rel: PathBuf,
    worker_id: String,
    lease_ttl: Duration,
}

impl GitEventStore {
    /// Open (or initialize the layout of) a GASP agent repo. `repo_root` must
    /// be inside a git work tree; `worker_id` names this writer in the lease.
    pub fn open(
        repo_root: impl Into<PathBuf>,
        worker_id: impl Into<String>,
    ) -> Result<Self, StateError> {
        let store = Self {
            repo_root: repo_root.into(),
            events_rel: PathBuf::from(DEFAULT_EVENTS_PATH),
            worker_id: worker_id.into(),
            lease_ttl: Duration::from_secs(600),
        };
        if !store.repo_root.join(".git").exists() {
            return Err(StateError::Store(format!(
                "{} is not a git repository (the GASP layout requires one)",
                store.repo_root.display()
            )));
        }
        std::fs::create_dir_all(store.events_path().parent().unwrap())?;
        std::fs::create_dir_all(store.lease_path().parent().unwrap())?;
        store.ensure_gitignore()?;
        Ok(store)
    }

    pub fn with_lease_ttl(mut self, ttl: Duration) -> Self {
        self.lease_ttl = ttl;
        self
    }

    pub fn events_path(&self) -> PathBuf {
        self.repo_root.join(&self.events_rel)
    }

    fn lease_path(&self) -> PathBuf {
        self.repo_root.join(LEASE_PATH)
    }

    /// The control plane is local-only: make sure the repo ignores it.
    fn ensure_gitignore(&self) -> Result<(), StateError> {
        let path = self.repo_root.join(".gitignore");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let missing: Vec<&str> = GITIGNORED
            .iter()
            .copied()
            .filter(|entry| !existing.lines().any(|line| line.trim() == *entry))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        let mut content = existing;
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        for entry in missing {
            content.push_str(entry);
            content.push('\n');
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Take or renew the single-writer lease. Errors if another worker holds
    /// an unexpired lease. Called inside `append` — never bolt it on beside.
    fn acquire_lease(&self) -> Result<(), StateError> {
        let path = self.lease_path();
        let now_ms = crate::now_ms();
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(lease) = serde_json::from_str::<Lease>(&raw) {
                if lease.worker_id != self.worker_id && lease.expires_at_ms > now_ms {
                    return Err(StateError::Store(format!(
                        "lease held by worker `{}` (pid {}) until {}",
                        lease.worker_id, lease.pid, lease.expires_at_ms
                    )));
                }
            }
        }
        let lease = Lease {
            worker_id: self.worker_id.clone(),
            pid: std::process::id(),
            expires_at_ms: now_ms + self.lease_ttl.as_millis() as i64,
        };
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&tmp, serde_json::to_vec(&lease)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Release the lease if this worker holds it.
    pub fn release_lease(&self) -> Result<(), StateError> {
        let path = self.lease_path();
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(lease) = serde_json::from_str::<Lease>(&raw) {
                if lease.worker_id == self.worker_id {
                    std::fs::remove_file(&path)?;
                }
            }
        }
        Ok(())
    }

    fn git(&self, args: &[&str]) -> Result<String, StateError> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .args(args)
            .output()
            .map_err(|err| StateError::Store(format!("git: {err}")))?;
        if !out.status.success() {
            return Err(StateError::Store(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// The run-boundary commit: stage the log plus any projection paths and
    /// commit with the GASP trailer. Returns the commit sha, or `None` when
    /// there is nothing to commit.
    pub fn commit_run(
        &self,
        run_id: &str,
        goal: &str,
        outcome: &str,
        extra_paths: &[&str],
    ) -> Result<Option<String>, StateError> {
        let events_rel = self.events_rel.to_string_lossy().to_string();
        let mut add_args = vec!["add", "--", events_rel.as_str()];
        add_args.extend(extra_paths);
        self.git(&add_args)?;
        if self.git(&["status", "--porcelain", "--untracked-files=no"])?.is_empty()
            && self.git(&["diff", "--cached", "--name-only"])?.is_empty()
        {
            return Ok(None);
        }
        let message =
            format!("run {run_id}: {outcome}\n\nRun-Id: {run_id}\nGoal: {goal}\nOutcome: {outcome}");
        self.git(&["commit", "-q", "-m", &message])?;
        Ok(Some(self.git(&["rev-parse", "HEAD"])?))
    }

    async fn read_events(&self) -> Result<Vec<Event>, StateError> {
        let path = self.events_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = tokio::fs::read_to_string(&path).await?;
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(StateError::from))
            .collect()
    }
}

#[async_trait]
impl EventStore for GitEventStore {
    async fn append(&self, events: Vec<Event>) -> Result<Vec<EventId>, StateError> {
        self.acquire_lease()?;
        let ids = events.iter().map(|event| event.id.clone()).collect();
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.events_path())
            .await?;
        for event in events {
            let mut line = serde_json::to_string(&event)?;
            line.push('\n');
            file.write_all(line.as_bytes()).await?;
        }
        file.flush().await?;
        file.sync_all().await?; // durability: the tail survives a crash mid-run
        Ok(ids)
    }

    async fn scan(&self) -> Result<Vec<Event>, StateError> {
        self.read_events().await
    }

    async fn scan_after(&self, event_id: Option<EventId>) -> Result<Vec<Event>, StateError> {
        scan_after_events(self.read_events().await?, event_id)
    }
}

/// Convenience for examples/tests: initialize a git repo suitable as an agent
/// repo (git init + minimal AGENT.md/identity), returning the store.
pub fn init_agent_repo(
    root: impl AsRef<Path>,
    agent_id: &str,
    worker_id: &str,
) -> Result<GitEventStore, StateError> {
    let root = root.as_ref();
    std::fs::create_dir_all(root)?;
    let run = |args: &[&str]| -> Result<(), StateError> {
        let ok = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|err| StateError::Store(format!("git: {err}")))?
            .status
            .success();
        if ok {
            Ok(())
        } else {
            Err(StateError::Store(format!("git {args:?} failed")))
        }
    };
    if !root.join(".git").exists() {
        run(&["init", "-q"])?;
    }
    let identity = root.join("identity");
    std::fs::create_dir_all(&identity)?;
    let identity_md = identity.join("IDENTITY.md");
    if !identity_md.exists() {
        std::fs::write(identity_md, format!("# Identity\n\nI am {agent_id}.\n"))?;
    }
    let agent_md = root.join("AGENT.md");
    if !agent_md.exists() {
        std::fs::write(
            agent_md,
            format!("# AGENT\n\n```yaml\nspec_version: 1\nagent_id: {agent_id}\n```\n"),
        )?;
    }
    GitEventStore::open(root, worker_id)
}
