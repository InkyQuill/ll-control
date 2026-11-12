//! Source Screen banks and payload inventory. No extraction, media decoding or Apply.
//! Runtime config JSON and editor setting JSON are deliberately distinct.
use crate::{read_selected, strict_json, Error, Group, Inspection, Limits};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, SeekFrom},
};
use zip::ZipArchive;

const EFFECTS: &[&str] = &[
    "Unknown",
    "ImageVideo",
    "BackgroundColor",
    "Rain",
    "Snow",
    "MeteorShower",
    "Fireworks",
    "TwistHole",
    "Kaleidoscope",
    "Mandala",
    "Blossom",
    "Transform",
    "TimeTunnel",
    "Whirlpool",
    "RadiantRainbow",
    "FlyInWording",
    "SnowWording",
    "Time",
    "Snow3D",
    "Robot",
    "PlanetCity",
    "Steampunk",
    "Atmosphere",
    "Radio",
];
const FIELDS: &[&str] = &[
    "ScreenBrightness",
    "ScreenRotation",
    "ScreenConfigJson",
    "ScreenProfileName",
    "ScreenImageFileInfos",
    "ScreenImageVideoFileInfos",
    "ScreenWatermarkFileInfos",
    "ScreenEffectType",
    "StyleExpressScreenEffectType",
    "ScreenProfiles",
];
/// These are host-side inspection budgets, not Windows format limits.
#[derive(Clone, Copy, Debug)]
pub struct ScreenLimits {
    pub container: Limits,
    pub json_bytes: usize,
    pub gallery_entries: usize,
    pub payload_bytes: u64,
    pub total_payload_bytes: u64,
}
impl Default for ScreenLimits {
    fn default() -> Self {
        Self {
            container: Limits::default(),
            json_bytes: 512 * 1024,
            gallery_entries: 256,
            payload_bytes: 256 * 1024 * 1024,
            total_payload_bytes: 1024 * 1024 * 1024,
        }
    }
}
/// Preserve exact JSON for future conversion and provenance, including inactive settings.
/// Empty strings select Windows defaults; they are not invented empty objects.
#[derive(Debug, Serialize)]
pub struct SourceJson {
    pub source_json: String,
    pub sha256: String,
    pub uses_default: bool,
}
/// Source array position. Index0 is current Full Screen; other entries are not
/// evidenced as named-profile slots in the LCDV2 2.1.28 UI.
#[derive(Debug, Serialize)]
pub struct ScreenSlot {
    pub index: usize,
    pub name: Option<String>,
    pub selected_effect: String,
    pub style_express_effect: String,
    /// Symbolic enum dictionary keys, each with the original ordered settings strings.
    pub editor_settings: BTreeMap<String, Vec<SourceJson>>,
}
#[derive(Debug, Serialize)]
pub struct Payload {
    pub entry: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct FileInfo {
    file_path: String,
    is_default_file: bool,
}
#[derive(Debug, Serialize)]
pub struct GalleryFile {
    /// Source reference only; never opened on the host.
    pub source_path: String,
    pub is_default_file: bool,
    /// Only flat uploaded assets can resolve into a Windows export. Default assets
    /// need a separate identity mapping even if their basename matches an upload.
    pub zip_entry: Option<String>,
    pub resolution: &'static str,
}
#[derive(Debug, Serialize)]
pub struct ScreenInspection {
    pub archive: Inspection,
    pub record_index: usize,
    pub brightness_percent: u8,
    pub rotation_degrees: u16,
    pub runtime_config: SourceJson,
    /// Historical field name; these eight source entries are not Linux named slots.
    pub slots: Vec<ScreenSlot>,
    pub images: Vec<GalleryFile>,
    pub image_video: Vec<GalleryFile>,
    pub watermarks: Vec<GalleryFile>,
    /// All payloads, not just gallery references: thumbnails and unused uploads survive.
    pub payloads: Vec<Payload>,
    pub unconverted_controller_fields: Vec<String>,
    pub group_import_supported: bool,
    pub apply_supported: bool,
    pub payload_streams_verified: bool,
    pub media_decoded: bool,
    pub theme_semantics_validated: bool,
}

pub fn inspect_windows_screen<R: Read + Seek>(
    reader: R,
    record: usize,
) -> Result<ScreenInspection, Error> {
    inspect_with_limits(reader, record, ScreenLimits::default())
}
/// Read all banks and hash bounded payload streams, without publishing any file.
pub fn inspect_with_limits<R: Read + Seek>(
    mut reader: R,
    record: usize,
    limits: ScreenLimits,
) -> Result<ScreenInspection, Error> {
    let (archive, selected) = read_selected(&mut reader, limits.container, Some(record))?;
    if archive.group != Group::Screen {
        return Err(Error::Invalid(
            "Screen inspection requires a Screen archive",
        ));
    }
    let selected = selected.ok_or(Error::Invalid("selected record is not an LCDV2 controller"))?;
    let object = selected
        .as_object()
        .ok_or(Error::Invalid("controller profile must be an object"))?;
    let field = |key: &str| {
        object
            .get(key)
            .cloned()
            .ok_or(Error::Invalid("missing LCDV2 Screen field"))
    };
    let brightness: u8 = serde_json::from_value(field("ScreenBrightness")?)?;
    if brightness > 100 {
        return Err(Error::Invalid("invalid Screen brightness"));
    }
    let rotation: u8 = serde_json::from_value(field("ScreenRotation")?)?;
    if rotation > 3 {
        return Err(Error::Invalid("invalid Screen rotation enum"));
    }
    let runtime: String = serde_json::from_value(field("ScreenConfigJson")?)?;
    let runtime_config = source_json(runtime, limits.json_bytes)?;
    let names: Vec<Option<String>> = serde_json::from_value(field("ScreenProfileName")?)?;
    let effects: Vec<u8> = serde_json::from_value(field("ScreenEffectType")?)?;
    let style: Vec<u8> = serde_json::from_value(field("StyleExpressScreenEffectType")?)?;
    let banks: Vec<BTreeMap<String, Vec<String>>> =
        serde_json::from_value(field("ScreenProfiles")?)?;
    if names.len() != 8 || effects.len() != 8 || style.len() != 8 || banks.len() != 8 {
        return Err(Error::Invalid(
            "Screen requires all eight slots and selected effect arrays",
        ));
    }
    let mut slots = Vec::new();
    for (index, (((name, effect), style_effect), bank)) in names
        .into_iter()
        .zip(effects)
        .zip(style)
        .zip(banks)
        .enumerate()
    {
        if name
            .as_ref()
            .is_some_and(|s| s.len() > 4096 || s.contains('\0'))
        {
            return Err(Error::Invalid("Screen slot name exceeds inspection budget"));
        }
        let mut settings = BTreeMap::new();
        for (effect, values) in bank {
            if !EFFECTS.contains(&effect.as_str()) || values.is_empty() || values.len() > 8 {
                return Err(Error::Invalid(
                    "invalid Screen editor bank key or settings count",
                ));
            }
            settings.insert(
                effect,
                values
                    .into_iter()
                    .map(|s| source_json(s, limits.json_bytes))
                    .collect::<Result<_, _>>()?,
            );
        }
        slots.push(ScreenSlot {
            index,
            name,
            selected_effect: effect_name(effect)?,
            style_express_effect: effect_name(style_effect)?,
            editor_settings: settings,
        });
    }
    let files = [
        "ScreenImageFileInfos",
        "ScreenImageVideoFileInfos",
        "ScreenWatermarkFileInfos",
    ]
    .into_iter()
    .map(|key| {
        let value: Vec<FileInfo> = serde_json::from_value(field(key)?)?;
        if value.len() > limits.gallery_entries {
            return Err(Error::Invalid("Screen gallery exceeds inspection budget"));
        }
        Ok(value)
    })
    .collect::<Result<Vec<_>, Error>>()?;

    // Re-run bounded container validation before opening payload streams; do not trust
    // a stale central directory after inspecting a caller-owned mutable reader.
    reader.seek(SeekFrom::Start(0))?;
    let (second, _) = read_selected(&mut reader, limits.container, None)?;
    if second.backup_sha256 != archive.backup_sha256 {
        return Err(Error::Invalid(
            "Windows backup changed during Screen inspection",
        ));
    }
    reader.seek(SeekFrom::Start(0))?;
    let mut zip = ZipArchive::new(reader)?;
    let backup = zip.by_name("backup")?;
    let mut bytes = Vec::new();
    backup
        .take(limits.container.backup_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limits.container.backup_bytes
        || format!("{:x}", Sha256::digest(&bytes)) != archive.backup_sha256
    {
        return Err(Error::Invalid(
            "Windows backup changed during payload inspection",
        ));
    }
    let mut payloads = Vec::new();
    let mut entries = BTreeMap::new();
    let mut total = 0u64;
    for index in 0..zip.len() {
        let mut file = zip.by_index(index)?;
        let name = file.name()?.into_owned();
        if file.is_dir() || !name.starts_with("wmsq/") {
            continue;
        }
        let base = name.strip_prefix("wmsq/").unwrap_or_default();
        // Windows imports item.Name into one directory. Nested members can alias an
        // uploaded file; refuse those instead of silently flattening them.
        if base.contains('/') || !crate::valid_name(base.as_bytes(), base) {
            return Err(Error::Invalid(
                "Windows wmsq assets must have flat unique names",
            ));
        }
        if entries.insert(base.to_lowercase(), name.clone()).is_some() {
            return Err(Error::Invalid("ambiguous Windows payload basename"));
        }
        if file.size() > limits.payload_bytes {
            return Err(Error::Invalid("Screen payload exceeds inspection budget"));
        }
        total = total
            .checked_add(file.size())
            .filter(|size| *size <= limits.total_payload_bytes)
            .ok_or(Error::Invalid(
                "Screen payload total exceeds inspection budget",
            ))?;
        let expected = file.size();
        let mut hasher = Sha256::new();
        let mut actual = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            actual = actual
                .checked_add(count as u64)
                .filter(|size| *size <= limits.payload_bytes && *size <= expected)
                .ok_or(Error::Invalid(
                    "Screen payload expanded beyond declared size or budget",
                ))?;
            hasher.update(&buffer[..count]);
        }
        if actual != expected {
            return Err(Error::Invalid("truncated Screen payload"));
        }
        payloads.push(Payload {
            entry: name,
            bytes: actual,
            sha256: format!("{:x}", hasher.finalize()),
        });
    }
    payloads.sort_by(|a, b| a.entry.cmp(&b.entry));
    let mut galleries = files.into_iter().map(|files| gallery(files, &entries));
    Ok(ScreenInspection {
        archive,
        record_index: record,
        brightness_percent: brightness,
        rotation_degrees: u16::from(rotation) * 90,
        runtime_config,
        slots,
        images: galleries
            .next()
            .ok_or(Error::Invalid("missing image gallery"))??,
        image_video: galleries
            .next()
            .ok_or(Error::Invalid("missing image/video gallery"))??,
        watermarks: galleries
            .next()
            .ok_or(Error::Invalid("missing watermark gallery"))??,
        payloads,
        unconverted_controller_fields: object
            .keys()
            .filter(|key| !FIELDS.contains(&key.as_str()))
            .cloned()
            .collect(),
        group_import_supported: false,
        apply_supported: false,
        payload_streams_verified: true,
        media_decoded: false,
        theme_semantics_validated: false,
    })
}
fn source_json(source_json: String, max: usize) -> Result<SourceJson, Error> {
    if source_json.len() > max {
        return Err(Error::Invalid("Screen JSON exceeds inspection budget"));
    }
    if !source_json.is_empty() && !strict_json::parse(source_json.as_bytes())?.is_object() {
        return Err(Error::Invalid(
            "Screen configuration must be an object or empty default",
        ));
    }
    Ok(SourceJson {
        uses_default: source_json.is_empty(),
        sha256: format!("{:x}", Sha256::digest(source_json.as_bytes())),
        source_json,
    })
}
fn effect_name(id: u8) -> Result<String, Error> {
    EFFECTS
        .get(usize::from(id))
        .map(|s| (*s).to_owned())
        .ok_or(Error::Invalid("unknown Screen effect enum"))
}
fn gallery(
    files: Vec<FileInfo>,
    entries: &BTreeMap<String, String>,
) -> Result<Vec<GalleryFile>, Error> {
    let mut paths = BTreeSet::new();
    files
        .into_iter()
        .map(|file| {
            let path = &file.file_path;
            let base = path.rsplit(['/', '\\']).next().unwrap_or_default();
            if path.len() > 32768
                || path.chars().any(char::is_control)
                || path
                    .split(['/', '\\'])
                    .any(|part| matches!(part, "." | ".."))
                || !crate::valid_name(base.as_bytes(), base)
                || !paths.insert(path.to_lowercase())
            {
                return Err(Error::Invalid(
                    "invalid or duplicate Windows gallery reference",
                ));
            }
            let entry = if file.is_default_file {
                None
            } else {
                entries.get(&base.to_lowercase()).cloned()
            };
            let resolution = if file.is_default_file {
                "external_default"
            } else if entry.is_some() {
                "archive_payload"
            } else {
                "missing_payload"
            };
            Ok(GalleryFile {
                source_path: file.file_path,
                is_default_file: file.is_default_file,
                zip_entry: entry,
                resolution,
            })
        })
        .collect()
}
