# Examples

## Basic lineage

```bash
cargo run --example basic_lineage
```

This creates a failure and a hypothesis, links them with `explains`, and prints a markdown lineage report.

## Patch, eval, decision

```bash
cargo run --example patch_eval_decision
```

This demonstrates the main MVP flow:

```text
failure -> patch -> eval -> approval -> promotion
```

The patch has a fake `git.diff` artifact and records the eval command that validated it.

## yoagent integration

```bash
cargo run --example yoagent_integration
```

This uses `YoAgentStateSink` and `YoAgentStateAdapter` to record run, model, and tool lifecycle events without making the agent loop state-heavy.

## yoyo evolve demo

```bash
cargo run --example yoyo_evolve_demo
```

This records a compact growth loop with a failure, project reference, diff artifact, changed file relations, eval result, approval decision, and promoted patch status.
