# Safety Model

`yoagent-state` is designed around explicit mutation and evidence-backed promotion.

## Self-modification must be explicit

Agents should not silently mutate prompts, policies, tools, memory, or code.

Use patches.

## Promotion requires evidence

A patch should not be promoted without at least one of:

- passing eval
- passing test
- human approval
- policy approval

The evidence should be represented in lineage, not hidden in a transcript.

## Project base must be checked

If a patch was created against commit `abc123`, do not blindly apply it to another commit.

Mark the patch stale or conflicted, then reobserve.

## Staleness is first-class

Assumptions go stale. So do patches, projections, and observations.

Use stale nodes or statuses when state is no longer current.

## Important artifacts need hashes

Use hashes for:

- diffs
- logs
- files
- eval outputs
- generated reports

Hashes make lineage more trustworthy.

## Keep the layer small

Do not turn `yoagent-state` into a hidden automation engine. It should record state and lineage first. Automation can come later, after the state model proves useful.
