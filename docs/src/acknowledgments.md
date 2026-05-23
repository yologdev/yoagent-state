# Acknowledgments

The core idea for `yoagent-state` comes from [Yohei Nakajima](https://github.com/yoheinakajima) and his [ActiveGraph](https://github.com/yoheinakajima/activegraph) work.

This project is an independent Rust implementation inspired by that idea. It keeps the architecture small for `yoagent` and `yoyo evolve`, while preserving the important ActiveGraph-style primitives: append-only events, replayed graph projection, goals, tasks, observations, hypotheses, patches, artifacts, evals, decisions, policies, behaviors, packs, replay, and forks.

The guiding boundary is:

```text
Git stores what changed.
yoagent-state stores why it changed, what tested it, and what it means.
```
