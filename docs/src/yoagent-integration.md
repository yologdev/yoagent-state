# yoagent Integration

`yoagent-state` should be optional. The agent loop emits events to a sink; the state layer records them and builds lineage.

The boundary is:

```text
yoagent = execution
yoagent-state = state, lineage, patches, evals, decisions
```

## Adapter shape

The crate provides `YoAgentStateSink` and `YoAgentStateAdapter`.

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

## Run lifecycle

A typical run emits:

```text
run.started
model.called
model.finished
tool.called
tool.finished
run.finished
```

Those events stay historical unless converted into state ops. This keeps the graph projection focused on durable semantic state.

## Example

Run:

```bash
cargo run --example yoagent_integration
```

The example records a short run with model and tool events, then prints the event log as JSON.

## Integration advice

- Keep state recording optional.
- Attach selected tool outputs as artifacts instead of dumping everything into graph nodes.
- Use causation and correlation IDs when connecting model/tool events to a run.
- Convert only meaningful facts into state ops.

The goal is continuity, not a heavier agent runtime.
