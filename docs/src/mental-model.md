# Core Mental Model

`yoagent-state` starts with three moving parts:

```text
append events -> replay graph -> query lineage
```

The full runtime adds typed packs, behaviors, policies, replay, forks, frames, and views on top of that event-sourced base.

The graph is not the source of truth. It is a projection derived from append-only events.

## Event log

An event is an immutable fact about something that happened.

Examples:

- `run.started`
- `tool.finished`
- `goal.created`
- `task.created`
- `failure.observed`
- `hypothesis.created`
- `patch.proposed`
- `patch.status_changed`
- `artifact.attached`
- `state.ops_applied`

Events are append-only. Do not mutate historical events.

## State ops

State ops are the small mutation language for the graph projection.

They can:

- create or update nodes
- tombstone nodes
- create or delete relations
- mark nodes stale
- attach artifacts

Only `state.ops_applied` events mutate the graph directly.

## Graph projection

The graph is a semantic view of agent state.

Common node kinds:

- `goal`
- `task`
- `run`
- `observation`
- `failure`
- `hypothesis`
- `patch`
- `eval`
- `decision`
- `artifact`
- `file`
- `model_call`
- `tool_call`
- `frame`

Common relation kinds:

- `serves`
- `blocks`
- `advances`
- `observes`
- `addresses`
- `explains`
- `validated_by`
- `approved_by`
- `rejected_by`
- `modifies`
- `references`
- `produced_by`
- `contained_in_frame`
- `forked_from`

The graph should stay lossy. It should preserve what matters for continuity and explanation, not every line of a log.

## Patches

A state patch is a proposed semantic change with evidence.

It can include:

- base state version
- project reference
- preconditions
- expected effects
- evidence nodes
- artifact refs
- state ops
- lifecycle status

Patch lifecycle:

```text
proposed -> applied_in_fork -> evaluated -> approved/rejected -> promoted
```

## Artifacts

Artifacts point to external evidence such as:

- Git diffs
- commits
- files
- test output
- build logs
- eval result JSON
- model or tool output

Store paths, URIs, summaries, and hashes where practical.

## Replay

On startup, the store scans events and replays them into the graph projection.

This makes state durable without requiring a graph database.

## Behaviors, policies, and packs

Typed packs validate object and relation shapes.

Behaviors react to event patterns and return state ops.

Policies gate sensitive actions by allowing, denying, or requiring approval.

These are runtime features, but they still preserve the same rule: durable state comes from append-only events.
