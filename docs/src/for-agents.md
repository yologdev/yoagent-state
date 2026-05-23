# For Agents

This page is for coding agents and LLMs working in this repo.

Read the root `AGENTS.md` first. This page mirrors the most important guidance in the hosted docs.

## Project boundary

```text
yoagent = execution
yoagent-state = state, lineage, patches, evals, decisions
yoyo evolve = growth loop using both
```

Do not turn this crate into a workflow engine, graph database, Git replacement, compiler, or universal memory system.

## Where to look

- `src/event.rs`: append-only event shape
- `src/patch.rs`: state ops, patches, statuses, preconditions, effects
- `src/graph.rs`: graph projection data structures
- `src/projector.rs`: event replay into graph
- `src/state.rs`: high-level public API
- `src/store.rs`: memory and JSONL event stores
- `examples/`: runnable usage flows
- `tests/state_flow.rs`: regression coverage

## Commands

```bash
cargo test
/Users/yuanhao/.cargo/bin/mdbook build docs
cargo run --example patch_eval_decision
```

## Design rule

Prefer boring, explicit state over clever machinery.

When adding behavior, preserve this chain:

```text
failure -> hypothesis -> patch -> artifact -> eval -> decision -> promotion
```

When in doubt, store meaning and references. Let Git and the filesystem store concrete project state.
