# Patch Lifecycle

A state patch records semantic intent and evidence. It is not a replacement for a Git diff.

The lifecycle is:

```text
proposed -> applied_in_fork -> evaluated -> approved/rejected -> promoted
```

```mermaid
stateDiagram-v2
  [*] --> Proposed
  Proposed --> AppliedInFork
  AppliedInFork --> Evaluated
  Evaluated --> Approved
  Evaluated --> Rejected
  Approved --> Promoted
  Proposed --> Stale
  AppliedInFork --> Conflicted
  Evaluated --> Stale
  Rejected --> [*]
  Promoted --> [*]
```

Additional states:

- `stale`
- `conflicted`

The patch should answer:

- what failure it addresses
- what hypothesis or evidence supports it
- what concrete artifact contains the project diff
- what eval validated it
- who or what approved it
- which commit or promotion contains it

Promotion should require evidence such as a passing eval, a passing test, or explicit human approval.

```mermaid
flowchart LR
  patch["patch"]
  failure["failure"]
  artifact["diff / log / file artifact"]
  eval["passing eval or test"]
  decision["approval decision"]
  promoted["promoted status"]

  patch -- addresses --> failure
  patch -- references --> artifact
  patch -- validated_by --> eval
  patch -- approved_by --> decision
  decision --> promoted
```
