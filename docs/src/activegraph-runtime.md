# ActiveGraph-Inspired Runtime

`yoagent-state` is inspired by Yohei Nakajima's ActiveGraph work, adapted into an idiomatic Rust runtime for `yoagent` and `yoyo evolve`.

The full concept is:

```text
append-only event log
  -> deterministic replay
  -> typed graph projection
  -> pattern subscriptions
  -> behaviors
  -> policy-gated patches
  -> replay, fork, and diff
```

## What changed in v0.2

The core lineage path now starts from goals:

```text
goal -> task -> run -> observation -> failure -> hypothesis -> patch -> artifact -> eval -> decision -> promotion
```

This is the common graph spine. It is not a claim that every agent run must create every node. `artifact` includes diffs, logs, screenshots, files, eval output, and other external evidence. `promotion` is represented by the patch lifecycle status.

`yoagent-state` now has first-class IDs and helpers for:

- goals
- tasks
- runs
- observations
- hypotheses
- evals
- decisions
- project snapshots
- model calls
- tool calls
- frames
- forks
- behaviors
- policies
- packs
- views

## Runtime layers

`YoAgentState` remains the simple state API: record events, apply ops, query graph, query lineage.

`YoAgentRuntime` adds the ActiveGraph-inspired runtime layer:

- register typed packs
- validate typed nodes and relations
- register behavior subscriptions
- enforce policy gates
- create approval requests

This keeps simple usage simple while allowing richer agent systems to use the full concept.

## Extensible storage

The event log remains the source of truth.

Storage is split into traits:

- `EventStore`
- `SnapshotStore`
- `ForkStore`
- `IndexStore`
- `ArtifactStore`

JSONL is implemented first because it is inspectable. SQLite, PostgreSQL, and graph-backed projections can be added later behind the same traits.

## Behaviors

Behaviors subscribe to event patterns and return state ops.

They do not mutate the graph directly.

```text
event -> matching behavior -> new events/state ops -> replayable state
```

This keeps behavior execution auditable.

## Policies

Policies can allow, deny, or require approval for sensitive actions.

The current policy foundation supports approval requests for runtime operations. More policy surfaces can be added without changing the event-sourced model.

## Replay, fork, diff

Replay rebuilds graph state from events.

Fork creates an alternate event history from a parent event cutoff.

Diff compares projected graphs so agents can inspect what changed between histories.
