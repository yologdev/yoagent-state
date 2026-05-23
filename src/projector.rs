use crate::{Event, Graph, StateError, StateOp};

pub const STATE_OPS_APPLIED: &str = "state.ops_applied";

pub fn project_event(graph: &mut Graph, event: &Event) -> Result<(), StateError> {
    if event.kind == STATE_OPS_APPLIED {
        let ops: Vec<StateOp> = event.payload_as()?;
        graph.apply_ops(&ops)?;
    }

    Ok(())
}

pub fn replay(events: &[Event]) -> Result<Graph, StateError> {
    let mut graph = Graph::default();
    for event in events {
        project_event(&mut graph, event)?;
    }
    Ok(graph)
}
