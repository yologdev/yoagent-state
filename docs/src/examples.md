# Examples

The examples move from small lineage to the current goal-centered runtime features.

## Goal lineage

```bash
cargo run --example goal_lineage
```

Start here for the current core graph shape. It shows a goal being served by a task, blocked by a failure, and advanced by a patch.

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

This is the main patch lifecycle demo.

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

## Behavior subscription

```bash
cargo run --example behavior_subscription
```

This registers a behavior that reacts to `failure.observed` and creates an investigation task.

## Policy approval

```bash
cargo run --example policy_approval
```

This registers a policy requiring approval before node creation. The attempted operation is blocked and an approval request node is created.

## Replay and fork

```bash
cargo run --example replay_and_fork
```

This creates a graph, forks at an earlier event, and diffs the fork against current state.

## Typed pack

```bash
cargo run --example typed_pack
```

This registers a pack that validates `goal`, `task`, and `serves` relation shapes.

## Choosing an example

Start here:

```text
new to the current model -> goal_lineage
want the smallest relation -> basic_lineage
want the patch lifecycle -> patch_eval_decision
need behaviors -> behavior_subscription
need policy gates -> policy_approval
need replay/fork/diff -> replay_and_fork
need typed validation -> typed_pack
integrating an agent loop -> yoagent_integration
building yoyo-style project evolution -> yoyo_evolve_demo
```
