use std::path::Path;
use std::process::Command;
use std::time::Duration;
use yoagent_state::{
    init_agent_repo, ActorRef, EvalResult, EvalStatus, EventStore, GitEventStore, Goal, GoalId,
    NodeId, YoAgentState,
};

fn git_env(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn setup(dir: &Path) -> GitEventStore {
    let store = init_agent_repo(dir, "test-agent", "worker-a").unwrap();
    // commit_run needs an identity for the boundary commit to be meaningful,
    // and git needs a configured author.
    git_env(dir, &["add", "-A"]);
    git_env(dir, &["commit", "-qm", "init agent repo"]);
    store
}

#[tokio::test]
async fn durable_append_survives_without_commit() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());

    let state = YoAgentState::load(store).await.unwrap();
    let actor = ActorRef::agent("t");
    state
        .record_goal(Goal::new(GoalId::new("goal_x"), "title", "sum", actor))
        .await
        .unwrap();
    drop(state); // "crash": no commit, no release

    // A fresh load from the same repo sees the fsynced tail.
    let store = GitEventStore::open(dir.path(), "worker-a").unwrap();
    let events = store.scan().await.unwrap();
    assert_eq!(events.len(), 2, "goal.created + state.ops_applied");
    let state = YoAgentState::load(store).await.unwrap();
    assert!(state.get_node(NodeId::new("goal_x")).await.is_some());
}

#[tokio::test]
async fn pairing_rule_causation_is_threaded() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let state = YoAgentState::load(store).await.unwrap();
    let actor = ActorRef::agent("t");
    state
        .record_goal(Goal::new(GoalId::new("goal_x"), "title", "sum", actor.clone()))
        .await
        .unwrap();
    state
        .record_eval(
            actor,
            EvalResult {
                id: yoagent_state::EvalId::new("eval_x"),
                command: "cargo test".into(),
                status: EvalStatus::Passed,
                score: Some(1.0),
                metadata: serde_json::json!({}),
            },
            None,
        )
        .await
        .unwrap();

    let events = state.store().scan().await.unwrap();
    assert_eq!(events.len(), 4);
    for pair in events.chunks(2) {
        let (domain, ops) = (&pair[0], &pair[1]);
        assert_eq!(ops.kind, "state.ops_applied");
        assert_eq!(
            ops.causation_id.as_ref(),
            Some(&domain.id),
            "ops event must be caused by its domain event ({})",
            domain.kind
        );
    }
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
    state_a
        .record_goal(Goal::new(GoalId::new("goal_a"), "a", "a", actor.clone()))
        .await
        .unwrap();

    // worker-b must not be able to append while worker-a holds the lease
    let err = state_a; // keep state alive; try b directly at store level
    let refused = store_b
        .append(vec![yoagent_state::Event::new(
            actor.clone(),
            "goal.created",
            serde_json::json!({"id": "goal_b"}),
        )])
        .await;
    assert!(refused.is_err(), "second writer slipped past the lease");
    drop(err);

    store_a.release_lease().unwrap();
    let allowed = store_b
        .append(vec![yoagent_state::Event::new(
            actor,
            "goal.created",
            serde_json::json!({"id": "goal_b"}),
        )])
        .await;
    assert!(allowed.is_ok(), "released lease should be takeable");
}

#[tokio::test]
async fn boundary_commit_carries_trailers_and_is_append_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let state = YoAgentState::load(store.clone()).await.unwrap();
    let actor = ActorRef::agent("t");

    state
        .record_goal(Goal::new(GoalId::new("goal_x"), "t", "s", actor.clone()))
        .await
        .unwrap();
    let first = store
        .commit_run("run_1", "goal_x", "promoted", &[])
        .unwrap()
        .expect("first run commits");

    state
        .record_goal(Goal::new(GoalId::new("goal_y"), "t", "s", actor))
        .await
        .unwrap();
    let second = store
        .commit_run("run_2", "goal_y", "rejected", &[])
        .unwrap()
        .expect("second run commits");
    assert_ne!(first, second);

    // nothing new -> no empty commit
    assert!(store.commit_run("run_3", "-", "-", &[]).unwrap().is_none());

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
