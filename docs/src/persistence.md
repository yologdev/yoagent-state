# Persistence

`yoagent-state` ships with two v0 stores:

- `MemoryEventStore`
- `JsonlEventStore`

The JSONL store writes one serialized event per line. This keeps persistence inspectable and easy to replay.

Load and replay are the same operation:

```rust
let state = YoAgentState::load(JsonlEventStore::new(".yoagent-state/events.jsonl")).await?;
```

On startup, all events are scanned and replayed into the graph projection.

SQLite is intentionally left for later. JSONL is enough to prove the state model and makes local debugging simple.
