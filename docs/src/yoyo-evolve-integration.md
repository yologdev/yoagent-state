# yoyo evolve Integration

The intended growth loop is:

```text
observe project
record snapshot reference
run task or eval
observe failure
create hypothesis
propose patch
apply patch in branch or worktree
run eval
record eval result
decide approve or reject
promote if approved
record lineage
```

For projects that yoyo is improving, `yoagent-state` should track:

- why a module exists
- why a dependency was added
- what test validates behavior
- what failure caused a patch
- what decision approved it
- what assumptions became stale
- what version introduced behavior

Concrete project diffs remain external. Reference them with `ArtifactRef` and `ProjectRef`.
