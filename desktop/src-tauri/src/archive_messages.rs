//! Stable app-owned notices. Raw diagnostics stay separate from presentation.
use serde::Serialize;
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveNotice {
    Cancelled,
    ProfileExportUnconfirmed,
    GroupExportUnconfirmed,
    CancelUnconfirmed,
    InvalidArchiveFile,
    InvalidProfileArchive,
    InvalidManifestLength,
    DestinationExists,
}
impl ArchiveNotice {
    pub fn from_error(error: &anyhow::Error) -> Option<Self> {
        error.downcast_ref::<Self>().copied()
    }
}
impl fmt::Display for ArchiveNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Operation cancelled.",
            Self::ProfileExportUnconfirmed => "The daemon prepared the archive, but the local file is unconfirmed. Cancel the transfer and export again.",
            Self::GroupExportUnconfirmed => "The local file is unconfirmed. Cancel the transfer and export again.",
            Self::CancelUnconfirmed => "Transfer cancellation is unconfirmed. Read the result using the operation ID.",
            Self::InvalidArchiveFile => "Choose a regular archive file within the size limit.",
            Self::InvalidProfileArchive => "This is not an L-Connect Linux profile archive.",
            Self::InvalidManifestLength => "Invalid archive manifest length.",
            Self::DestinationExists => "The file already exists. Choose a new name.",
        })
    }
}
impl Error for ArchiveNotice {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_survives_error_context_and_unknown_diagnostics_never_acquire_a_code() {
        let error =
            anyhow::Error::from(ArchiveNotice::DestinationExists).context("Настройки {1}.lcp");
        assert_eq!(
            ArchiveNotice::from_error(&error),
            Some(ArchiveNotice::DestinationExists)
        );
        assert_eq!(error.to_string(), "Настройки {1}.lcp");
        let unknown = anyhow::anyhow!("The file already exists. Choose a new name.");
        assert_eq!(ArchiveNotice::from_error(&unknown), None);
        assert_eq!(
            serde_json::to_value(ArchiveNotice::DestinationExists).unwrap(),
            "destination_exists"
        );
    }
}
