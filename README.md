# yoagent-state

A lightweight Rust continuity layer for long-running agents.

`yoagent-state` records durable state and lineage for agent work:

- append-only events
- graph projection
- patches
- evals
- decisions
- artifacts
- project references

It is designed to sit on top of `yoagent`.

```text
yoagent executes.
yoagent-state remembers.
yoyo evolve improves.
```

## Motto

Simple but effective.

## What it is

`yoagent-state` stores agent-facing meaning: what failed, what the agent believed, which patch addressed the failure, what eval tested it, and which decision promoted or rejected it.

Git and the filesystem still own concrete project state. This crate stores references to commits, diffs, files, logs, and eval outputs.

## Non-goals

- not a workflow engine
- not a graph database
- not a replacement for Git
- not a full project database
- not a universal agent framework

## Quick Start

```bash
cargo test
cargo run --example patch_eval_decision
```

Create a persisted event log:

```bash
cargo run --bin yoagent-state -- init
cargo run --bin yoagent-state -- graph
```

Use a custom event log path:

```bash
YOAGENT_STATE_EVENTS=.yoyo/state/events.jsonl cargo run --bin yoagent-state -- events
```

## Minimal Example

```rust
use serde_json::json;
use yoagent_state::{ActorRef, MemoryEventStore, NodeId, StateOp, YoAgentState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let failure = NodeId::new("failure_1");

    state.apply_ops(
        ActorRef::agent("demo"),
        vec![StateOp::CreateNode {
            id: failure.clone(),
            kind: "failure".to_string(),
            props: json!({ "title": "retry state lost after timeout" }),
        }],
    ).await?;

    print!("{}", state.lineage(failure).await.to_markdown());
    Ok(())
}
```

## Documentation

The user guide is an mdBook under `docs/`.

Hosted docs:

```text
https://yologdev.github.io/yoagent-state/
```

Run the docs locally:

```bash
mdbook serve docs
```

If `mdbook` is not installed:

```bash
cargo install mdbook
```

If Cargo's binary directory is not on your `PATH`, run it directly:

```bash
~/.cargo/bin/mdbook serve docs
```

GitHub Pages is deployed by `.github/workflows/docs.yml`. In the GitHub repo settings, Pages source should be set to **GitHub Actions**.

## Roadmap

The future plan is tracked in [ROADMAP.md](./ROADMAP.md) and mirrored in the mdBook guide.

## Acknowledgments

The core idea for `yoagent-state` comes from [Yohei Nakajima](https://github.com/yoheinakajima) and his [ActiveGraph](https://github.com/yoheinakajima/activegraph) work. This project is an independent Rust implementation inspired by that idea, intentionally kept smaller in scope for `yoagent` and `yoyo evolve`. See [ACKNOWLEDGMENTS.md](./ACKNOWLEDGMENTS.md).

## License

Licensed under the [MIT license](./LICENSE).
