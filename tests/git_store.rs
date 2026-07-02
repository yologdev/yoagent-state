use std::path::Path;
use std::process::Command;
use std::time::Duration;
use yoagent_state::{
    init_agent_repo, ActorRef, EventStore, GitEventStore, Goal, GoalId, NodeId, RunId,
    YoAgentState,
};

fn git_env(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Commit the scaffold first so boundary commits contain only the events log
/// (the append-only prefix check relies on this). Sets repo-local git identity
/// because commit_run's internal git uses ambient config.
fn setup(dir: &Path) -> GitEventStore {
    let store = init_agent_repo(dir, "test-agent", "worker-a").unwrap();
    git_env(dir, &["config", "user.name", "t"]);
    git_env(dir, &["config", "user.email", "t@t"]);
    git_env(dir, &["add", "-A"]);
    git_env(dir, &["commit", "-qm", "init agent repo"]);
    store
}

fn goal(id: &str) -> Goal {
    Goal::new(GoalId::new(id), "title", "summary", ActorRef::agent("t"))
}

#[tokio::test]
async fn durable_append_survives_without_commit() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());

    let state = YoAgentState::load(store).await.unwrap();
    state.record_goal(goal("goal_x")).await.unwrap();
    drop(state); // "crash": no commit, no release

    // A fresh load from the same repo sees the fsynced tail.
    let store = GitEventStore::open(dir.path(), "worker-a").unwrap();
    let events = store.scan().await.unwrap();
    assert_eq!(events.len(), 2, "goal.created + state.ops_applied");
    let state = YoAgentState::load(store).await.unwrap();
    assert!(state.get_node(NodeId::new("goal_x")).await.is_some());
}

#[tokio::test]
async fn torn_final_line_yields_actionable_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let state = YoAgentState::load(store).await.unwrap();
    state.record_goal(goal("goal_x")).await.unwrap();

    // simulate a crash mid-write: a truncated final JSON line
    let log = dir.path().join("state/events.jsonl");
    let mut content = std::fs::read_to_string(&log).unwrap();
    content.push_str("{\"id\":\"event_torn");
    std::fs::write(&log, content).unwrap();

    let store = GitEventStore::open(dir.path(), "worker-a").unwrap();
    let err = store.scan().await.unwrap_err().to_string();
    assert!(err.contains("torn final line"), "unhelpful error: {err}");
    assert!(err.contains("events.jsonl:3"), "missing location: {err}");
}

#[tokio::test]
async fn second_writer_is_refused_until_release() {
    let dir = tempfile::tempdir().unwrap();
    let store_a = setup(dir.path()).with_lease_ttl(Duration::from_secs(60));
    let store_b = GitEventStore::open(dir.path(), "worker-b")
        .unwrap()
        .with_lease_ttl(Duration::from_secs(60));

    let state_a = YoAgentState::load(store_a.clone()).await.unwrap();
    let actor = ActorRef::agent("t");
    state_a.record_goal(goal("goal_a")).await.unwrap();
    let len_before = store_a.scan().await.unwrap().len();

    // worker-b must not append while worker-a holds the lease...
    let refused = store_b
        .append(vec![yoagent_state::Event::new(
            actor.clone(),
            "goal.created",
            serde_json::json!({"id": "goal_b"}),
        )])
        .await;
    assert!(refused.is_err(), "second writer slipped past the lease");
    // ...and a refused writer must not have written anything
    assert_eq!(store_a.scan().await.unwrap().len(), len_before);

    // a non-holder's release must not evict the holder
    store_b.release_lease().unwrap();
    assert!(store_b
        .append(vec![yoagent_state::Event::new(
            actor.clone(),
            "goal.created",
            serde_json::json!({"id": "goal_b"}),
        )])
        .await
        .is_err());

    store_a.release_lease().unwrap();
    assert!(store_b
        .append(vec![yoagent_state::Event::new(
            actor,
            "goal.created",
            serde_json::json!({"id": "goal_b"}),
        )])
        .await
        .is_ok());
}

#[tokio::test]
async fn expired_lease_is_taken_over() {
    let dir = tempfile::tempdir().unwrap();
    let store_a = setup(dir.path()).with_lease_ttl(Duration::from_millis(50));
    let store_b = GitEventStore::open(dir.path(), "worker-b").unwrap();
    let actor = ActorRef::agent("t");
    let event = || yoagent_state::Event::new(actor.clone(), "goal.created", serde_json::json!({}));

    store_a.append(vec![event()]).await.unwrap();
    assert!(store_b.append(vec![event()]).await.is_err(), "lease live");

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        store_b.append(vec![event()]).await.is_ok(),
        "expired lease must be takeable"
    );
    // and after the takeover, the original holder is now the refused one
    assert!(store_a.append(vec![event()]).await.is_err());
}

#[tokio::test]
async fn corrupt_lease_refuses_instead_of_stealing() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    std::fs::write(dir.path().join(".agent/lease"), "not json").unwrap();

    let actor = ActorRef::agent("t");
    let err = store
        .append(vec![yoagent_state::Event::new(
            actor,
            "goal.created",
            serde_json::json!({}),
        )])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("corrupt"), "expected corrupt-lease refusal: {err}");
    // release reports the same problem instead of silently stranding it
    assert!(store.release_lease().is_err());
}

#[tokio::test]
async fn boundary_commit_carries_trailers_and_is_append_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let state = YoAgentState::load(store.clone()).await.unwrap();

    state.record_goal(goal("goal_x")).await.unwrap();
    let first = store
        .commit_run(&RunId::new("run_1"), &GoalId::new("goal_x"), "promoted", &[])
        .unwrap()
        .expect("first run commits");

    state.record_goal(goal("goal_y")).await.unwrap();
    let second = store
        .commit_run(&RunId::new("run_2"), &GoalId::new("goal_y"), "rejected", &[])
        .unwrap()
        .expect("second run commits");
    assert_ne!(first, second);

    // nothing new -> no empty commit
    assert!(store
        .commit_run(&RunId::new("run_3"), &GoalId::new("-"), "-", &[])
        .unwrap()
        .is_none());

    let message = git_env(dir.path(), &["log", "-1", "--format=%B", &first]);
    assert!(message.contains("Run-Id: run_1"));
    assert!(message.contains("Goal: goal_x"));
    assert!(message.contains("Outcome: promoted"));

    // append-only across the two commits: v1 is a prefix of v2
    let v1 = git_env(dir.path(), &["show", &format!("{first}:state/events.jsonl")]);
    let v2 = git_env(dir.path(), &["show", &format!("{second}:state/events.jsonl")]);
    assert!(v2.starts_with(&v1), "boundary commits must be append-only");

    // the lease never ships: .gitignore covers it and git sees it as ignored
    let ignored = git_env(dir.path(), &["check-ignore", ".agent/lease"]);
    assert_eq!(ignored, ".agent/lease");
}

#[tokio::test]
async fn commit_run_ignores_unrelated_dirty_and_staged_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let state = YoAgentState::load(store.clone()).await.unwrap();

    // idle run + unrelated dirty tracked file -> Ok(None), not Err
    std::fs::write(dir.path().join("AGENT.md"), "# AGENT (edited mid-work)\n").unwrap();
    assert!(store
        .commit_run(&RunId::new("run_1"), &GoalId::new("g"), "idle", &[])
        .unwrap()
        .is_none());

    // a real run commits ONLY the requested paths: neither the dirty file nor
    // externally staged content gets swept into the boundary commit
    std::fs::write(dir.path().join("unrelated.txt"), "staged by a human\n").unwrap();
    git_env(dir.path(), &["add", "unrelated.txt"]);
    state.record_goal(goal("goal_x")).await.unwrap();
    let sha = store
        .commit_run(&RunId::new("run_2"), &GoalId::new("goal_x"), "promoted", &[])
        .unwrap()
        .expect("run with new events commits");
    let files = git_env(dir.path(), &["show", "--name-only", "--format=", &sha]);
    assert_eq!(files.trim(), "state/events.jsonl", "swept in: {files}");

    // trailer forgery is rejected
    assert!(store
        .commit_run(&RunId::new("run_3"), &GoalId::new("g"), "done\nOutcome: forged", &[])
        .is_err());
}
