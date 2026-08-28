use crate::{Event, Graph, SkippedOp, StateError, StateOp};

pub const STATE_OPS_APPLIED: &str = "state.ops_applied";

pub fn project_event(graph: &mut Graph, event: &Event) -> Result<(), StateError> {
    if event.kind == STATE_OPS_APPLIED {
        let ops: Vec<StateOp> = event.payload_as()?;
        graph.apply_ops(&ops)?;
    }

    Ok(())
}

/// Like [`project_event`] but skips ops referencing missing nodes, returning
/// what was skipped.
///
/// A malformed *payload* is still an error — that is a broken event, not a
/// dangling reference, and nothing sensible can be done with it.
pub fn project_event_lenient(
    graph: &mut Graph,
    event: &Event,
) -> Result<Vec<SkippedOp>, StateError> {
    if event.kind == STATE_OPS_APPLIED {
        let ops: Vec<StateOp> = event.payload_as()?;
        return Ok(graph.apply_ops_lenient(&ops));
    }
    Ok(Vec::new())
}

/// Fold events into a graph, surviving ops that reference missing nodes.
///
/// # This is a reader, not a validator
///
/// It used to abort on the first dangling reference, which on an append-only
/// log is permanent: the offending op cannot be removed, and nothing appended
/// after it is ever reached. One such op made an entire store unreadable
/// forever, with no legal repair — inserting a fix before it rewrites
/// published history, which is the property the format exists to guarantee.
///
/// A validator should reject a malformed log. A reader should not become
/// permanently unable to read one. Use [`replay_strict`] when validating.
///
/// **This discards the diagnostics.** Prefer
/// [`replay_with_diagnostics`] and surface what it reports — a skipped op
/// means the log says something the graph cannot represent, and silence there
/// trades an unreadable store for a quietly wrong one.
pub fn replay(events: &[Event]) -> Result<Graph, StateError> {
    replay_with_diagnostics(events).map(|(graph, _skipped)| graph)
}

/// [`replay`], plus every op that was skipped.
///
/// A non-empty second element means the log is malformed but readable. Report
/// it; do not drop it.
pub fn replay_with_diagnostics(events: &[Event]) -> Result<(Graph, Vec<SkippedOp>), StateError> {
    let mut graph = Graph::default();
    let mut skipped = Vec::new();
    for event in events {
        skipped.extend(project_event_lenient(&mut graph, event)?);
    }
    Ok((graph, skipped))
}

/// Fold events, failing on the first op that references a missing node.
///
/// The pre-0.5.1 behaviour of [`replay`], kept for validation. A conformance
/// checker wants exactly this: a log that cannot be folded cleanly is one it
/// should reject, even though a reader must still be able to open it.
pub fn replay_strict(events: &[Event]) -> Result<Graph, StateError> {
    let mut graph = Graph::default();
    for event in events {
        project_event(&mut graph, event)?;
    }
    Ok(graph)
}
