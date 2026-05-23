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
cargo run --example goal_lineage
```

You should see a lineage report like this:

```text
# Make retry behavior reliable

- id: goal_retry_reliability
- kind: goal
- status: InProgress

## Incoming
- serves <- task_retry_timeout
- blocks <- failure_retry_timeout
- advances <- patch_retry_state
```

Read it as: `goal_retry_reliability` is being served by a task, blocked by a failure, and advanced by a patch.

That is the core promise: state is not just a log. It is a graph that connects intent, work, evidence, change, and decision.

To inspect the patch/eval/decision lane directly:

```bash
cargo run --example patch_eval_decision
```

## Run the test suite

```bash
cargo test
```

The tests cover event append and scan, state ops, replay, goal/task/failure lineage, typed packs, policy approvals, behavior subscriptions, fork/diff helpers, patch status transitions, lineage, JSONL persistence, and changed-file observer helpers.

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
- [ActiveGraph-Inspired Runtime](./activegraph-runtime.md)
- [Patch, Eval, Decision Tutorial](./patch-eval-decision-tutorial.md)
