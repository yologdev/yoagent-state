use serde_json::json;
use yoagent_state::{
    ActorRef, BehaviorContext, BehaviorId, Event, EventPattern, FnBehavior, Goal, GoalId,
    GoalStatus, MemoryEventStore, NodeId, ObjectType, Pack, PackId, Policy, PolicyAction, PolicyId,
    RelationType, StateError, StateOp, Task, TaskId, TaskStatus, YoAgentRuntime, YoAgentState,
    diff_graphs,
};

#[tokio::test]
async fn records_goal_task_failure_patch_lineage() {
    let actor = ActorRef::agent("test");
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();

    state
        .record_goal(Goal {
            status: GoalStatus::InProgress,
            ..Goal::new(
                GoalId::new("goal_1"),
                "Improve retry reliability",
                "Timeout retry should preserve state",
                actor.clone(),
            )
        })
        .await
        .unwrap();
    state
        .record_task(Task {
            id: TaskId::new("task_1"),
            title: "Investigate retry timeout".to_string(),
            summary: "Find why retry state resets".to_string(),
            status: TaskStatus::InProgress,
            goal: Some(GoalId::new("goal_1")),
            created_by: actor.clone(),
            metadata: json!({}),
        })
        .await
        .unwrap();
    state
        .record_failure(
            actor.clone(),
            NodeId::new("failure_1"),
            "retry timeout loses state",
            "attempt count resets",
        )
        .await
        .unwrap();
    state
        .link(
            actor,
            NodeId::new("failure_1"),
            yoagent_state::REL_BLOCKS,
            NodeId::new("goal_1"),
        )
        .await
        .unwrap();

    let lineage = state.lineage(NodeId::new("goal_1")).await;
    assert_eq!(lineage.root.unwrap().kind, "goal");
    assert!(lineage.incoming.iter().any(|rel| rel.rel == "serves"));
    assert!(lineage.incoming.iter().any(|rel| rel.rel == "blocks"));
}

#[tokio::test]
async fn typed_pack_validates_nodes_and_relations() {
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();
    let mut runtime = YoAgentRuntime::new(state);
    runtime.register_pack(
        Pack::new(PackId::new("pack_1"), "lineage", "0.1.0")
            .add_object_type(ObjectType::new("goal").require("title"))
            .add_object_type(ObjectType::new("task").require("title"))
            .add_relation_type(
                RelationType::new("serves")
                    .from_kind("task")
                    .to_kind("goal"),
            ),
    );

    let bad = runtime
        .create_typed_node(
            ActorRef::agent("test"),
            NodeId::new("goal_bad"),
            "goal",
            json!({}),
        )
        .await;
    assert!(matches!(bad, Err(StateError::Validation(_))));

    runtime
        .create_typed_node(
            ActorRef::agent("test"),
            NodeId::new("goal_1"),
            "goal",
            json!({ "title": "Improve retry" }),
        )
        .await
        .unwrap();
    runtime
        .create_typed_node(
            ActorRef::agent("test"),
            NodeId::new("task_1"),
            "task",
            json!({ "title": "Investigate retry" }),
        )
        .await
        .unwrap();
    runtime
        .create_typed_relation(
            ActorRef::agent("test"),
            NodeId::new("task_1"),
            "serves",
            NodeId::new("goal_1"),
            json!({}),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn policy_gate_creates_approval_request() {
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();
    let mut runtime = YoAgentRuntime::new(state.clone());
    runtime.register_policy(Policy::require_approval(
        PolicyId::new("policy_1"),
        "Node creation requires approval",
        PolicyAction::CreateNode,
    ));

    let result = runtime
        .create_typed_node(
            ActorRef::agent("test"),
            NodeId::new("node_1"),
            "memory",
            json!({ "title": "sensitive" }),
        )
        .await;
    assert!(matches!(result, Err(StateError::PolicyDenied(_))));
    assert_eq!(
        state
            .graph()
            .await
            .nodes
            .values()
            .filter(|node| node.kind == "approval_request")
            .count(),
        1
    );
}

#[tokio::test]
async fn behavior_subscription_applies_state_ops() {
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();
    let mut runtime = YoAgentRuntime::new(state.clone());
    runtime.register_behavior(FnBehavior::new(
        BehaviorId::new("behavior_1"),
        EventPattern::Kind("failure.observed".to_string()),
        |_ctx: BehaviorContext, _event: Event| async move {
            Ok(vec![StateOp::CreateNode {
                id: NodeId::new("task_from_behavior"),
                kind: "task".to_string(),
                props: json!({ "title": "Investigate failure" }),
            }])
        },
    ));

    runtime
        .emit_event(Event::new(
            ActorRef::agent("test"),
            "failure.observed",
            json!({ "title": "retry failed" }),
        ))
        .await
        .unwrap();
    assert!(
        state
            .graph()
            .await
            .get_node(&NodeId::new("task_from_behavior"))
            .is_some()
    );
}

#[tokio::test]
async fn fork_and_diff_compare_projected_graphs() {
    let state = YoAgentState::load(MemoryEventStore::new()).await.unwrap();
    let first = state
        .apply_ops(
            ActorRef::agent("test"),
            vec![StateOp::CreateNode {
                id: NodeId::new("goal_1"),
                kind: "goal".to_string(),
                props: json!({ "title": "Improve retry" }),
            }],
        )
        .await
        .unwrap();
    state
        .apply_ops(
            ActorRef::agent("test"),
            vec![StateOp::CreateNode {
                id: NodeId::new("task_1"),
                kind: "task".to_string(),
                props: json!({ "title": "Investigate retry" }),
            }],
        )
        .await
        .unwrap();

    let fork = state
        .fork_at_event(yoagent_state::ForkId::new("fork_1"), Some(first))
        .await
        .unwrap();
    let diff = diff_graphs(&fork.graph, &state.graph().await);
    assert_eq!(fork.graph.nodes.len(), 1);
    assert_eq!(diff.added_nodes, vec!["task_1".to_string()]);
}
