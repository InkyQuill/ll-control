//! Device-free, read-only inspection of Windows L-Connect3 profile exports.
//!
//! Container/Screen inspection, RPM normalization and lighting-bank decoding never extract,
//! publish or apply assets. Target-bound FanAndLighting commit lives in the daemon;
//! Screen inventory hashes bounded payload streams; screen_assets separately prepares static images.
//! Cooling preparation requires an explicit source-record index; callers provide
//! Linux source bindings separately and choose the eventual target explicitly.
pub mod control;
pub mod cooling;
mod directory;
pub mod lighting;
pub mod linux_cooling;
pub mod linux_lighting;
pub mod linux_screen;
pub mod screen;
pub mod screen_assets;
pub mod screen_editor;
pub mod screen_media;
pub mod screen_plan;
pub mod screen_requirements;
mod strict_json;
mod wpf_color;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Seek},
};
use zip::{CompressionMethod, ZipArchive};

/// Windows ControllerTypeID.Galahad2LCDV2 in the frozen2.1.28 reference.
pub const LCDV2_TYPE_ID: u32 = 16_846_849;
/// Inspection bounds apply before indexing/decompression, including ZIP64 files.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub archive_bytes: u64,
    pub entries: usize,
    pub directory_bytes: u64,
    pub backup_bytes: usize,
    pub profile_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            archive_bytes: 8 * 1024 * 1024 * 1024,
            entries: 4096,
            directory_bytes: 4 * 1024 * 1024,
            backup_bytes: 16 * 1024 * 1024,
            profile_bytes: 8 * 1024 * 1024,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("cannot read profile archive: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot represent Windows settings as a Linux policy: {0}")]
    LinuxPolicy(#[from] lconnect_ipc::IpcError),
    #[error("cannot prepare Screen media: {0}")]
    Media(#[from] anyhow::Error),
    #[error("invalid Screen image: {0}")]
    Image(#[from] lconnect_image::Error),
    #[error("invalid ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("invalid profile JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid controller Data base64: {0}")]
    Base64(#[from] base64::DecodeError),
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    FanAndLighting,
    Screen,
}
#[derive(Debug, Clone, Serialize)]
pub struct ControllerSummary {
    pub record_index: usize,
    pub controller_type: u32,
    /// Only field names, never private device paths or user profile values.
    pub profile_fields: Vec<String>,
    pub profile_bytes: usize,
    pub profile_sha256: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Inspection {
    pub source_version: String,
    pub group: Group,
    pub created_at: Option<String>,
    pub total_records: usize,
    pub controllers: Vec<ControllerSummary>,
    pub wmsq_files: usize,
    pub zip_entries: usize,
    pub declared_bytes: u64,
    pub backup_sha256: String,
    pub inspection_only: bool,
    pub import_supported: bool,
    pub asset_payloads_validated: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Backup {
    version: String,
    created_at: Option<String>,
    #[serde(rename = "Type")]
    kind: u32,
    datas: Vec<Record>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Record {
    main_type: u32,
    sub_type: u32,
    // Intentionally discarded: never use a Windows hardware path as a Linux target.
    #[serde(rename = "Metadata", default)]
    _metadata: Value,
    data: String,
}
fn valid_name(raw: &[u8], name: &str) -> bool {
    !raw.contains(&0)
        && name.len() <= 1024
        && !name.contains(['\\', ':'])
        && !name.starts_with('/')
        && !name.chars().any(char::is_control)
        && name
            .trim_end_matches('/')
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
/// Inspect a seekable ZIP without connecting to a daemon or extracting files.
pub fn inspect_windows<R: Read + Seek>(reader: R) -> Result<Inspection, Error> {
    inspect_with_limits(reader, Limits::default())
}
/// As [`inspect_windows`], with caller-owned bounds for a preparation budget.
pub fn inspect_with_limits<R: Read + Seek>(reader: R, limits: Limits) -> Result<Inspection, Error> {
    read_selected(reader, limits, None).map(|(inspection, _)| inspection)
}

fn read_selected<R: Read + Seek>(
    mut reader: R,
    limits: Limits,
    selected_index: Option<usize>,
) -> Result<(Inspection, Option<Value>), Error> {
    let directory = directory::preflight(&mut reader, limits)?;
    let mut archive = ZipArchive::new(reader)?;
    if archive.len() as u64 != directory.entries
        || archive.offset() != 0
        || archive.central_directory_start() != directory.offset
        || !archive.comment().is_empty()
    {
        return Err(Error::Invalid(
            "ZIP directory disagrees with bounded preflight",
        ));
    }
    let mut names = BTreeSet::new();
    let mut backup_index = None;
    let mut wmsq_files = 0;
    let mut declared_bytes = 0u64;
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index)?;
        let name = file.name()?;
        if !valid_name(file.name_raw(), &name) || !names.insert(name.to_lowercase()) {
            return Err(Error::Invalid("unsafe or duplicate ZIP member name"));
        }
        if file.encrypted()
            || !matches!(
                file.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            )
            || file
                .unix_mode()
                .is_some_and(|mode| !matches!(mode & 0o170000, 0 | 0o100000 | 0o040000))
        {
            return Err(Error::Invalid(
                "unsupported encryption, compression or special ZIP member",
            ));
        }
        declared_bytes = declared_bytes
            .checked_add(file.size())
            .filter(|total| *total <= limits.archive_bytes)
            .ok_or(Error::Invalid(
                "declared ZIP content exceeds inspection budget",
            ))?;
        if name == "backup" {
            if file.is_dir() || file.size() == 0 || file.size() > limits.backup_bytes as u64 {
                return Err(Error::Invalid("backup entry exceeds inspection budget"));
            }
            backup_index = Some(index);
        } else if name.starts_with("wmsq/") && !file.is_dir() {
            wmsq_files += 1;
        }
    }
    let index = backup_index.ok_or(Error::Invalid("Windows ZIP has no backup entry"))?;
    let mut bytes = Vec::new();
    archive
        .by_index(index)?
        .take(limits.backup_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limits.backup_bytes {
        return Err(Error::Invalid("backup entry exceeded inspection budget"));
    }
    let backup: Backup = serde_json::from_value(strict_json::parse(&bytes)?)?;
    if backup.version != "2.1.28.0" {
        return Err(Error::Invalid(
            "unsupported Windows profile version; expected 2.1.28.0",
        ));
    }
    if backup
        .created_at
        .as_ref()
        .is_some_and(|value| value.len() > 64)
    {
        return Err(Error::Invalid(
            "archive timestamp exceeds inspection budget",
        ));
    }
    let group = match backup.kind {
        0 => Group::FanAndLighting,
        1 => Group::Screen,
        _ => return Err(Error::Invalid("unknown Windows profile group")),
    };
    if backup.datas.is_empty() || backup.datas.len() > 256 {
        return Err(Error::Invalid("invalid controller record count"));
    }
    let total_records = backup.datas.len();
    let mut controllers = Vec::new();
    let mut selected = None;
    for (record_index, record) in backup.datas.into_iter().enumerate() {
        if record.main_type != 0 || record.sub_type != LCDV2_TYPE_ID {
            continue;
        }
        if record.data.len()
            > limits
                .profile_bytes
                .saturating_add(2)
                .saturating_div(3)
                .saturating_mul(4)
        {
            return Err(Error::Invalid(
                "controller profile exceeds inspection budget",
            ));
        }
        let data = base64::engine::general_purpose::STANDARD.decode(record.data)?;
        if data.len() > limits.profile_bytes {
            return Err(Error::Invalid(
                "controller profile exceeds inspection budget",
            ));
        }
        let value = strict_json::parse(&data)?;
        let object = value
            .as_object()
            .ok_or(Error::Invalid("controller profile must be a JSON object"))?;
        if object.len() > 128 || object.keys().any(|key| key.len() > 128) {
            return Err(Error::Invalid(
                "controller field names exceed inspection budget",
            ));
        }
        if object.is_empty() {
            return Err(Error::Invalid("controller profile is empty"));
        }
        controllers.push(ControllerSummary {
            record_index,
            controller_type: record.sub_type,
            profile_fields: object.keys().cloned().collect(),
            profile_bytes: data.len(),
            profile_sha256: format!("{:x}", Sha256::digest(&data)),
        });
        if selected_index == Some(record_index) {
            selected = Some(value);
        }
    }
    if controllers.is_empty() {
        return Err(Error::Invalid(
            "archive has no GALAHAD II LCDV2 controller record",
        ));
    }
    Ok((
        Inspection {
            source_version: backup.version,
            group,
            created_at: backup.created_at,
            total_records,
            controllers,
            wmsq_files,
            zip_entries: archive.len(),
            declared_bytes,
            backup_sha256: format!("{:x}", Sha256::digest(&bytes)),
            inspection_only: true,
            import_supported: false,
            asset_payloads_validated: false,
        },
        selected,
    ))
}
