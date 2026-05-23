# Acknowledgments

The core idea for `yoagent-state` comes from [Yohei Nakajima](https://github.com/yoheinakajima) and his [ActiveGraph](https://github.com/yoheinakajima/activegraph) work.

This project is an independent Rust implementation inspired by that idea. It intentionally keeps the first version smaller for `yoagent` and `yoyo evolve`: append-only events, a replayed graph projection, patch lifecycle, eval lineage, decisions, and artifact references.

The guiding boundary is:

```text
Git stores what changed.
yoagent-state stores why it changed, what tested it, and what it means.
```
