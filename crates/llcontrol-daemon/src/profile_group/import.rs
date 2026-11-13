//! Durable acknowledgement of a full-group storage commit, independent of Apply.
use super::*;

pub const MAX_GROUP_IMPORT_RECEIPTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupImportReceipt {
    pub id: String,
    pub kind: GroupKind,
    /// SHA-256 of the original archive header; its entries bind all payload hashes.
    pub source_sha256: String,
    pub expected: GroupRevision,
    pub profile_revision: u64,
    pub collection_revision: Option<u64>,
    /// Accepted desired group contents, never a claim of live hardware Apply.
    pub content_sha256: String,
}
impl GroupImportReceipt {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_operation_id(&self.id)?;
        validate_digest(&self.source_sha256)?;
        validate_digest(&self.content_sha256)?;
        validate_digest(&self.expected.settings_sha256)?;
        ensure!(
            self.expected.profile_revision.checked_add(1) == Some(self.profile_revision),
            "invalid group import profile revision"
        );
        ensure!(
            match self.kind {
                GroupKind::Screen => self.collection_revision.is_some_and(|r| r > 0),
                GroupKind::FanAndLighting => self.collection_revision.is_none(),
            },
            "invalid group import collection revision"
        );
        Ok(())
    }
    pub(crate) fn matches(&self, intent: &GroupImportIntent) -> bool {
        self.id == intent.id
            && self.kind == intent.kind
            && self.source_sha256 == intent.source_sha256
            && self.expected == intent.expected
    }
}

#[derive(Debug, Clone)]
pub struct GroupImportReadback {
    pub receipt: GroupImportReceipt,
    pub settings: DeviceSettings,
    pub profiles: ProfileLibrary,
    /// Later edits may change the selected group while retaining its receipt.
    pub selected_content_current: bool,
}
impl GroupImportReadback {
    pub(crate) fn new(
        receipt: GroupImportReceipt,
        settings: DeviceSettings,
        profiles: ProfileLibrary,
    ) -> anyhow::Result<Self> {
        receipt.validate()?;
        ensure!(
            receipt.profile_revision <= profiles.revision,
            "group import receipt refers to a missing profile revision"
        );
        let current = GroupProfile::capture(&settings, &profiles, receipt.kind)?;
        let selected_content_current = current.revision.settings_sha256 == receipt.content_sha256;
        Ok(Self {
            receipt,
            settings,
            profiles,
            selected_content_current,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GroupImportCommit {
    pub readback: GroupImportReadback,
    pub repeated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GroupImportError {
    #[error("{0}")]
    Rejected(String),
    #[error("{0}")]
    RecoveryRequired(String),
}

#[derive(Debug, Clone)]
pub(crate) struct GroupImportIntent {
    pub id: String,
    pub kind: GroupKind,
    pub source_sha256: String,
    pub expected: GroupRevision,
}
impl GroupImportIntent {
    pub fn new(
        id: String,
        expected: GroupRevision,
        prepared: &crate::profile_archive::PreparedGroup,
    ) -> anyhow::Result<Self> {
        validate_operation_id(&id)?;
        validate_digest(&expected.settings_sha256)?;
        validate_digest(prepared.source_sha256())?;
        Ok(Self {
            id,
            kind: prepared.profile().kind(),
            source_sha256: prepared.source_sha256().into(),
            expected,
        })
    }
}

pub(crate) fn validate_operation_id(id: &str) -> anyhow::Result<()> {
    ensure!(
        id.len() == 32
            && id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "group import id must be32 lowercase hexadecimal digits"
    );
    Ok(())
}
fn validate_digest(digest: &str) -> anyhow::Result<()> {
    ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "invalid group import digest"
    );
    Ok(())
}
