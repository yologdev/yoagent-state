use std::env;
use std::process::ExitCode;
use yoagent_state::{
    ActorRef, EventStore, ForkId, Goal, GoalId, GoalStatus, JsonlEventStore, NodeId, PatchId,
    PatchStatus, YoAgentState,
};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());
    let path = env::var("YOAGENT_STATE_EVENTS")
        .unwrap_or_else(|_| ".yoagent-state/events.jsonl".to_string());
    let store = JsonlEventStore::new(path);
    let state = YoAgentState::load(store).await?;

    match command.as_str() {
        "init" => {
            tokio::fs::create_dir_all(".yoagent-state").await?;
            if !std::path::Path::new(".yoagent-state/events.jsonl").exists() {
                tokio::fs::write(".yoagent-state/events.jsonl", "").await?;
            }
            println!("initialized .yoagent-state/events.jsonl");
        }
        "events" => {
            let events = state.store().scan().await?;
            println!("{}", serde_json::to_string_pretty(&events)?);
        }
        "graph" | "replay" => {
            println!("{}", serde_json::to_string_pretty(&state.graph().await)?);
        }
        "node" => {
            let id = args.next().ok_or("usage: yoagent-state node <id>")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&state.get_node(NodeId::new(id)).await)?
            );
        }
        "lineage" => {
            let id = args.next().ok_or("usage: yoagent-state lineage <id>")?;
            let lineage = state.lineage(NodeId::new(id)).await;
            if args.next().as_deref() == Some("--markdown") {
                print!("{}", lineage.to_markdown());
            } else {
                println!("{}", serde_json::to_string_pretty(&lineage)?);
            }
        }
        "patch" => match args.next().as_deref() {
            Some("promote") => {
                let id = args
                    .next()
                    .ok_or("usage: yoagent-state patch promote <id>")?;
                state
                    .update_patch_status(
                        PatchId::new(id),
                        PatchStatus::Promoted,
                        Some("promoted from CLI".to_string()),
                    )
                    .await?;
                println!("promoted patch");
            }
            Some("show") => {
                let id = args.next().ok_or("usage: yoagent-state patch show <id>")?;
                let node_id = NodeId::new(PatchId::new(id).0);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state.get_node(node_id).await)?
                );
            }
            Some("list") => {
                let graph = state.graph().await;
                let patches = graph
                    .nodes
                    .values()
                    .filter(|node| node.kind == "patch")
                    .collect::<Vec<_>>();
                println!("{}", serde_json::to_string_pretty(&patches)?);
            }
            _ => print_help(),
        },
        "goal" => match args.next().as_deref() {
            Some("create") => {
                let id = args
                    .next()
                    .ok_or("usage: yoagent-state goal create <id> <title> [summary]")?;
                let title = args
                    .next()
                    .ok_or("usage: yoagent-state goal create <id> <title> [summary]")?;
                let summary = args.next().unwrap_or_else(|| title.clone());
                state
                    .record_goal(Goal::new(
                        GoalId::new(id),
                        title,
                        summary,
                        ActorRef::user("cli"),
                    ))
                    .await?;
                println!("created goal");
            }
            Some("status") => {
                let id = args
                    .next()
                    .ok_or("usage: yoagent-state goal status <id> <status>")?;
                let status = args
                    .next()
                    .ok_or("usage: yoagent-state goal status <id> <status>")?;
                state
                    .update_goal_status(
                        GoalId::new(id),
                        parse_goal_status(&status)?,
                        Some("updated from CLI".to_string()),
                    )
                    .await?;
                println!("updated goal status");
            }
            Some("show") => {
                let id = args.next().ok_or("usage: yoagent-state goal show <id>")?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state.get_node(NodeId::new(id)).await)?
                );
            }
            Some("list") => {
                let graph = state.graph().await;
                let goals = graph
                    .nodes
                    .values()
                    .filter(|node| node.kind == "goal")
                    .collect::<Vec<_>>();
                println!("{}", serde_json::to_string_pretty(&goals)?);
            }
            _ => print_help(),
        },
        "fork" => match args.next().as_deref() {
            Some("create") => {
                let id = args
                    .next()
                    .ok_or("usage: yoagent-state fork create <id> [event-id]")?;
                let event_id = args.next().map(yoagent_state::EventId::new);
                let fork = state.fork_at_event(ForkId::new(id), event_id).await?;
                println!("{}", serde_json::to_string_pretty(&fork)?);
            }
            _ => print_help(),
        },
        _ => print_help(),
    }

    Ok(())
}

fn print_help() {
    println!(
        "yoagent-state\n\ncommands:\n  init\n  events\n  graph\n  node <id>\n  lineage <id> [--markdown]\n  goal create <id> <title> [summary]\n  goal list\n  goal show <id>\n  goal status <id> <open|in-progress|satisfied|abandoned|blocked|stale>\n  patch list\n  patch show <id>\n  patch promote <id>\n  fork create <id> [event-id]\n  replay"
    );
}

fn parse_goal_status(value: &str) -> Result<GoalStatus, Box<dyn std::error::Error>> {
    match value {
        "open" => Ok(GoalStatus::Open),
        "in-progress" => Ok(GoalStatus::InProgress),
        "satisfied" => Ok(GoalStatus::Satisfied),
        "abandoned" => Ok(GoalStatus::Abandoned),
        "blocked" => Ok(GoalStatus::Blocked),
        "stale" => Ok(GoalStatus::Stale),
        _ => Err(format!("unknown goal status: {value}").into()),
    }
}
