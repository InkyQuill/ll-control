//! Owned Windows Screen resource-remap plan, before target validation/publication.
use crate::{
    linux_screen::{self, Bindings, PreparedRuntime, ResourceBinding},
    screen::ScreenInspection,
    screen_assets::{prepare_images, PreparedImages},
    screen_editor::{prepare_current_banks, PreparedBanks},
    screen_media::{prepare_media, PreparedMedia},
    strict_json, Error,
};
use llcontrol_ipc::{ScreenBackgroundAsset, ScreenCollections};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, SeekFrom},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Archive { entry: String },
    ExistingImage { key: String },
    ExistingVideo { key: String },
}
pub type ResourceMap = BTreeMap<String, Target>;
#[derive(Serialize)]
pub struct ScreenPlan {
    pub source_backup_sha256: String,
    pub source_record_index: usize,
    pub staged_resources: BTreeMap<String, ResourceBinding>,
    pub resolved_references: BTreeMap<String, ResourceBinding>,
    pub collections: ScreenCollections,
    pub runtime: Option<PreparedRuntime>,
    pub runtime_deferred: bool,
    pub editor_banks: PreparedBanks,
    /// Windows array index0 is the current Full Screen editor, not a named slot.
    pub current_editor_theme: Option<llcontrol_ipc::LcdTheme>,
    pub current_editor_deferred: bool,
    /// Retained source positions deliberately unassigned to Linux named slots.
    pub inactive_editor_indices: Vec<usize>,
    pub existing_images_to_verify: BTreeSet<String>,
    pub existing_videos_to_verify: BTreeSet<String>,
    pub existing_fonts_to_verify: BTreeSet<String>,
    pub unprepared_entries: Vec<String>,
    pub provisional_keys: bool,
    pub assets_published: bool,
    pub complete_screen_group_supported: bool,
    #[serde(skip)]
    pub images: PreparedImages,
    #[serde(skip)]
    pub media: PreparedMedia,
}
pub fn parse_resource_map(bytes: &[u8]) -> Result<ResourceMap, Error> {
    if bytes.len() > 64 * 1024 {
        return Err(Error::Invalid("Screen resource map exceeds64KiB"));
    }
    let map: ResourceMap = serde_json::from_value(strict_json::parse(bytes)?)?;
    if map.len() > 256
        || map.keys().any(|key| {
            key.trim().is_empty() || key.len() > 32768 || key.chars().any(char::is_control)
        })
    {
        return Err(Error::Invalid("invalid Screen source reference map"));
    }
    for target in map.values() {
        let valid = match target {
            Target::Archive { entry } => {
                entry.starts_with("wmsq/") && crate::valid_name(entry.as_bytes(), entry)
            }
            Target::ExistingImage { key } => {
                linux_screen::key_valid(key) || llcontrol_image::defaults::image(key).is_some()
            }
            Target::ExistingVideo { key } => linux_screen::key_valid(key),
        };
        if !valid {
            return Err(Error::Invalid("invalid Screen resource-map target"));
        }
    }
    Ok(map)
}
fn stage_key(backup: &str, entry: &str, digest: &str, kind: &str) -> String {
    let mut hash = Sha256::new();
    for field in ["llcontrol-windows-stage-v1", backup, entry, digest, kind] {
        hash.update(field.as_bytes());
        hash.update([0]);
    }
    format!("{:x}", hash.finalize())[..32].to_owned()
}
/// Inspect and prepare one owned source without blocking an async/UI executor.
pub fn inspect_and_prepare<R: Read + Seek>(
    reader: R,
    record: usize,
    bindings: &Bindings,
    map: &ResourceMap,
    cancel: &AtomicBool,
) -> Result<(ScreenInspection, ScreenPlan), Error> {
    let mut reader = crate::screen_media::Cancellable { reader, cancel };
    let source = crate::screen::inspect_windows_screen(&mut reader, record)?;
    let plan = prepare(reader, &source, bindings, map, cancel)?;
    Ok((source, plan))
}
/// All supplied aliases are explicit. No basename/path/URI guessing or Windows
/// default replacement is performed. Existing native resources still need pins.
pub fn prepare<R: Read + Seek>(
    mut reader: R,
    source: &ScreenInspection,
    bindings: &Bindings,
    map: &ResourceMap,
    cancel: &AtomicBool,
) -> Result<ScreenPlan, Error> {
    let bindings = linux_screen::parse_bindings(&serde_json::to_vec(bindings)?)?;
    let map = parse_resource_map(&serde_json::to_vec(map)?)?;
    if cancel.load(Ordering::Acquire) {
        return Err(Error::Invalid("Screen planning cancelled"));
    }
    let images = prepare_images(&mut reader, source)?;
    reader.seek(SeekFrom::Start(0))?;
    let media = prepare_media(&mut reader, source, cancel)?;
    let mut staged = BTreeMap::new();
    let mut keys = BTreeSet::new();
    for image in &images.images {
        let key = stage_key(
            &source.archive.backup_sha256,
            &image.source_entry,
            &image.normalized_sha256,
            "image",
        );
        if !keys.insert(key.clone()) {
            return Err(Error::Invalid("provisional resource key collision"));
        }
        staged.insert(
            image.source_entry.clone(),
            ResourceBinding::Image { asset: key },
        );
    }
    for video in &media.items {
        let key = stage_key(
            &source.archive.backup_sha256,
            &video.source_entry,
            &video.encoded_sha256,
            "video",
        );
        if !keys.insert(key.clone()) {
            return Err(Error::Invalid("provisional resource key collision"));
        }
        staged.insert(video.source_entry.clone(), ResourceBinding::Video { key });
    }
    let mut resolved = bindings.clone();
    if resolved.resources.values().any(|value| {
        keys.contains(match value {
            ResourceBinding::Image { asset } => asset,
            ResourceBinding::Video { key } => key,
        })
    }) {
        return Err(Error::Invalid(
            "existing resource collides with provisional key",
        ));
    }
    for (reference, target) in map {
        if resolved.resources.contains_key(&reference) {
            return Err(Error::Invalid(
                "source reference appears in both bindings and resource map",
            ));
        }
        let resource = match target {
            Target::Archive { entry } => staged
                .get(&entry)
                .cloned()
                .ok_or(Error::Invalid("mapped archive entry has not been prepared"))?,
            Target::ExistingImage { key } => {
                if keys.contains(&key) {
                    return Err(Error::Invalid(
                        "existing resource collides with provisional key",
                    ));
                }
                ResourceBinding::Image { asset: key }
            }
            Target::ExistingVideo { key } => {
                if keys.contains(&key) {
                    return Err(Error::Invalid(
                        "existing resource collides with provisional key",
                    ));
                }
                ResourceBinding::Video { key }
            }
        };
        resolved.resources.insert(reference, resource);
    }
    // Reuse binding admission, including managed-key/type constraints.
    let resolved = linux_screen::parse_bindings(&serde_json::to_vec(&resolved)?)?;
    let mut collections = ScreenCollections::defaults(1);
    let reference = |path: &str| {
        resolved.resources.get(path).ok_or(Error::Invalid(
            "gallery reference has no explicit prepared/native mapping",
        ))
    };
    let mut image_keys = BTreeSet::new();
    let mut background_keys = BTreeSet::new();
    let mut mark_keys = BTreeSet::new();
    for image in &source.images {
        let ResourceBinding::Image { asset } = reference(&image.source_path)? else {
            return Err(Error::Invalid("image gallery reference mapped to video"));
        };
        if !image_keys.insert(asset.clone()) {
            return Err(Error::Invalid(
                "source gallery aliases duplicate image membership",
            ));
        }
        append(&mut collections.images, asset.clone());
    }
    for file in &source.image_video {
        let asset = match reference(&file.source_path)? {
            ResourceBinding::Image { asset } => ScreenBackgroundAsset::Image(asset.clone()),
            ResourceBinding::Video { key } => ScreenBackgroundAsset::Video(key.clone()),
        };
        if !background_keys.insert(asset.clone()) {
            return Err(Error::Invalid(
                "source gallery aliases duplicate background membership",
            ));
        }
        append(&mut collections.backgrounds, asset);
    }
    for mark in &source.watermarks {
        let ResourceBinding::Image { asset } = reference(&mark.source_path)? else {
            return Err(Error::Invalid(
                "watermark gallery reference mapped to video",
            ));
        };
        if !mark_keys.insert(asset.clone()) {
            return Err(Error::Invalid(
                "source gallery aliases duplicate watermark membership",
            ));
        }
        append(&mut collections.watermarks, asset.clone());
    }
    collections.validate()?;
    let editor_banks = prepare_current_banks(source, &resolved)?;
    let current_editor = editor_banks
        .slots
        .first()
        .ok_or(Error::Invalid("missing current Windows Screen editor"))?;
    let current_editor_deferred = !matches!(
        current_editor.selected_effect.as_str(),
        "ImageVideo" | "BackgroundColor"
    );
    let current_editor_theme = if current_editor_deferred {
        None
    } else {
        Some(current_editor.native_theme()?)
    };
    let inactive_editor_indices = source
        .slots
        .iter()
        .skip(1)
        .map(|entry| entry.index)
        .collect();
    let current = &source.runtime_config.source_json;
    let (runtime, runtime_deferred) = if current.is_empty() {
        (None, true)
    } else {
        let document = strict_json::parse(current.as_bytes())?;
        match document
            .pointer("/Screens/0/Theme/Type")
            .and_then(|v| v.as_str())
        {
            Some("Image" | "Color") => (
                Some(linux_screen::prepare_runtime(current, &resolved)?),
                false,
            ),
            Some(_)
                if source.slots.first().is_some_and(|s| {
                    !matches!(s.selected_effect.as_str(), "ImageVideo" | "BackgroundColor")
                }) =>
            {
                (None, true)
            }
            _ => {
                return Err(Error::Invalid(
                    "runtime cannot be classified as basic or explicitly deferred",
                ))
            }
        }
    };
    let mut existing_images = collections
        .image_keys()
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut existing_videos = collections
        .video_keys()
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut fonts = BTreeSet::new();
    let themes = runtime.iter().map(|r| &r.theme).chain(
        editor_banks
            .slots
            .iter()
            .flat_map(|slot| slot.basic.values())
            .flat_map(|bank| bank.overlays.values())
            .map(|r| &r.theme),
    );
    for theme in themes {
        existing_images.extend(theme.image_assets().into_iter().map(str::to_owned));
        existing_videos.extend(theme.video_assets().into_iter().map(str::to_owned));
        fonts.extend(theme.font_assets().into_iter().map(str::to_owned));
    }
    existing_images.retain(|key| !keys.contains(key));
    existing_videos.retain(|key| !keys.contains(key));
    let unprepared_entries = source
        .payloads
        .iter()
        .filter(|p| !staged.contains_key(&p.entry))
        .map(|p| p.entry.clone())
        .collect();
    if cancel.load(Ordering::Acquire) {
        return Err(Error::Invalid("Screen planning cancelled"));
    }
    Ok(ScreenPlan {
        source_backup_sha256: source.archive.backup_sha256.clone(),
        source_record_index: source.record_index,
        resolved_references: resolved.resources,
        staged_resources: staged,
        collections,
        runtime,
        runtime_deferred,
        editor_banks,
        current_editor_theme,
        current_editor_deferred,
        inactive_editor_indices,
        existing_images_to_verify: existing_images,
        existing_videos_to_verify: existing_videos,
        existing_fonts_to_verify: fonts,
        unprepared_entries,
        provisional_keys: true,
        assets_published: false,
        complete_screen_group_supported: false,
        images,
        media,
    })
}
fn append<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}
