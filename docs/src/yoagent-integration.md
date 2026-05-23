# yoagent Integration

`yoagent-state` should be optional. The agent loop can emit state events through `YoAgentStateSink`.

The adapter records:

- run started and finished
- model called and finished
- tool called and finished
- failure observed when a tool finishes unsuccessfully

Minimal setup:

```rust
let state = YoAgentState::load(MemoryEventStore::new()).await?;
let sink = YoAgentStateAdapter::new(state, ActorRef::agent("yoagent"));
```

The integration boundary is:

```text
yoagent = execution
yoagent-state = state, lineage, patches, evals, decisions
```
