use std::env;
use std::process::ExitCode;
use yoagent_state::{EventStore, JsonlEventStore, NodeId, PatchId, YoAgentState};

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
        _ => print_help(),
    }

    Ok(())
}

fn print_help() {
    println!(
        "yoagent-state\n\ncommands:\n  init\n  events\n  graph\n  node <id>\n  lineage <id> [--markdown]\n  patch list\n  patch show <id>\n  replay"
    );
}
