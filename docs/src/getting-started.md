# Quick Start

This page gets you from a fresh clone to a working lineage report.

## Prerequisites

You need a Rust toolchain with Cargo.

Check:

```bash
cargo --version
```

## Clone and run the main demo

```bash
git clone https://github.com/yologdev/yoagent-state.git
cd yoagent-state
cargo run --example patch_eval_decision
```

Expected shape of the output:

```text
# Persist retry state across timeout

- id: patch_42
- kind: patch
- status: Promoted

## Artifacts
- git.diff: file://.yoyo/artifacts/patch_42.diff

## Outgoing
- addresses -> failure_17
- validated_by -> eval_55
- approved_by -> decision_9
```

That output is the core promise: a patch is not just a change. It has a reason, evidence, validation, and a decision.

## Run the test suite

```bash
cargo test
```

The tests cover event append and scan, state ops, replay, patch status transitions, lineage, JSONL persistence, and changed-file observer helpers.

## Try local persistence

Initialize a local JSONL event log:

```bash
cargo run --bin yoagent-state -- init
```

Inspect the current graph:

```bash
cargo run --bin yoagent-state -- graph
```

Use a custom event log path:

```bash
YOAGENT_STATE_EVENTS=.yoyo/state/events.jsonl cargo run --bin yoagent-state -- events
```

The default local event log path is `.yoagent-state/events.jsonl`.

## Read next

- [Why Agents Need State](./why-agent-state.md)
- [First Lineage Example](./first-lineage-example.md)
- [Patch, Eval, Decision Tutorial](./patch-eval-decision-tutorial.md)
