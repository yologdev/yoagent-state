# Introduction

`yoagent-state` is a small Rust continuity layer for long-running agents.

It records durable state and lineage without taking over execution, Git, the filesystem, CI, or project management. The core model is intentionally small:

```text
append events -> replay graph -> query lineage
```

The useful product is not just a changed file. The useful product is an explanation:

```text
failure -> hypothesis -> patch -> project diff -> eval -> decision -> promotion
```

Use `yoagent-state` when an agent needs to remember why work exists, what tested it, and what decision made it part of the project.
