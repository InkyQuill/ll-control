//! Finite export over a dedicated IPC connection. Subscribe to `media_export`
//! before calling `open_media_export`; disconnect cancels the transfer.
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaExportInfo {
    pub fps: u8,
    pub frames: usize,
    pub encoded_bytes: u64,
}

pub struct MediaExport {
    pub info: MediaExportInfo,
    pub units: BoxStream<'static, crate::Result<crate::H264AccessUnit>>,
}

/// A prefix is never a successful export. Consumers must check stream id,
/// contiguous indices, metadata totals and the explicit Complete event.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum MediaExportEvent {
    Unit {
        stream_id: String,
        index: usize,
        unit: crate::H264AccessUnit,
    },
    Complete {
        stream_id: String,
    },
    Failed {
        stream_id: String,
        message: String,
    },
}
