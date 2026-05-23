# Getting Started

Run the tests:

```bash
cargo test
```

Run the main lineage demo:

```bash
cargo run --example patch_eval_decision
```

Initialize a local JSONL event log:

```bash
cargo run --bin yoagent-state -- init
```

Inspect the graph:

```bash
cargo run --bin yoagent-state -- graph
```

Use a custom log path:

```bash
YOAGENT_STATE_EVENTS=.yoyo/state/events.jsonl cargo run --bin yoagent-state -- events
```

The default local event log path is `.yoagent-state/events.jsonl`.
