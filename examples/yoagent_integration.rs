use serde_json::json;
use yoagent_state::{
    ActorRef, EventStore, MemoryEventStore, RunId, YoAgentModelCalled, YoAgentModelFinished,
    YoAgentRunFinished, YoAgentRunStarted, YoAgentState, YoAgentStateAdapter, YoAgentStateSink,
    YoAgentToolCalled, YoAgentToolFinished,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = YoAgentState::load(MemoryEventStore::new()).await?;
    let sink = YoAgentStateAdapter::new(state.clone(), ActorRef::agent("yoagent"));
    let run_id = RunId::new("run_demo");

    sink.on_run_started(YoAgentRunStarted {
        run_id: run_id.clone(),
        task: "Investigate retry timeout".to_string(),
        metadata: json!({}),
    })
    .await?;
    sink.on_model_called(YoAgentModelCalled {
        run_id: run_id.clone(),
        model: "example-model".to_string(),
        prompt_summary: "Find likely retry failure cause".to_string(),
    })
    .await?;
    sink.on_model_finished(YoAgentModelFinished {
        run_id: run_id.clone(),
        model: "example-model".to_string(),
        output_summary: "Retry state appears scoped too narrowly".to_string(),
    })
    .await?;
    sink.on_tool_called(YoAgentToolCalled {
        run_id: run_id.clone(),
        tool: "cargo test".to_string(),
        input_summary: "tool_retry_survives_timeout".to_string(),
    })
    .await?;
    sink.on_tool_finished(YoAgentToolFinished {
        run_id: run_id.clone(),
        tool: "cargo test".to_string(),
        output_summary: "test passed".to_string(),
        success: true,
    })
    .await?;
    sink.on_run_finished(YoAgentRunFinished {
        run_id,
        outcome: "patch ready for review".to_string(),
        metadata: json!({}),
    })
    .await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&state.store().scan().await?)?
    );
    Ok(())
}
