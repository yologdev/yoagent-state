# API Guide

Create an in-memory state:

```rust
let state = YoAgentState::load(MemoryEventStore::new()).await?;
```

Create a persisted state:

```rust
let state = YoAgentState::load(JsonlEventStore::new(".yoagent-state/events.jsonl")).await?;
```

Apply graph operations:

```rust
state.apply_ops(actor, vec![StateOp::CreateNode {
    id: NodeId::new("failure_1"),
    kind: "failure".to_string(),
    props: serde_json::json!({ "title": "retry failed" }),
}]).await?;
```

Propose a patch:

```rust
let patch = StatePatch::new(
    PatchId::new("patch_1"),
    "Persist retry state",
    "Keep attempt count across timeouts",
    ActorRef::agent("yoyo-evolve"),
);
state.propose_patch(patch).await?;
```

Query lineage:

```rust
let lineage = state.lineage(NodeId::new("patch_1")).await;
println!("{}", lineage.to_markdown());
```
