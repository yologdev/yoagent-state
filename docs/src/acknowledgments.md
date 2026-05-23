# Acknowledgments

`yoagent-state` is informed by ActiveState/ActiveGraph-style ideas around durable agent state, lineage, and explainable project evolution.

This project is independent. It intentionally keeps the first version smaller: append-only events, a replayed graph projection, patch lifecycle, eval lineage, decisions, and artifact references.

The guiding boundary is:

```text
Git stores what changed.
yoagent-state stores why it changed, what tested it, and what it means.
```
