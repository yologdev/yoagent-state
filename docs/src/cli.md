# CLI

The CLI is intentionally small.

```bash
yoagent-state init
yoagent-state events
yoagent-state graph
yoagent-state node <id>
yoagent-state lineage <id>
yoagent-state lineage <id> --markdown
yoagent-state patch list
yoagent-state patch show <id>
yoagent-state replay
```

The default event log is `.yoagent-state/events.jsonl`.

Set `YOAGENT_STATE_EVENTS` to use another path.
