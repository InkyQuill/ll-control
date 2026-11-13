//! Bounded, daemon-lifetime profile archive transfers. Caller-generated IDs make
//! Begin retryable; keep the ID until Finish/readback is acknowledged. Sessions
//! expire after idle timeout; daemon restart invalidates unfinished transfers.
use crate::{NamedProfile, ProfileGroup, ProfileLibrary};
use serde::{Deserialize, Serialize};
pub const PROFILE_ARCHIVE_CHUNK_BYTES: usize = 64 * 1024;
/// Payload8GiB, manifest1MiB, fixed prefix44 bytes.
pub const MAX_PROFILE_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024 + 1024 * 1024 + 44;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileArchiveRequest {
    BeginExport {
        id: String,
        expected_revision: u64,
        group: ProfileGroup,
        slot: u8,
    },
    BeginImport {
        id: String,
        size: u64,
        expected_revision: u64,
        group: ProfileGroup,
        slot: u8,
    },
    Chunk {
        id: String,
        offset: u64,
        bytes: Vec<u8>,
    },
    Read {
        id: String,
        offset: u64,
        length: usize,
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
    /// Release an acknowledged receipt or completed export; never undo an import.
    Release {
        id: String,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileArchiveDirection {
    Import,
    Export,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileArchivePhase {
    Uploading,
    Ready,
    Imported,
    Failed,
    Cancelled,
}
/// Durable acknowledgement of one import, stored atomically with the slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveImportReceipt {
    /// Original archive manifest hash. None is a legacy, metadata-only receipt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_sha256: Option<String>,
    pub id: String,
    pub size: u64,
    pub expected_revision: u64,
    pub committed_revision: u64,
    pub group: ProfileGroup,
    pub slot: u8,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileArchiveState {
    pub id: String,
    pub direction: ProfileArchiveDirection,
    pub group: ProfileGroup,
    pub slot: u8,
    pub expected_revision: u64,
    pub phase: ProfileArchivePhase,
    pub size: u64,
    pub received: u64,
    pub profile: Option<NamedProfile>,
    pub library: Option<ProfileLibrary>,
    pub last_error: Option<String>,
    pub committed_revision: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileArchiveReply {
    pub state: ProfileArchiveState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bytes: Vec<u8>,
    pub eof: bool,
}

#[cfg(test)]
mod receipt_tests {
    use super::*;

    #[test]
    fn persisted_legacy_receipts_round_trip_and_new_receipts_bind_the_manifest() {
        let legacy = serde_json::json!({
            "id": "77777777777777777777777777777777", "size": 44,
            "expected_revision": 0, "committed_revision": 1,
            "group": "screen", "slot": 7
        });
        let mut receipt: ArchiveImportReceipt = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(receipt.source_sha256, None);
        assert_eq!(serde_json::to_value(&receipt).unwrap(), legacy);
        let mut library = ProfileLibrary {
            revision: 1,
            archive_imports: vec![receipt.clone()],
            ..Default::default()
        };
        library.validate().unwrap();
        receipt.source_sha256 = Some("ab".repeat(32));
        library.archive_imports[0] = receipt.clone();
        library.validate().unwrap();
        let persisted = serde_json::to_vec(&library).unwrap();
        assert_eq!(
            serde_json::from_slice::<ProfileLibrary>(&persisted).unwrap(),
            library
        );
        for invalid in ["".into(), "ab".repeat(31), "AB".repeat(32), "xz".repeat(32)] {
            library.archive_imports[0].source_sha256 = Some(invalid);
            assert!(library.validate().is_err());
        }
    }
}
