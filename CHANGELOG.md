# Changelog

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
