# yoagent-state

Durable memory and lineage for long-running agents.

![yoagent-state banner](docs/images/banner.png)

Agents do not just need logs. They need to remember what failed, what changed, what tested it, who approved it, and why the current project state exists.

`yoagent-state` is an ActiveGraph-inspired Rust continuity runtime for agent systems. It records append-only events, replays them into a semantic graph, and gives you primitives for goals, tasks, observations, hypotheses, patches, artifacts, evals, decisions, policies, behaviors, replay, and forks.

It helps answer the questions that matter after an agent run:

- What goal was the agent trying to satisfy?
- What task, run, or observation produced this state?
- Why does this patch exist?
- What failure did it address?
- What eval validated it?
- What files or artifacts did it reference?
- Was it approved, rejected, or promoted?

```text
goal -> task -> run -> observation -> failure -> hypothesis -> patch -> artifact -> eval -> decision -> promotion
```

That line is the common causal spine, not a required linear workflow. A diff is an artifact. Promotion is a patch status transition backed by evals and decisions.

```mermaid
flowchart LR
  goal["goal"]
  task["task"]
  run["run"]
  observation["observation"]
  failure["failure"]
  hypothesis["hypothesis"]
  patch["patch"]
  artifact["artifact"]
  eval["eval"]
  decision["decision"]
  promoted["promoted status"]

  task -- serves --> goal
  run -- produces --> observation
  observation -- observes --> failure
  hypothesis -- explains --> failure
  patch -- addresses --> failure
  patch -- advances --> goal
  patch -- references --> artifact
  patch -- validated_by --> eval
  patch -- approved_by --> decision
  decision -- allows --> promoted
```

```text
yoagent executes.
yoagent-state remembers.
yoyo evolve improves.
```

## Start in 60 seconds

Add the crate:

```bash
cargo add yoagent-state
```

Run the demo from a local clone:

```bash
git clone https://github.com/yologdev/yoagent-state.git
cd yoagent-state
cargo run --example goal_lineage
```

You should see a lineage report like this:

```text
# Make retry behavior reliable

- id: goal_retry_reliability
- kind: goal
- status: InProgress

## Incoming
- serves <- task_retry_timeout
- blocks <- failure_retry_timeout
- advances <- patch_retry_state
```

This means the goal is being served by a task, blocked by a failure, and advanced by a patch.

```mermaid
flowchart LR
  task["task_retry_timeout<br/>kind: task"]
  failure["failure_retry_timeout<br/>kind: failure"]
  patch["patch_retry_state<br/>kind: patch"]
  goal["goal_retry_reliability<br/>kind: goal<br/>status: InProgress"]

  task -- serves --> goal
  failure -- blocks --> goal
  patch -- advances --> goal
```

To see the patch/eval/decision lane:

```bash
cargo run --example patch_eval_decision
```

Run the full test suite:

```bash
cargo test
```

Try local JSONL persistence:

```bash
cargo run --bin yoagent-state -- init
cargo run --bin yoagent-state -- graph
YOAGENT_STATE_EVENTS=.yoyo/state/events.jsonl cargo run --bin yoagent-state -- events
```

## What it does

`yoagent-state` gives long-running agents durable continuity without taking over your project.

- Records append-only events for goals, tasks, runs, observations, model calls, tool calls, failures, hypotheses, patches, evals, decisions, and artifacts.
- Replays events into a small semantic graph projection.
- Tracks goal/task lineage and patch lifecycle from proposal to approval, rejection, or promotion.
- References real project artifacts such as diffs, commits, logs, eval output, and files.
- Supports typed packs, policy gates, behavior subscriptions, replay, fork, and diff primitives.
- Exposes lineage queries so agents and humans can explain why state exists.

Git still owns concrete project changes. `yoagent-state` stores why those changes happened, what tested them, and what they mean.

## When you need this

Use `yoagent-state` when:

- your agent runs longer than one prompt
- you need to explain why a code change exists
- you want eval and decision history attached to patches
- you want durable state without adopting a workflow engine or graph database
- you are building on `yoagent`, `yoyo evolve`, or another Rust agent loop

You probably do not need it for one-off scripts, stateless chat flows, or projects where Git commit messages already capture enough context.

## Minimal Rust example

```rust
use serde_json::json;
use yoagent_state::{ActorRef, MemoryEventStore, NodeId, StateOp, YoAgentState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let failure = NodeId::new("failure_1");

    state.apply_ops(
        ActorRef::agent("demo"),
        vec![StateOp::CreateNode {
            id: failure.clone(),
            kind: "failure".to_string(),
            props: json!({ "title": "retry state lost after timeout" }),
        }],
    ).await?;

    print!("{}", state.lineage(failure).await.to_markdown());
    Ok(())
}
```

## What it is not

`yoagent-state` is intentionally small.

- not a replacement for Git
- not a workflow engine
- not a graph database
- not a full project database
- not a universal agent framework
- not a hidden self-modification system

The motto is simple but effective.

## Documentation

Hosted docs:

```text
https://yologdev.github.io/yoagent-state/
```

Run the mdBook locally:

```bash
mdbook serve docs
```

If `mdbook` is not installed:

```bash
cargo install mdbook
```

If Cargo's binary directory is not on your `PATH`, run it directly:

```bash
~/.cargo/bin/mdbook serve docs
```

GitHub Pages is deployed by `.github/workflows/docs.yml`. In the GitHub repo settings, Pages source should be set to **GitHub Actions**.

## For coding agents

Read [AGENTS.md](./AGENTS.md) before modifying the repo. It explains the project boundary, core files, test commands, and the simple-but-effective design rule.

## Roadmap

The future plan is tracked in [ROADMAP.md](./ROADMAP.md) and mirrored in the mdBook guide.

## Acknowledgments

The core idea for `yoagent-state` comes from [Yohei Nakajima](https://github.com/yoheinakajima) and his [ActiveGraph](https://github.com/yoheinakajima/activegraph) work. This project is an independent Rust implementation inspired by that idea, with a Rust-first architecture for `yoagent` and `yoyo evolve`. See [ACKNOWLEDGMENTS.md](./ACKNOWLEDGMENTS.md).

## License

Licensed under the [MIT license](./LICENSE).
