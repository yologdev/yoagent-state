//! Git-backed event store satisfying the GASP store contract (the rules
//! restated inline here are authoritative for this crate; the full spec lives
//! in the `gasp` repo's SPEC.md):
//!
//! 1. **Durable append, boundary commit.** Every `append` opens the log in
//!    append mode, writes, flushes, and fsyncs (plus a parent-directory fsync
//!    when the log file is first created), so a crash loses at most the batch
//!    being written. A crash mid-write can leave a torn final line; `scan`
//!    reports it with a recovery hint rather than reading past it.
//!    `commit_run` makes the one closing git commit per run boundary.
//! 2. **The lease lives inside the append path.** `append` takes or renews the
//!    cross-process lease at `.agent/lease` (local-only, gitignored) before
//!    writing; acquisition races on the lease file are decided by an atomic
//!    exclusive create. `worker_id` MUST be unique per worker — two processes
//!    sharing a worker id are treated as the same writer. Residual caveat:
//!    stealing an *expired* lease has a narrow three-process race window, and
//!    file-based leases are advisory on network filesystems.

use crate::store::scan_after_events;
use crate::{Event, EventId, EventStore, GoalId, RunId, StateError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
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

impl Lease {
    fn take(worker_id: &str, ttl: Duration) -> Self {
        Self {
            worker_id: worker_id.to_string(),
            pid: std::process::id(),
            expires_at_ms: crate::now_ms() + ttl.as_millis() as i64,
        }
    }

    /// Absent file -> `Ok(None)`. Corrupt or unreadable file -> `Err`: a lease
    /// we cannot read is NOT a lease we may steal.
    fn read(path: &Path) -> Result<Option<Lease>, StateError> {
        match std::fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str(&raw).map(Some).map_err(|err| {
                StateError::Store(format!(
                    "lease file {} is corrupt ({err}); inspect or remove it manually",
                    path.display()
                ))
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(StateError::Store(format!(
                "cannot read lease {}: {err}; refusing to write without proof of exclusivity",
                path.display()
            ))),
        }
    }

    fn is_held_by(&self, worker_id: &str) -> bool {
        self.worker_id == worker_id
    }

    fn is_live(&self, now_ms: i64) -> bool {
        self.expires_at_ms > now_ms
    }
}

#[derive(Debug, Clone)]
pub struct GitEventStore {
    repo_root: PathBuf,
    worker_id: String,
    lease_ttl: Duration,
    /// Serializes appends across tasks/clones in this process; the lease
    /// serializes across processes.
    append_lock: Arc<tokio::sync::Mutex<()>>,
}

impl GitEventStore {
    /// Open a GASP agent repo. `repo_root` must be inside a git work tree;
    /// `worker_id` names this writer in the lease and must be unique per
    /// worker.
    pub fn open(
        repo_root: impl Into<PathBuf>,
        worker_id: impl Into<String>,
    ) -> Result<Self, StateError> {
        let store = Self {
            repo_root: repo_root.into(),
            worker_id: worker_id.into(),
            lease_ttl: Duration::from_secs(600),
            append_lock: Arc::new(tokio::sync::Mutex::new(())),
        };
        if store.worker_id.trim().is_empty() {
            return Err(StateError::Validation("worker_id must be non-empty".into()));
        }
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
        self.repo_root.join(DEFAULT_EVENTS_PATH)
    }

    fn lease_path(&self) -> PathBuf {
        self.repo_root.join(LEASE_PATH)
    }

    /// The control plane is local-only: make sure the repo ignores it. Only a
    /// missing .gitignore reads as empty — any other read error propagates so
    /// a transient failure can never truncate user content.
    fn ensure_gitignore(&self) -> Result<(), StateError> {
        let path = self.repo_root.join(".gitignore");
        let existing = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err.into()),
        };
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
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Take or renew the single-writer lease. Mutual exclusion is decided by
    /// an atomic exclusive create of the lease file; renewal (same worker)
    /// replaces it via tmp+rename. Private by design: the only call sites are
    /// `append` and `commit_run` — keep it that way so the check can't be
    /// skipped.
    fn acquire_lease(&self) -> Result<(), StateError> {
        let path = self.lease_path();
        for _ in 0..3 {
            let lease = Lease::take(&self.worker_id, self.lease_ttl);
            let payload = serde_json::to_vec(&lease)?;
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    file.write_all(&payload)?;
                    file.sync_all()?;
                    return Ok(());
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let Some(existing) = Lease::read(&path)? else {
                        continue; // vanished between create and read; retry
                    };
                    if existing.is_held_by(&self.worker_id) {
                        // Renewal: we own it; atomic replace.
                        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
                        std::fs::write(&tmp, &payload)?;
                        std::fs::rename(&tmp, &path)?;
                        return Ok(());
                    }
                    if existing.is_live(crate::now_ms()) {
                        return Err(StateError::Store(format!(
                            "lease held by worker `{}` (pid {}) until {}",
                            existing.worker_id, existing.pid, existing.expires_at_ms
                        )));
                    }
                    // Expired lease of another worker: steal by atomically
                    // renaming it away, then verify we didn't grab a lease
                    // that was refreshed between our read and the rename.
                    let steal = path.with_extension(format!("steal-{}", std::process::id()));
                    match std::fs::rename(&path, &steal) {
                        Ok(()) => {
                            if let Some(stolen) = Lease::read(&steal)? {
                                if stolen.is_live(crate::now_ms())
                                    && !stolen.is_held_by(&self.worker_id)
                                {
                                    std::fs::rename(&steal, &path)?;
                                    return Err(StateError::Store(format!(
                                        "lease held by worker `{}` (pid {}) until {}",
                                        stolen.worker_id, stolen.pid, stolen.expires_at_ms
                                    )));
                                }
                            }
                            let _ = std::fs::remove_file(&steal);
                            continue; // lease slot is free; retry the create
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(err) => return Err(err.into()),
                    }
                }
                Err(err) => return Err(err.into()),
            }
        }
        Err(StateError::Store(
            "could not acquire lease after repeated contention; retry".into(),
        ))
    }

    /// Release the lease if this worker holds it. A corrupt or unreadable
    /// lease file is an error — silently leaving it in place would deadlock
    /// every future `acquire_lease`.
    pub fn release_lease(&self) -> Result<(), StateError> {
        let path = self.lease_path();
        match Lease::read(&path)? {
            Some(lease) if lease.is_held_by(&self.worker_id) => {
                std::fs::remove_file(&path)?;
                Ok(())
            }
            _ => Ok(()),
        }
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
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    /// The run-boundary commit: stage the log plus any projection paths and
    /// commit **only those paths** with the GASP trailers (content staged by
    /// anyone else stays staged, untouched). Requires the lease. Returns the
    /// commit sha, or `Ok(None)` when nothing in the requested paths changed —
    /// unrelated dirty files in the worktree do not affect the outcome.
    pub fn commit_run(
        &self,
        run_id: &RunId,
        goal: &GoalId,
        outcome: &str,
        extra_paths: &[&str],
    ) -> Result<Option<String>, StateError> {
        for (name, value) in [
            ("run_id", run_id.as_str()),
            ("goal", goal.as_str()),
            ("outcome", outcome),
        ] {
            if value.contains('\n') {
                return Err(StateError::Validation(format!(
                    "{name} must not contain newlines (would forge commit trailers)"
                )));
            }
        }
        self.acquire_lease()?;
        if !self.events_path().exists() {
            let tracked = self.git(&["ls-files", "--", DEFAULT_EVENTS_PATH])?;
            if tracked.trim().is_empty() {
                return Ok(None); // no events ever recorded: nothing to ship
            }
            return Err(StateError::Store(format!(
                "{DEFAULT_EVENTS_PATH} is tracked but missing from the worktree — refusing to commit its deletion as a run boundary"
            )));
        }

        let mut paths: Vec<&str> = vec![DEFAULT_EVENTS_PATH];
        paths.extend(extra_paths);

        let mut add_args = vec!["add", "--"];
        add_args.extend(&paths);
        self.git(&add_args)?;

        let mut status_args = vec!["status", "--porcelain", "--"];
        status_args.extend(&paths);
        if self.git(&status_args)?.trim().is_empty() {
            return Ok(None);
        }

        let message = format!(
            "run {run_id}: {outcome}\n\nRun-Id: {run_id}\nGoal: {goal}\nOutcome: {outcome}"
        );
        let mut commit_args = vec!["commit", "-q", "-m", &message, "--"];
        commit_args.extend(&paths);
        self.git(&commit_args)?;
        Ok(Some(self.git(&["rev-parse", "HEAD"])?.trim().to_string()))
    }

    async fn read_events(&self) -> Result<Vec<Event>, StateError> {
        let path = self.events_path();
        let raw = match tokio::fs::read_to_string(&path).await {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        let numbered: Vec<(usize, &str)> = raw
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .collect();
        let last = numbered.last().map(|(n, _)| *n);
        let mut events = Vec::with_capacity(numbered.len());
        for (n, line) in numbered {
            match serde_json::from_str(line) {
                Ok(event) => events.push(event),
                Err(err) => {
                    let hint = if Some(n) == last && err.is_eof() {
                        " (torn final line — likely a crash mid-append; truncate the last line to recover)"
                    } else {
                        ""
                    };
                    return Err(StateError::Store(format!(
                        "{}:{}: corrupt event: {err}{hint}",
                        path.display(),
                        n + 1
                    )));
                }
            }
        }
        Ok(events)
    }
}

#[async_trait]
impl EventStore for GitEventStore {
    async fn append(&self, events: Vec<Event>) -> Result<Vec<EventId>, StateError> {
        let _guard = self.append_lock.lock().await;
        self.acquire_lease()?;
        let path = self.events_path();
        let existed = path.exists();
        let ids = events.iter().map(|event| event.id.clone()).collect();
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        for event in events {
            let mut line = serde_json::to_string(&event)?;
            line.push('\n');
            file.write_all(line.as_bytes()).await?;
        }
        file.flush().await?;
        file.sync_all().await?; // durability: the tail survives a crash mid-run
        if !existed {
            // First append created the file: fsync the directory so the entry
            // itself is durable.
            if let Some(parent) = path.parent() {
                std::fs::File::open(parent)?.sync_all()?;
            }
        }
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
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|err| StateError::Store(format!("git: {err}")))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(StateError::Store(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )))
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
