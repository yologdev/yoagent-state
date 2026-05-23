# Safety Rules

## Self-modification must be explicit

Do not silently mutate prompts, policies, tools, memory, or code. Use patches.

## Promotion requires evidence

A patch should not be promoted without at least one passing eval, passing test, policy approval, or human approval.

## Project base must be checked

If a patch was created against commit `abc123`, do not blindly apply it to another commit. Mark it stale or conflicted, then reobserve.

## Staleness is first-class

Use stale nodes or statuses when assumptions, patches, projections, or observations are no longer current.

## Important artifacts need hashes

Use hashes for diffs, logs, files, and eval outputs when possible.
