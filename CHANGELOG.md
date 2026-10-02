# Changelog

## 0.5.3 — 2026-10-02

### Added

- **`init_agent_repo` now declares the identity digest it was always supposed
  to** ([gasp#1](https://github.com/yologdev/gasp/issues/1)). The GASP restore
  contract (step 2) verifies `identity_hash` against the identity bytes, but
  the reference emitter wrote a manifest without one — so every repo it
  initialized was born unverifiable, and the conformance checker had nothing
  to hold it to. The manifest now carries the Part I digest, computed by the
  new public `identity_hash(root)` — SHA-256 over each identity file's
  relative path + newline + bytes, in byte-order-sorted path order — over
  whatever `identity/` holds at init. Pairs with the gasp checker release
  that verifies declared digests fail-closed; once this version is consumed
  there, a missing digest graduates from warning to failure.

## 0.5.2 — 2026-08-28

### Fixed

- **0.5.1's "not silent" promise was unreachable through the front door.**
  `YoAgentState::load` and `fork_events_at` both call `replay`, which discards
  the skip report — so the one system whose store had been bricked survived the
  brick and was told nothing. Green while incomplete is the property that made
  the original incident damaging, and 0.5.1 preserved it one layer quieter.

  `YoAgentState::load_with_diagnostics` returns them. `load` keeps its
  signature and now documents plainly that it drops them.

- **`missing_node` matched non-exhaustively**, so a future `StateOp` needing an
  existing node would compile clean and report `node: None` in every
  diagnostic — losing the operator's only handle. That is the same rot that
  made the original report name two ops when four were affected. Now exhaustive.

- **`SkippedOp::index` does not identify the event, and its doc claimed it
  did.** One `state.ops_applied` event carries many ops and the event identity
  is not recorded, so five skips from five events all report the same index.
  The node id is the more selective locator today; carrying an `EventId` needs
  a new field, which needs 0.6.0.

- **The `version` comment gave the wrong reason.** "Counts ops seen" is true of
  the lenient path only; strict counts ops *applied*. The conclusion held —
  two successful folds of one log cannot disagree — but the real divergence is
  writer-vs-reader, now documented.

- **`version` advanced on a skipped op, so every reader disagreed with the
  writer.** `Graph.version` is compared by whole-graph equality when a snapshot
  is checked against a fold of its log prefix, and strict `apply_ops`
  increments only *after* an op succeeds. Measured on the production shape:
  live `0`, replay `1`. Left alone, this would have failed snapshot
  verification the day an emitter ships — reporting non-conformance for a store
  every runtime can restore, the inversion this change exists to prevent.

  Nothing pinned it: counting skipped ops or not counting them both left the
  whole suite green. Now pinned by a test that drives the real write path.

  This aligns the single-op batch, the shape seen in the wild. A multi-op batch
  still diverges — strict abandons the ops after the failure while lenient
  applies them — which needs the write-path fix noted below.

### Added

- `VERSION` — this crate's version, baked in at its own compile time. Reports
  what a consumer **linked** rather than what a lockfile resolved, so a tool
  that must state which fold produced a verdict needs no build script.
- `#[must_use]` on `apply_ops_lenient` and `replay_with_diagnostics`. Dropping
  the diagnostics was not even a warning, which is how both internal call sites
  did it.

### Known

- The **write path is unchanged**: `record_event` appends before folding
  strictly, and `apply_ops` is not atomic. A writer that swallows the error
  keeps a live graph no later replay reproduces — same bytes, different graph,
  silently. 0.5.1 made the read survivable; it did not stop the divergence
  being created. Tracked separately.
- `CreateNode` on an existing id silently overwrites, and relation ops accept
  dangling endpoints without a diagnostic. `SkippedOp` reports ops that did
  *nothing*; it cannot report an op that did the *wrong thing*.

## 0.5.1 — 2026-08-28

### Fixed

- **A dangling op no longer makes an append-only log permanently unreadable**
  ([yologdev/yoagent#168](https://github.com/yologdev/yoagent/issues/168)).

  `Graph::apply_ops` aborts the whole fold on the first op referencing a node
  that does not exist. On an append-only log that is unrecoverable: the
  offending op cannot be removed, and nothing appended after it is ever
  reached. One such op made a live store unreadable, so every subsequent
  session failed to open it and the agent recorded nothing while running green.

  Neither repair was legal. Appending a corrective `CreateNode` is never
  reached, because the fold dies first. Inserting one before the bad op folds
  correctly but rewrites published history — permanently failing the
  append-only check, which is the property the format exists to guarantee.

  `replay` now skips such ops and continues. **The fix is also the repair:** an
  already-broken store becomes readable on upgrade, with no history rewrite.

  The issue named `UpdateNode` and `TombstoneNode` — and `CreateRelation`, which
  in fact never aborts. Four ops do: `UpdateNode`, `TombstoneNode`,
  `MarkStale` and `AttachArtifact`.
  `AttachArtifact` is the likeliest in practice, since artifacts are attached
  opportunistically by whichever process happens to hold one.

  **Not silent.** `replay_with_diagnostics` returns every skipped op with the
  batch index, the op kind, the node id and the strict error. Discarding that
  trades an unreadable store for a quietly wrong one, which is the failure mode
  this must not become. `replay` remains available for callers that do not
  inspect diagnostics, and its docs say plainly that it drops them.

  **`replay_strict` keeps the old behaviour**, for tools asserting
  well-formedness. The distinction is the point: a validator should reject a
  malformed log; a reader should not become permanently unable to read one.
  **Correction (0.5.2):** this originally said conformance checkers want the
  strict path. They do not — see 0.5.2.

  Note this does not change what the crate accepts on *write* — it changes what
  it can survive on read. `CreateRelation` and `DeleteRelation` were already
  lenient, and silently so; the node ops were strict. This makes that split
  coherent and, unlike the relation ops, reports what it skipped.

### Added

- `Graph::apply_ops_lenient`, `SkippedOp`, `projector::replay_with_diagnostics`,
  `projector::replay_strict`, `projector::project_event_lenient`.

## 0.5.0 — 2026-08-06

Measurement fidelity for the sink adapter: the log becomes sufficient for
offline cost analysis, compaction inference, and call matching
(yologdev/yoagent#104).

### Changed (breaking)

- **`YoAgentToolCalled` and `YoAgentModelFinished` gained a `metadata:
  JsonValue` field.** Struct-literal constructors must add it (`json!({})` to
  keep the old behaviour). Both deserialize with `#[serde(default)]`, so
  events recorded by earlier versions still parse.
  - `YoAgentToolCalled.metadata` is persisted onto the folded `ToolCall`
    node's props (previously hardcoded `{}`). Canonical use: a stable
    argument fingerprint (`{"args_fingerprint": ..}`) so calls can be
    matched — `input_summary` is a truncated human summary and cannot be.
  - `YoAgentModelFinished.metadata` rides in the raw `model.finished` event.
    Canonical use: token usage (`{"usage": {"input", "output", "cache_read",
    "cache_write"}}`), which makes cost computable from the log and makes
    compactions inferable (a sharp input-token drop between consecutive
    model calls in one run).


## 0.4.1 — 2026-07-03

### Added

- `YoAgentState::resume_open_run()` — recovers the open-run marker from the
  committed log (the last `run.started` with no matching `run.finished`), so
  auto-chaining and correlation continue across process boundaries. Needed by
  per-invocation CLI emitters (e.g. yoyo's gasp-emit shim), where every
  transition is a fresh process. `load` itself stays side-effect free.

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
