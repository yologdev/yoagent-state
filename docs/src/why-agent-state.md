# Why Agents Need State

Logs tell you what happened. They do not reliably tell you why it mattered.

Long-running agents need a continuity layer because their work often spans many observations, tool calls, hypotheses, patches, evals, and decisions. Without durable state, the reasoning chain gets scattered across chat history, terminal output, temporary files, and Git commits.

## The failure mode

An agent can make a good change and still leave behind a weak explanation:

```text
test failed
agent tried something
files changed
test passed
commit created
```

That is not enough when someone later asks:

- What failure caused this patch?
- What evidence supported the hypothesis?
- Which eval validated it?
- What concrete diff did the patch refer to?
- Was the patch approved, rejected, promoted, or later made stale?

`yoagent-state` records that chain directly.

In the current runtime, that chain usually starts with durable intent:

```text
goal -> task -> run -> observation -> failure -> hypothesis -> patch -> artifact -> eval -> decision -> promotion
```

The exact run may only use part of the graph, but the state model has a place for each piece.

## Logs are not enough

Logs are chronological. Lineage is causal.

Chronology says:

```text
tool ran, model responded, file changed, test passed
```

Lineage says:

```text
patch_42 addresses failure_17
patch_42 references diff artifact patch_42.diff
eval_55 validated patch_42
decision_9 approved patch_42
```

The second form is what agents and maintainers need to explain project evolution.

## Durable state vs project diff

Git owns the concrete project state. `yoagent-state` owns the agent-facing meaning.

```text
Git stores what changed.
yoagent-state stores why it changed, what tested it, and what it means.
```

This keeps the library small. It does not parse every symbol, mirror every file, or replace source control.

## When to use it

Use `yoagent-state` when:

- an agent runs across multiple steps or sessions
- a patch needs evidence before promotion
- an eval result should be tied to a change
- a future agent should understand prior decisions
- project evolution needs an explainable history

Skip it when:

- the agent is stateless
- the task is a one-off script
- Git commit messages already capture enough context
- you need a full workflow engine, not a lineage layer
