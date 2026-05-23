# AGENTS.md

Guidance for coding agents working on `yoagent-state`.

## Purpose

`yoagent-state` is an ActiveGraph-inspired Rust continuity runtime for long-running agents.

It records durable state and lineage:

```text
goal -> task -> run -> observation -> failure -> hypothesis -> patch -> artifact -> eval -> decision -> promotion
```

Treat this as a causal graph spine, not a mandatory linear workflow. Diffs are artifacts. Promotion is a patch status transition backed by eval and decision lineage.

Keep the boundary clear:

```text
Git stores what changed.
yoagent-state stores why it changed, what tested it, and what it means.
```

## Do not expand the scope casually

Do not turn this crate into:

- a Git replacement
- a workflow engine
- a graph database platform
- a compiler or AST database
- a universal memory system
- a hidden self-modification mechanism

The motto is simple but effective.

## Repo map

- `src/event.rs`: event and actor types
- `src/ids.rs`: ID newtypes
- `src/patch.rs`: `StateOp`, `StatePatch`, statuses, preconditions, expected effects
- `src/primitives.rs`: goals, tasks, observations, hypotheses, evals, decisions, frames, constants
- `src/schema.rs`: typed packs and relation validation
- `src/policy.rs`: policies and approval requests
- `src/behavior.rs`: behavior subscriptions
- `src/runtime.rs`: `YoAgentRuntime`
- `src/fork.rs`: fork and graph diff helpers
- `src/artifact.rs`: artifact references
- `src/graph.rs`: nodes, relations, graph projection
- `src/projector.rs`: replay rules
- `src/store.rs`: `EventStore`, memory store, JSONL store
- `src/state.rs`: main `YoAgentState` API
- `src/adapter.rs`: optional yoagent sink adapter
- `src/observer.rs`: coarse project-diff helpers
- `src/bin/yoagent-state.rs`: CLI
- `examples/`: runnable usage examples
- `tests/state_flow.rs`: integration tests
- `docs/src/`: mdBook source

## Verification

Run before finishing changes:

```bash
cargo test
/Users/yuanhao/.cargo/bin/mdbook build docs
```

For docs changes that mention examples, also run:

```bash
cargo run --example basic_lineage
cargo run --example patch_eval_decision
cargo run --example yoyo_evolve_demo
cargo run --example goal_lineage
cargo run --example behavior_subscription
cargo run --example policy_approval
cargo run --example replay_and_fork
cargo run --example typed_pack
```

## Implementation preferences

- Prefer append-only events and explicit state ops.
- Keep graph projection lossy and semantic.
- Reference external artifacts instead of embedding large blobs.
- Add tests when changing behavior.
- Keep docs honest about what exists now versus roadmap items.
- Preserve MIT-only licensing.

## Attribution

The core idea comes from Yohei Nakajima and ActiveGraph. Preserve the acknowledgment in README and docs.
