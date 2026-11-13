//! Bounded, retryable user font upload protocol.
use serde::{Deserialize, Serialize};
pub const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;
pub const FONT_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum FontUploadRequest {
    Begin {
        size: usize,
    },
    Chunk {
        id: String,
        offset: usize,
        bytes: Vec<u8>,
    },
    Status {
        id: String,
    },
    Finish {
        id: String,
    },
    Cancel {
        id: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontUploadState {
    pub id: String,
    pub size: usize,
    pub received: usize,
    pub asset: Option<String>,
    pub cancelled: bool,
}
