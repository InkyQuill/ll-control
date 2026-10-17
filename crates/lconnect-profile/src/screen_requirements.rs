//! Read-only binding requirements for a future Screen import form. No source
//! path is opened, no resource mapping is inferred, and no proposal leaves here.
use crate::{linux_screen::RuntimeRequirements, screen::ScreenInspection, Error};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PayloadChoice {
    pub entry: String,
    pub bytes: u64,
    /// Extension classification only; decoding still belongs to preparation.
    pub kind: &'static str,
}
#[derive(Debug, Clone, Serialize)]
pub struct ScreenRequirements {
    pub record_index: usize,
    pub backup_sha256: String,
    pub bindings: RuntimeRequirements,
    pub payloads: Vec<PayloadChoice>,
    pub deferred: Vec<&'static str>,
    pub payloads_decoded: bool,
    pub apply_supported: bool,
}
pub fn inspect(source: &ScreenInspection) -> Result<ScreenRequirements, Error> {
    let mut bindings = crate::screen_editor::inspect_current_requirements(source)?;
    let mut deferred = Vec::new();
    if source.runtime_config.source_json.is_empty() {
        deferred.push("empty_runtime_default_request");
    } else {
        let document = crate::strict_json::parse(source.runtime_config.source_json.as_bytes())?;
        if matches!(
            document
                .pointer("/Screens/0/Theme/Type")
                .and_then(|v| v.as_str()),
            Some("Image" | "Color")
        ) {
            bindings.merge(crate::linux_screen::inspect_runtime_requirements(
                &source.runtime_config.source_json,
            )?);
        } else {
            deferred.push("unsupported_runtime");
        }
    }
    if !source
        .slots
        .first()
        .is_some_and(|s| matches!(s.selected_effect.as_str(), "ImageVideo" | "BackgroundColor"))
    {
        deferred.push("unsupported_current_editor");
    }
    for (gallery, image_only) in [
        (&source.images, true),
        (&source.image_video, false),
        (&source.watermarks, true),
    ] {
        for item in gallery {
            let reference = &item.source_path;
            if reference.is_empty()
                || reference.len() > 32768
                || reference.chars().any(char::is_control)
            {
                return Err(Error::Invalid("invalid Screen gallery reference"));
            }
            *bindings.resources.entry(reference.clone()).or_default() |= image_only;
        }
    }
    if bindings.resources.len() > 256 {
        return Err(Error::Invalid(
            "combined Screen references exceed binding budget",
        ));
    }
    let payloads: Vec<_> = source
        .payloads
        .iter()
        .map(|payload| {
            let extension = payload
                .entry
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            PayloadChoice {
                entry: payload.entry.clone(),
                bytes: payload.bytes,
                kind: match extension.as_str() {
                    "png" | "jpg" | "jpeg" => "image",
                    "gif" | "mp4" => "video",
                    _ => "unsupported",
                },
            }
        })
        .collect();
    if payloads.iter().any(|p| p.kind == "unsupported") {
        deferred.push("unsupported_payload");
    }
    let result = ScreenRequirements {
        record_index: source.record_index,
        backup_sha256: source.archive.backup_sha256.clone(),
        bindings,
        payloads,
        deferred,
        payloads_decoded: false,
        apply_supported: false,
    };
    if serde_json::to_vec(&result)?.len() > 512 * 1024 {
        return Err(Error::Invalid("Screen requirements exceed512KiB"));
    }
    Ok(result)
}
