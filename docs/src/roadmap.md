# Roadmap

`yoagent-state` starts with an event-sourced graph runtime: append-only events, replayed graph projection, goal-first lineage, patch lifecycle, artifacts, policies, behaviors, forks, examples, and mdBook docs.

This roadmap lists the likely next steps without turning the project into a workflow engine, graph database, Git replacement, or universal agent framework.

## Current MVP

Implemented:

- Core ID, actor, event, artifact, patch, precondition, and expected-effect types.
- `MemoryEventStore` and `JsonlEventStore`.
- Append-only event recording and replay into an in-memory graph.
- State operations for nodes, relations, stale markers, tombstones, and artifacts.
- Patch proposal, patch status changes, eval records, decision records, and lineage queries.
- Goal, task, observation, hypothesis, model call, tool call, frame, fork, behavior, policy, pack, and view IDs.
- Runtime layer for typed packs, policy gates, behavior subscriptions, replay, fork, and diff.
- Small `yoagent` sink adapter for run/model/tool lifecycle events.
- Coarse project observer helpers for changed files and diff artifacts.
- CLI for init, events, graph, node, lineage, patch list/show, and replay.
- Examples and regression tests for the main state flows.
- mdBook user guide.

## Phase 1: MVP Hardening

Goal: make the current crate more reliable for early users.

- Add crate-level API examples as Rust doc tests.
- Add tests for tombstones, stale nodes, artifact attachment, relation deletion, and error cases.
- Add CLI integration tests using temporary JSONL event logs.
- Add schema compatibility tests for serialized event and patch JSON.
- Add stricter validation for patch status transitions.
- Add markdown lineage report tests so output remains stable.
- Improve error messages for malformed JSONL and missing nodes.

Success criteria:

- Public examples compile as doc tests.
- CLI behavior is covered by regression tests.
- Event JSON compatibility is protected by fixtures.

## Phase 2: Persistence and Replay

Goal: make local state durable and inspectable beyond simple demos.

- Make JSONL append safer for concurrent writers.
- Add compaction or snapshot support for large event logs.
- Add `scan_after` coverage for missing IDs and resumed replay.
- Add optional SQLite storage after JSONL behavior is proven.
- Add import/export commands for portable state bundles.

Success criteria:

- Restart/replay is reliable with larger logs.
- Users can choose JSONL for simplicity or SQLite for local scale.

## Phase 3: Better Query and Reports

Goal: answer practical “why does this exist?” questions directly.

- Add focused query helpers:
  - patches for failure
  - evals for patch
  - decisions for patch
  - artifacts for node
  - files modified by patch
  - stale assumptions related to patch
- Expand markdown lineage reports with grouped sections:
  - status
  - addresses
  - evidence
  - evals
  - modified files
  - decisions
  - promotion references
- Add JSON and markdown output options consistently to CLI commands.

Success criteria:

- One command can show why a patch exists, what validated it, what files changed, and whether it was promoted.

## Phase 4: Project Observer v0+

Goal: connect semantic patches to concrete project diffs without becoming a compiler.

- Add git command helpers for changed files, base commit, and diff artifact creation.
- Hash important artifacts such as diffs, logs, and eval outputs.
- Detect common project changes:
  - tests added or changed
  - docs changed
  - dependency manifest changed
  - source files changed
- Add base commit precondition helpers.
- Mark patches stale or conflicted when the project base changes.

Success criteria:

- A patch can automatically reference changed files, base commit, diff artifact, and basic project facts.

## Phase 5: yoagent Integration

Goal: let `yoagent` emit durable state without becoming state-heavy.

- Wire the adapter into real `yoagent` run hooks.
- Record model/tool causation and correlation IDs.
- Attach selected tool outputs as artifacts.
- Add examples that use the actual `yoagent` crate once its integration point is stable.
- Add tests for failed tool calls becoming failure observations.

Success criteria:

- `yoagent` can run normally with optional state recording enabled.

## Phase 6: yoyo evolve Demo

Goal: prove the growth loop end to end.

- Create a demo command that:
  - observes a failure
  - proposes a patch
  - records a diff artifact
  - runs an eval command
  - records eval output
  - approves or rejects the patch
  - prints a lineage report
- Use temporary branches or worktrees for patch evaluation.
- Keep promotion explicit and evidence-backed.

Success criteria:

- One demo shows failure -> patch -> diff artifact -> eval -> decision -> promotion with a readable lineage report.

## Phase 7: Policy and Safety Gates

Goal: make risky mutation explicit.

- Add policy checks for:
  - prompt mutation
  - tool configuration changes
  - memory/state mutation
  - project patch promotion
- Record policy decisions as first-class decision nodes.
- Require evidence before promotion.
- Add stale/conflicted status helpers for unsafe bases.

Success criteria:

- The library makes self-modification visible, reviewable, and auditable.

## Later, Only If Needed

Potential extensions:

- Behavior subscriptions for simple reactions like “failure observed -> create task”.
- Fork and replay support for comparing alternate histories.
- Richer project observers for Rust symbols, Cargo dependencies, and test surfaces.
- Web or TUI inspection UI.
- Remote artifact storage.
- Multi-agent views over the same state log.

These should wait until real yoyo evolve runs show that the added complexity is worth it.

## Non-Goals to Preserve

Do not turn `yoagent-state` into:

- a replacement for Git
- a workflow engine
- a graph database platform
- a compiler or AST database
- a universal memory system
- a hidden self-modification mechanism

The guiding rule remains: simple but effective.
