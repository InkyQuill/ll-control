//! Portable Linux profile container. Blocking, device-free operations; callers
//! must keep them off Tokio/HID workers. Transport admission belongs to the RPC layer.
//!
//! The manifest contains keys, never extraction paths. Payloads are copied to
//! private numbered staging files, hashed and fully decoded before publication.
//! This format does not claim compatibility with Windows L-Connect archives.
use crate::{
    media_assets::MediaAssetStore,
    media_file::{MediaFile, MediaFileWriter},
    profiles::ProfileStore,
    theme_assets::ThemeAssetStore,
    theme_fonts::ThemeFontStore,
};
use anyhow::{ensure, Context};
use llcontrol_ipc::{NamedProfile, ProfileContent, ProfileEdit, ProfileLibrary, ThemeLayer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
};

const MAGIC: &[u8; 8] = b"LCPROF01";
const MAX_MANIFEST: usize = 1024 * 1024;
const MAX_ARCHIVE: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 1024;
const BUFFER: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Image,
    Font,
    Media,
}
impl Kind {
    fn limit(self) -> u64 {
        match self {
            Self::Image => llcontrol_image::assets::MAX_ASSET_BYTES as u64,
            Self::Font => llcontrol_image::fonts::MAX_FONT_BYTES as u64,
            Self::Media => crate::media_file::MAX_MEDIA_BYTES + 8 * 1024 * 1024 + 12,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    kind: Kind,
    key: String,
    bytes: u64,
    sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u8,
    profile: NamedProfile,
    assets: Vec<Entry>,
}
fn dependencies(profile: &NamedProfile) -> BTreeSet<(Kind, String)> {
    let mut result = BTreeSet::new();
    match &profile.content {
        ProfileContent::Screen(theme) => {
            result.extend(
                theme
                    .image_assets()
                    .into_iter()
                    .map(|k| (Kind::Image, k.to_owned())),
            );
            result.extend(
                theme
                    .font_assets()
                    .into_iter()
                    .map(|k| (Kind::Font, k.to_owned())),
            );
            result.extend(
                theme
                    .video_assets()
                    .into_iter()
                    .map(|k| (Kind::Media, k.to_owned())),
            );
        }
        ProfileContent::ScreenMedia(key) => {
            result.insert((Kind::Media, key.clone()));
        }
        ProfileContent::Fan(_) | ProfileContent::Pump(_) => {}
    }
    result
}
impl Manifest {
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.version == 1, "unsupported profile archive version");
        self.profile.validate()?;
        validate_entries(&self.assets, &dependencies(&self.profile), 64 * 1024 * 1024)?;
        Ok(())
    }
}

/// Asset stores share their existing lifetime/cleanup gates with the daemon.
#[derive(Clone)]
pub struct ProfileArchives {
    images: ThemeAssetStore,
    fonts: ThemeFontStore,
    media: MediaAssetStore,
}
impl ProfileArchives {
    pub fn new(images: ThemeAssetStore, fonts: ThemeFontStore, media: MediaAssetStore) -> Self {
        Self {
            images,
            fonts,
            media,
        }
    }

    /// Create a complete snapshot. At most one image/font/AU and a fixed copy
    /// buffer are in memory; media and complete archives are staged on disk.
    /// A write error means the destination is incomplete and must be discarded.
    pub fn export(&self, profile: &NamedProfile, output: &mut impl Write) -> anyhow::Result<()> {
        profile.validate()?;
        let stage = tempfile::tempdir()?;
        let manifest = Manifest {
            version: 1,
            profile: profile.clone(),
            assets: self.stage_assets(&dependencies(profile), stage.path(), 64 * 1024 * 1024)?,
        };
        manifest.validate()?;
        write_container(MAGIC, &manifest, &manifest.assets, stage.path(), output)?;
        Ok(())
    }

    /// Export to a new file atomically. Never truncate an existing user file.
    pub fn export_file(&self, profile: &NamedProfile, path: &Path) -> anyhow::Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        self.export(profile, &mut temporary)?;
        temporary.as_file().sync_all()?;
        temporary.persist_noclobber(path).map_err(|e| e.error)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }

    /// Read only the validated manifest, not a claim that the payload is valid.
    /// Used to check the explicitly selected target group before publication.
    pub fn inspect(&self, input: &mut impl Read) -> anyhow::Result<NamedProfile> {
        Ok(read_manifest(input)?.profile)
    }

    /// Validate everything before importing any asset or replacing a slot.
    /// Slot group comes from the validated archive; the target slot is explicit.
    /// Saved bindings remain exact identities; importing does not apply hardware.
    /// Publication failures can leave valid unreferenced assets for later cleanup,
    /// but never publish an incomplete profile or overwrite an existing asset.
    pub fn import(
        &self,
        input: &mut impl Read,
        store: &ProfileStore,
        expected_revision: u64,
        slot: u8,
    ) -> anyhow::Result<ProfileLibrary> {
        self.import_inner(input, store, expected_revision, slot, None, None)
    }

    pub(crate) fn import_transfer_scoped(
        &self,
        input: &mut impl Read,
        store: &ProfileStore,
        receipt: llcontrol_ipc::ArchiveImportReceipt,
        publication: std::sync::Arc<crate::asset_publication::Publication>,
    ) -> anyhow::Result<ProfileLibrary> {
        ensure!(
            receipt.source_sha256.is_some(),
            "scoped slot import requires a source digest"
        );
        self.import_inner(
            input,
            store,
            receipt.expected_revision,
            receipt.slot,
            Some(receipt),
            Some(publication),
        )
    }
    fn import_inner(
        &self,
        input: &mut impl Read,
        store: &ProfileStore,
        expected_revision: u64,
        slot: u8,
        receipt: Option<llcontrol_ipc::ArchiveImportReceipt>,
        publication: Option<std::sync::Arc<crate::asset_publication::Publication>>,
    ) -> anyhow::Result<ProfileLibrary> {
        ensure!(
            store.list()?.revision == expected_revision,
            "profile library changed; reload before importing"
        );
        let PreparedSlot { profile, _assets } =
            self.prepare_slot_scoped(input, slot, receipt.as_ref(), publication.clone())?;
        if let Some(receipt) = receipt {
            if let Some(scope) = publication {
                scope.arm()?;
            }
            store.edit_import(expected_revision, profile, receipt)
        } else {
            store.edit(expected_revision, ProfileEdit::Save { profile })
        }
    }
    pub(crate) fn prepare_slot_scoped(
        &self,
        input: &mut impl Read,
        slot: u8,
        receipt: Option<&llcontrol_ipc::ArchiveImportReceipt>,
        publication: Option<std::sync::Arc<crate::asset_publication::Publication>>,
    ) -> anyhow::Result<PreparedSlot> {
        let private = publication.as_ref().and_then(|p| p.staging_directory());
        let temporary = if private.is_none() {
            Some(tempfile::tempdir()?)
        } else {
            None
        };
        let directory = private.unwrap_or_else(|| temporary.as_ref().unwrap().path());
        let mut manifest = read_staged(
            input,
            directory,
            slot,
            receipt.and_then(|r| r.source_sha256.as_deref()),
        )?;
        if let Some(receipt) = receipt {
            ensure!(
                manifest.profile.content.group() == receipt.group && receipt.slot == slot,
                "archive group differs from selected target group"
            );
        }
        manifest.profile.content.group().validate_slot(slot)?;
        manifest.profile.slot = slot;
        let published =
            self.publish_assets_scoped(&manifest.assets, directory, publication.as_ref())?;
        remap(&mut manifest.profile, &published.mapping)?;
        manifest.profile.validate()?;
        Ok(PreparedSlot {
            profile: manifest.profile,
            _assets: published,
        })
    }
    fn stage_assets(
        &self,
        dependencies: &BTreeSet<(Kind, String)>,
        directory: &Path,
        font_budget: u64,
    ) -> anyhow::Result<Vec<Entry>> {
        let mut total = 0u64;
        let mut font_total = 0u64;
        let mut entries = Vec::new();
        for (index, (kind, key)) in dependencies.iter().cloned().enumerate() {
            ensure!(index < MAX_ENTRIES, "too many archive assets");
            let path = directory.join(index.to_string());
            let mut name = None;
            match kind {
                Kind::Image => {
                    let _pin = self.images.acquire_reference(&key)?;
                    let (bytes, label) = self.images.read_for_archive(&key)?;
                    name = label;
                    fs::write(&path, bytes)?;
                }
                Kind::Font => {
                    fs::write(&path, self.fonts.load(&key)?)?;
                }
                Kind::Media => {
                    let mut source = self.media.acquire_source(&key)?;
                    let mut writer = MediaFileWriter::new(directory, source.name().to_owned())?;
                    for frame in 0..source.frames() {
                        writer.append(&source.read_unit(frame)?)?;
                    }
                    writer.finish(&path)?;
                }
            }
            let bytes = fs::metadata(&path)?.len();
            ensure!(
                (1..=kind.limit()).contains(&bytes),
                "archive asset exceeds byte budget"
            );
            total = total.checked_add(bytes).context("archive size overflow")?;
            ensure!(total <= MAX_ARCHIVE, "archive exceeds total byte budget");
            if kind == Kind::Font {
                font_total += bytes;
                ensure!(
                    font_total <= font_budget,
                    "archive fonts exceed byte budget"
                );
            }
            let digest = copy_hashed(&mut fs::File::open(&path)?, &mut std::io::sink(), bytes)?;
            entries.push(Entry {
                kind,
                key,
                bytes,
                sha256: digest,
                name,
            });
        }

        Ok(entries)
    }

    fn publish_assets_scoped(
        &self,
        entries: &[Entry],
        directory: &Path,
        publication: Option<&std::sync::Arc<crate::asset_publication::Publication>>,
    ) -> anyhow::Result<PublishedAssets> {
        use crate::asset_publication::CreatedKind;
        let mut mapping = BTreeMap::new();
        // Pins protect new images/media until the library commit finishes.
        let mut images = Vec::new();
        let mut media = Vec::new();
        for (index, e) in entries.iter().enumerate() {
            let path = directory.join(index.to_string());
            let imported = match e.kind {
                Kind::Image => {
                    let bytes = fs::read(&path)?;
                    let key = match publication {
                        Some(scope) => self.images.import_named_in(
                            &bytes,
                            e.name.as_deref(),
                            scope.staging_directory(),
                            |key, name, file| {
                                scope.created(
                                    if name {
                                        CreatedKind::ImageName
                                    } else {
                                        CreatedKind::Image
                                    },
                                    key,
                                    file,
                                )
                            },
                        )?,
                        None => self.images.import_named(&bytes, e.name.as_deref())?,
                    };
                    images.push(self.images.acquire_reference(&key)?);
                    key
                }
                Kind::Font => match publication {
                    Some(scope) => self.fonts.import_observed_in(
                        &fs::read(&path)?,
                        scope.staging_directory(),
                        |key, file| scope.created(CreatedKind::Font, key, file),
                    )?,
                    None => self.fonts.import(&fs::read(&path)?)?,
                },
                Kind::Media => {
                    let mut source = MediaFile::open(&path)?;
                    let mut writer = self.media.create_file(source.name().to_owned())?;
                    for frame in 0..source.frames() {
                        writer.append(&source.read_unit(frame)?)?;
                    }
                    let key = match publication {
                        Some(scope) => {
                            self.media
                                .publish_file_observed_in(
                                    writer,
                                    scope.staging_directory(),
                                    |key, file| scope.created(CreatedKind::Media, key, file),
                                )?
                                .key
                        }
                        None => self.media.publish_file(writer)?.key,
                    };
                    media.push(self.media.acquire_source(&key)?.pin());
                    key
                }
            };
            mapping.insert((e.kind, e.key.clone()), imported);
        }

        Ok(PublishedAssets {
            publication: publication.cloned(),
            mapping,
            _images: images,
            _media: media,
        })
    }
}

fn read_manifest(input: &mut impl Read) -> anyhow::Result<Manifest> {
    let header = read_header(input, MAGIC)?;
    let manifest: Manifest = serde_json::from_slice(&header)?;
    manifest.validate()?;
    Ok(manifest)
}

fn read_staged(
    input: &mut impl Read,
    directory: &Path,
    slot: u8,
    source: Option<&str>,
) -> anyhow::Result<Manifest> {
    let header = read_header(input, MAGIC)?;
    if let Some(expected) = source {
        ensure!(
            format!("{:x}", Sha256::digest(&header)) == expected,
            "slot archive source digest mismatch"
        );
    }
    let manifest: Manifest = serde_json::from_slice(&header)?;
    manifest.validate()?;
    manifest.profile.content.group().validate_slot(slot)?;
    read_payloads(input, directory, &manifest.assets)?;
    Ok(manifest)
}
fn copy_hashed(
    input: &mut impl Read,
    output: &mut impl Write,
    bytes: u64,
) -> anyhow::Result<String> {
    let mut remaining = bytes;
    let mut buffer = [0u8; BUFFER];
    let mut hash = Sha256::new();
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(BUFFER as u64))?;
        input.read_exact(&mut buffer[..limit])?;
        hash.update(&buffer[..limit]);
        output.write_all(&buffer[..limit])?;
        remaining -= limit as u64;
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn mapped(
    key: &mut String,
    kind: Kind,
    mapping: &BTreeMap<(Kind, String), String>,
) -> anyhow::Result<()> {
    *key = mapping
        .get(&(kind, key.clone()))
        .context("missing imported dependency")?
        .clone();
    Ok(())
}
fn remap(
    profile: &mut NamedProfile,
    mapping: &BTreeMap<(Kind, String), String>,
) -> anyhow::Result<()> {
    match &mut profile.content {
        ProfileContent::Screen(theme) => {
            // Validation forbids recursive caches; visit the active page plus its direct cache.
            remap_page(theme, mapping)?;
        }
        ProfileContent::ScreenMedia(key) => mapped(key, Kind::Media, mapping)?,
        ProfileContent::Fan(_) | ProfileContent::Pump(_) => {}
    }
    Ok(())
}
fn remap_page(
    theme: &mut llcontrol_ipc::LcdTheme,
    mapping: &BTreeMap<(Kind, String), String>,
) -> anyhow::Result<()> {
    if let Some(image) = &mut theme.background_image {
        mapped(&mut image.asset, Kind::Image, mapping)?;
    }
    if let Some(key) = &mut theme.background_video {
        mapped(key, Kind::Media, mapping)?;
    }
    for layer in &mut theme.layers {
        let style = match layer {
            ThemeLayer::Image { asset, .. } => {
                mapped(asset, Kind::Image, mapping)?;
                continue;
            }
            ThemeLayer::Text { style, .. } | ThemeLayer::Clock { style, .. } => style,
            ThemeLayer::Sensor { style, parts, .. }
            | ThemeLayer::SensorCycle { style, parts, .. } => {
                if let Some(parts) = parts {
                    for part in [&mut parts.label, &mut parts.unit] {
                        if let Some(key) = &mut part.font_asset {
                            mapped(key, Kind::Font, mapping)?;
                        }
                    }
                }
                style
            }
            ThemeLayer::Divider { .. }
            | ThemeLayer::Weather { .. }
            | ThemeLayer::Meteor { .. }
            | ThemeLayer::Snow3d { .. }
            | ThemeLayer::Fireworks { .. } => continue,
        };
        if let Some(key) = &mut style.font_asset {
            mapped(key, Kind::Font, mapping)?;
        }
    }
    if let Some(editor) = &mut theme.basic_editor {
        for bank in &mut editor.banks {
            for item in &mut bank.overlays {
                remap_page(&mut item.theme, mapping)?;
            }
        }
    }
    for cached in &mut theme.page_cache {
        remap_page(&mut cached.theme, mapping)?;
    }
    Ok(())
}

fn validate_entries(
    entries: &[Entry],
    expected: &BTreeSet<(Kind, String)>,
    font_budget: u64,
) -> anyhow::Result<()> {
    ensure!(entries.len() <= MAX_ENTRIES, "too many archive assets");
    let mut actual = BTreeSet::new();
    let mut total = 0u64;
    let mut font_total = 0u64;
    for e in entries {
        ensure!(
            actual.insert((e.kind, e.key.clone())),
            "duplicate archive asset"
        );
        ensure!(
            (1..=e.kind.limit()).contains(&e.bytes),
            "archive asset exceeds byte budget"
        );
        ensure!(
            e.sha256.len() == 64
                && e.sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "invalid archive digest"
        );
        if let Some(name) = &e.name {
            ensure!(
                e.kind == Kind::Image
                    && !name.trim().is_empty()
                    && name.len() <= 256
                    && !name.chars().any(char::is_control),
                "invalid archive image name"
            );
        }
        total = total
            .checked_add(e.bytes)
            .context("archive size overflow")?;
        ensure!(total <= MAX_ARCHIVE, "archive exceeds total byte budget");
        if e.kind == Kind::Font {
            font_total += e.bytes;
        }
    }
    ensure!(
        font_total <= font_budget,
        "archive fonts exceed byte budget"
    );
    ensure!(
        actual == *expected,
        "missing or unreferenced archive dependency"
    );
    Ok(())
}

fn write_container(
    magic: &[u8; 8],
    manifest: &impl Serialize,
    entries: &[Entry],
    directory: &Path,
    output: &mut impl Write,
) -> anyhow::Result<()> {
    let header = serde_json::to_vec(manifest)?;
    ensure!(header.len() <= MAX_MANIFEST, "archive manifest too large");
    output.write_all(magic)?;
    output.write_all(&u32::try_from(header.len())?.to_le_bytes())?;
    output.write_all(&Sha256::digest(&header))?;
    output.write_all(&header)?;
    for (index, e) in entries.iter().enumerate() {
        let actual = copy_hashed(
            &mut fs::File::open(directory.join(index.to_string()))?,
            output,
            e.bytes,
        )?;
        ensure!(actual == e.sha256, "archive staging changed");
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct PreparedSlot {
    pub profile: NamedProfile,
    _assets: PublishedAssets,
}
#[derive(Debug)]
struct PublishedAssets {
    publication: Option<std::sync::Arc<crate::asset_publication::Publication>>,
    mapping: BTreeMap<(Kind, String), String>,
    _images: Vec<crate::theme_assets::ImagePin>,
    _media: Vec<crate::media_assets::MediaPin>,
}

fn read_header(input: &mut impl Read, magic: &[u8; 8]) -> anyhow::Result<Vec<u8>> {
    let mut prefix = [0u8; 12];
    input.read_exact(&mut prefix)?;
    ensure!(&prefix[..8] == magic, "not a Linux profile archive");
    let length = u32::from_le_bytes(prefix[8..].try_into()?) as usize;
    ensure!(
        (1..=MAX_MANIFEST).contains(&length),
        "archive manifest exceeds byte budget"
    );
    let mut expected = [0u8; 32];
    input.read_exact(&mut expected)?;
    let mut header = vec![0; length];
    input.read_exact(&mut header)?;
    ensure!(
        Sha256::digest(&header).as_slice() == expected,
        "archive manifest checksum mismatch"
    );

    Ok(header)
}

fn read_payloads(input: &mut impl Read, directory: &Path, entries: &[Entry]) -> anyhow::Result<()> {
    for (index, e) in entries.iter().enumerate() {
        let path = directory.join(index.to_string());
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        ensure!(
            copy_hashed(input, &mut file, e.bytes)? == e.sha256,
            "archive asset checksum mismatch"
        );
        validate_staged_payload(&path, e.kind)?;
    }
    let mut tail = [0];
    ensure!(
        input.read(&mut tail)? == 0,
        "unexpected archive trailing bytes"
    );

    Ok(())
}

mod group;
pub use group::PreparedGroup;
pub(crate) use group::{StagedWindowsScreen, WindowsPreview};

fn validate_staged_payload(path: &Path, kind: Kind) -> anyhow::Result<()> {
    let mut file = fs::File::open(path)?;
    match kind {
        Kind::Image => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            let image = llcontrol_image::assets::decode_asset(&bytes)?;
            ensure!(
                image.width() <= 960 && image.height() <= 960,
                "archive image is not normalized"
            );
        }
        Kind::Font => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            llcontrol_image::fonts::validate_font(&bytes)?;
        }
        Kind::Media => {
            let mut reader = MediaFile::open(path)?;
            for frame in 0..reader.frames() {
                reader.read_unit(frame)?;
            }
        }
    }
    Ok(())
}
