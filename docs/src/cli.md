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
yoagent-state patch list
yoagent-state patch show <id>
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

Show one patch:

```bash
cargo run --bin yoagent-state -- patch show patch_42
```

## Current limitation

The CLI is inspection-first. It does not yet create complete patch/eval/decision flows from shell commands. For now, create state through the Rust API and inspect it through the CLI.
