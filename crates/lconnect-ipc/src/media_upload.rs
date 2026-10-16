//! Capability-addressed, bounded persistent-media upload. Generate a fresh random
//! 128-bit lowercase hexadecimal id per import and retain it across retries.
use crate::{H264AccessUnit, PreparedClipInfo};
use serde::{Deserialize, Serialize};

pub const MAX_MEDIA_UPLOAD_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_MEDIA_UPLOAD_FRAMES: usize = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum MediaUploadRequest {
    Begin {
        id: String,
        name: String,
        frames: usize,
        encoded_bytes: u64,
    },
    /// One AU per request. Only an exact retry of the last accepted AU is supported.
    Chunk {
        id: String,
        index: usize,
        unit: H264AccessUnit,
    },
    Status {
        id: String,
    },
    Finish {
        id: String,
    },
    /// A published asset is never deleted by cancellation; its receipt is returned.
    Cancel {
        id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaUploadPhase {
    Receiving,
    Published,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaUploadState {
    pub id: String,
    pub name: String,
    pub frames: usize,
    pub encoded_bytes: u64,
    pub received_frames: usize,
    pub received_bytes: u64,
    pub phase: MediaUploadPhase,
    pub asset: Option<PreparedClipInfo>,
    pub error: Option<String>,
}
