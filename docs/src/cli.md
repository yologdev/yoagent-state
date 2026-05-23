# CLI Guide

The CLI is intentionally small. It exists to inspect local event logs and graph projections.

## Commands

```bash
yoagent-state init
yoagent-state events
yoagent-state graph
yoagent-state node <id>
yoagent-state lineage <id>
yoagent-state lineage <id> --markdown
yoagent-state goal create <id> <title> [summary]
yoagent-state goal list
yoagent-state goal show <id>
yoagent-state goal status <id> <open|in-progress|satisfied|abandoned|blocked|stale>
yoagent-state patch list
yoagent-state patch show <id>
yoagent-state patch promote <id>
yoagent-state fork create <id> [event-id]
yoagent-state replay
```

When running from source, prefix commands with Cargo:

```bash
cargo run --bin yoagent-state -- graph
```

## Event log path

The default event log is:

```text
.yoagent-state/events.jsonl
```

Set `YOAGENT_STATE_EVENTS` to use another path:

```bash
YOAGENT_STATE_EVENTS=.yoyo/state/events.jsonl cargo run --bin yoagent-state -- events
```

## Common local flow

Initialize:

```bash
cargo run --bin yoagent-state -- init
```

Inspect raw events:

```bash
cargo run --bin yoagent-state -- events
```

Inspect projected graph:

```bash
cargo run --bin yoagent-state -- graph
```

Print lineage:

```bash
cargo run --bin yoagent-state -- lineage patch_42 --markdown
```

List patches:

```bash
cargo run --bin yoagent-state -- patch list
```

Create a goal:

```bash
cargo run --bin yoagent-state -- goal create goal_retry "Make retry reliable"
```

Update goal status:

```bash
cargo run --bin yoagent-state -- goal status goal_retry in-progress
```

Create a fork at an event:

```bash
cargo run --bin yoagent-state -- fork create fork_before_patch event_123
```

Show one patch:

```bash
cargo run --bin yoagent-state -- patch show patch_42
```

## Current limitation

The CLI is still intentionally small. Use the Rust API for full behavior/policy/pack flows and use the CLI for local inspection and simple goal/patch/fork operations.
