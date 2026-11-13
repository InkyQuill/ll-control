//! Full-group upload/preparation and explicit durable Finish/Apply.
use crate::{GroupArchiveSnapshot, GroupArchiveSummary};
use serde::{Deserialize, Serialize};
/// Explicit Windows bindings and aliases; source paths are data, never host paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsScreenSource {
    pub record: usize,
    pub bindings_json: String,
    pub resources_json: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupUploadRequest {
    Begin {
        id: String,
        size: u64,
        expected: String,
    },
    BeginWindowsScreen {
        id: String,
        size: u64,
        expected: String,
        source: WindowsScreenSource,
    },
    Chunk {
        id: String,
        offset: u64,
        bytes: Vec<u8>,
    },
    Inspect {
        id: String,
    },
    Finish {
        id: String,
    },
    Status {
        id: String,
    },
    Cancel {
        id: String,
    },
    Release {
        id: String,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupUploadPhase {
    Uploading,
    Inspecting,
    Prepared,
    Importing,
    Imported,
    RecoveryRequired,
    Cancelling,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupUploadState {
    pub id: String,
    pub target: GroupArchiveSnapshot,
    pub size: u64,
    pub received: u64,
    pub phase: GroupUploadPhase,
    pub summary: Option<GroupArchiveSummary>,
    pub source_sha256: Option<String>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub committed: Option<crate::FullGroupCommit>,
}

/// Independent source scene versus the selected source editor preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowsScreenPreviewSource {
    Scene,
    Editor,
}
