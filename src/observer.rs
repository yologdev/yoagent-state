use crate::{ArtifactRef, NodeId, PatchId, ProjectRef, StateOp};
use serde_json::json;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
}

pub fn parse_git_name_status(raw: &str) -> Vec<ChangedFile> {
    raw.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let status = parts.next()?;
            let path = parts.next()?;
            Some(ChangedFile {
                path: path.to_string(),
                status: status.to_string(),
            })
        })
        .collect()
}

pub fn diff_artifact(
    diff_path: impl AsRef<Path>,
    summary: impl Into<String>,
    base_commit: impl Into<String>,
    files: &[ChangedFile],
) -> ArtifactRef {
    ArtifactRef::new(
        "git.diff",
        format!("file://{}", diff_path.as_ref().display()),
    )
    .with_summary(summary)
    .with_metadata(json!({
        "base_commit": base_commit.into(),
        "files_changed": files.iter().map(|file| &file.path).collect::<Vec<_>>(),
    }))
}

pub fn project_ref(
    repo: impl Into<String>,
    branch: Option<String>,
    commit: Option<String>,
    worktree: Option<String>,
) -> ProjectRef {
    ProjectRef {
        repo: repo.into(),
        branch,
        commit,
        worktree,
    }
}

pub fn changed_file_ops(patch_id: PatchId, files: &[ChangedFile]) -> Vec<StateOp> {
    let patch_node_id = NodeId::new(patch_id.0);
    let mut ops = Vec::new();
    for file in files {
        let file_id = NodeId::new(format!("file:{}", file.path));
        ops.push(StateOp::CreateNode {
            id: file_id.clone(),
            kind: "file".to_string(),
            props: json!({ "path": file.path, "status": file.status }),
        });
        ops.push(StateOp::CreateRelation {
            from: patch_node_id.clone(),
            rel: "modifies".to_string(),
            to: file_id,
            props: json!({ "status": file.status }),
        });
    }
    ops
}
