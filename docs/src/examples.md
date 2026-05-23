# Examples

The examples move from small lineage to a compact yoyo evolve-style flow.

## Basic lineage

```bash
cargo run --example basic_lineage
```

Use this first if you want to understand nodes and relations.

It creates:

```text
hypothesis_retry_state_lost --explains--> failure_retry_timeout
```

You should see a markdown lineage report with the hypothesis as the root and the failure as an outgoing relation.

## Patch, eval, decision

```bash
cargo run --example patch_eval_decision
```

This is the main MVP demo.

It records:

- a failure
- a patch that addresses the failure
- a fake Git diff artifact
- an eval result
- an approval decision
- a promoted patch status

Use this pattern when an agent proposes a change and needs evidence before promotion.

## yoagent integration

```bash
cargo run --example yoagent_integration
```

This uses `YoAgentStateSink` and `YoAgentStateAdapter` to record run, model, and tool lifecycle events.

It demonstrates how `yoagent-state` stays optional: the agent loop emits events to a sink, but the state layer does not take over execution.

## yoyo evolve demo

```bash
cargo run --example yoyo_evolve_demo
```

This records a compact growth loop with:

- a failure
- a project reference
- a diff artifact
- changed file relations
- an eval result
- an approval decision
- a promoted patch status

Use this example when you want to see how project-level artifacts connect to semantic lineage.

## Choosing an example

Start here:

```text
new to the model -> basic_lineage
want the core product value -> patch_eval_decision
integrating an agent loop -> yoagent_integration
building yoyo-style project evolution -> yoyo_evolve_demo
```
