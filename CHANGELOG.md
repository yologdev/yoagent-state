# Changelog

## 0.4.0 — 2026-07-03

Conformance follow-ups: the sink adapter now emits GASP-conformant logs, runs
close in the folded graph, and events carry run correlation.

### Changed (breaking)

- **`YoAgentStateAdapter` routes callbacks through the paired helpers.**
  `on_run_started`/`on_run_finished` now enforce run-transition validation
  (double-start, finish-with-no-open-run, mismatched run id →
  `StateError::Validation`; previously they always succeeded).
- **Adapter event payloads changed shape.** `failure.observed` (from a failed
  `on_tool_finished`) is now the paired `{id, failure_id, title, summary}`
  instead of `{run_id, tool, output_summary}`; `model.called`/`tool.called`
  are now full `ModelCall`/`ToolCall` entities (generated node `id`,
  `output_summary: null`, `metadata`) and create graph nodes with
  `produced_by` relations. The run structs' `metadata` fields are currently
  not persisted by the paired helpers.
- **`record_run_finished` returns the paired `state.ops_applied` event id**,
  not the `run.finished` domain event id (consistent with
  `record_run_started`).

### Changed (log/graph shape)

- `run.finished` gains a paired ops event: the folded run node transitions to
  `status: "finished"` with an `outcome` prop, instead of staying `"started"`
  forever.
- `correlation_id` is populated with the run id on `run.started` and on every
  event recorded while a run is open (explicit correlations are never
  overwritten; events outside runs stay uncorrelated).
- `record_run_started` opens the run marker before appending the ops pair (so
  the pair carries the run correlation), rolling the marker back if the ops
  append fails; a failed `record_run_finished` leaves the run open for retry
  (a retry appends a fresh `run.finished` domain event).

## 0.3.0 — 2026-07-02

GASP store contract release: `yoagent-state` can now persist agent state as a
[GASP](https://github.com/yologdev/gasp)-conformant git repo. Emitted logs pass
all 7 GASP conformance checks.

### Added

- `GitEventStore`: git-backed `EventStore` for the GASP layout
  (`state/events.jsonl`). Appends are flushed + fsynced per batch (plus a
  parent-directory fsync on first creation); a cross-process single-writer
  lease at `.agent/lease` is taken atomically (exclusive create) inside the
  append path; an in-process mutex serializes appends across tasks/clones.
- `GitEventStore::commit_run(&RunId, &GoalId, outcome, extra_paths)`: one
  boundary commit per run with `Run-Id`/`Goal`/`Outcome` trailers,
  pathspec-scoped so unrelated staged/dirty files are never swept in; returns
  `Ok(None)` on idle runs. Rejects newline trailer forgery.
- `init_agent_repo`: convenience scaffold (git init + AGENT.md + identity/).
- `YoAgentState::apply_ops_caused_by`: `apply_ops` with an explicit
  `causation_id`.
- Corrupt/torn event-log lines are reported with `path:line` and a recovery
  hint; an unreadable log is an error rather than an empty graph.

### Changed (breaking)

- **Run transitions are validated.** `record_run_started` errors if a run is
  already open; `record_run_finished` errors if no run is open or the run id
  does not match. Previously both always succeeded.
- **`record_observation` emits the relation `observes`** (baseline GASP
  vocabulary) instead of the undeclared `observed_in`. Consumers querying
  `observed_in` edges must switch.

### Changed (log shape, non-API)

- Every `record_*` helper now sets its `state.ops_applied` event's
  `causation_id` to the paired domain event (the GASP pairing rule).
  Previously ops events had `causation_id: null`.
- Domain events recorded while a run is open auto-chain to the `run.started`
  event, so causation graphs root at `*.created` / `*.started`.
- `failure.observed` payloads carry `id` in addition to `failure_id`
  (additive).

## 0.2.0 — 2026-06

Initial public release: append-only event log, semantic graph fold, lineage,
replay, fork, diff, packs, policies, behaviors; `MemoryEventStore` and
`JsonlEventStore`.
