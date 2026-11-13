//! Verified, device-free preparation of static Windows Screen payloads.
//! Holds normalized bytes privately; publication/key assignment remains actor-owned.
use crate::{
    screen::{Payload, ScreenInspection},
    Error, Limits,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Seek, SeekFrom},
};
use zip::ZipArchive;
#[derive(Debug, Serialize)]
pub struct PreparedImage {
    pub source_entry: String,
    pub source_sha256: String,
    pub normalized_sha256: String,
    pub width: u32,
    pub height: u32,
    pub normalized_bytes: usize,
    #[serde(skip)]
    pub png: Vec<u8>,
}
#[derive(Debug, Serialize)]
pub struct PreparedImages {
    pub images: Vec<PreparedImage>,
    pub deferred_entries: Vec<String>,
    pub source_backup_sha256: String,
    pub source_payloads_verified: bool,
    pub images_decoded: bool,
    pub assets_published: bool,
    pub complete_screen_group_supported: bool,
}
/// Revalidate the envelope and each selected member against an inspection from
/// the same owned source. No path in source JSON is ever opened on the host.
pub fn read_verified<R: Read + Seek, T>(
    mut reader: R,
    source: &ScreenInspection,
    entries: &BTreeSet<String>,
    max_bytes: usize,
    mut prepare: impl FnMut(&Payload, &[u8]) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    reader.seek(SeekFrom::Start(0))?;
    let current = crate::inspect_windows(&mut reader)?;
    if current.group != crate::Group::Screen
        || current.backup_sha256 != source.archive.backup_sha256
    {
        return Err(Error::Invalid(
            "Screen source backup changed before asset preparation",
        ));
    }
    if entries
        .iter()
        .any(|name| !source.payloads.iter().any(|p| &p.entry == name))
    {
        return Err(Error::Invalid(
            "asset entry absent from inspected Screen payloads",
        ));
    }
    reader.seek(SeekFrom::Start(0))?;
    let mut archive = ZipArchive::new(reader)?;
    let mut backup = Vec::new();
    archive
        .by_name("backup")?
        .take(Limits::default().backup_bytes.saturating_add(1) as u64)
        .read_to_end(&mut backup)?;
    if backup.len() > Limits::default().backup_bytes
        || format!("{:x}", Sha256::digest(&backup)) != source.archive.backup_sha256
    {
        return Err(Error::Invalid("Screen backup changed while opening assets"));
    }
    let mut output = Vec::new();
    for name in entries {
        let expected = source
            .payloads
            .iter()
            .find(|p| &p.entry == name)
            .ok_or(Error::Invalid("missing inspected payload"))?;
        if expected.bytes > max_bytes as u64 {
            return Err(Error::Invalid(
                "Screen image payload exceeds encoded preparation limit",
            ));
        }
        let file = archive.by_name(name)?;
        if file.is_dir() || file.size() != expected.bytes {
            return Err(Error::Invalid("Screen payload identity changed"));
        }
        let mut bytes = Vec::new();
        file.take(max_bytes.saturating_add(1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > max_bytes
            || bytes.len() as u64 != expected.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != expected.sha256
        {
            return Err(Error::Invalid("Screen payload fingerprint changed"));
        }
        output.push(prepare(expected, &bytes)?);
    }
    Ok(output)
}
/// Decode/normalize PNG and JPEG uploads, including thumbnails/unused images.
/// GIF/video/unknown entries stay explicit, never flattened to a still image.
pub fn prepare_images<R: Read + Seek>(
    reader: R,
    source: &ScreenInspection,
) -> Result<PreparedImages, Error> {
    let mut entries = BTreeSet::new();
    let mut deferred_entries = Vec::new();
    for payload in &source.payloads {
        let ext = payload
            .entry
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
            entries.insert(payload.entry.clone());
        } else {
            deferred_entries.push(payload.entry.clone());
        }
    }
    let mut total = 0usize;
    let images = read_verified(
        reader,
        source,
        &entries,
        llcontrol_image::assets::MAX_ASSET_BYTES,
        |payload, bytes| {
            let png = bytes.starts_with(b"\x89PNG\r\n\x1a\n");
            let jpeg = bytes.starts_with(&[0xff, 0xd8, 0xff]);
            let extension = payload
                .entry
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
                .unwrap_or_default();
            if (extension == "png" && !png) || (extension != "png" && !jpeg) {
                return Err(Error::Invalid(
                    "Screen image content does not match PNG/JPEG extension",
                ));
            }
            if png {
                let mut offset = 8usize;
                while offset < bytes.len() {
                    let header = bytes
                        .get(offset..offset.saturating_add(8))
                        .ok_or(Error::Invalid("truncated PNG chunk"))?;
                    let len = u32::from_be_bytes(
                        header[..4]
                            .try_into()
                            .map_err(|_| Error::Invalid("invalid PNG chunk"))?,
                    ) as usize;
                    if &header[4..8] == b"acTL" {
                        return Err(Error::Invalid(
                            "animated PNG requires a separate media preparation",
                        ));
                    }
                    offset = offset
                        .checked_add(len)
                        .and_then(|v| v.checked_add(12))
                        .filter(|v| *v <= bytes.len())
                        .ok_or(Error::Invalid("PNG chunk exceeds source bounds"))?;
                }
            }
            let normalized = llcontrol_image::assets::normalize_asset(bytes)?;
            total = total
                .checked_add(normalized.len())
                .filter(|v| *v <= 32 * 1024 * 1024)
                .ok_or(Error::Invalid(
                    "normalized Screen images exceed32MiB staging budget",
                ))?;
            let decoded = llcontrol_image::assets::decode_asset(&normalized)?;
            Ok(PreparedImage {
                source_entry: payload.entry.clone(),
                source_sha256: payload.sha256.clone(),
                normalized_sha256: format!("{:x}", Sha256::digest(&normalized)),
                width: decoded.width(),
                height: decoded.height(),
                normalized_bytes: normalized.len(),
                png: normalized,
            })
        },
    )?;
    Ok(PreparedImages {
        images,
        deferred_entries,
        source_backup_sha256: source.archive.backup_sha256.clone(),
        source_payloads_verified: true,
        images_decoded: true,
        assets_published: false,
        complete_screen_group_supported: false,
    })
}
