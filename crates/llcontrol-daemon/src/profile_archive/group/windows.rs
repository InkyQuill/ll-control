//! Convert owned, source-verified Screen preparation into the native group
//! transaction. Existing resources retain identity; only new uploads publish.
use super::*;
use llcontrol_profile::{
    linux_screen::ResourceBinding, screen::ScreenInspection, screen_plan::ScreenPlan,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

fn check_cancel(cancel: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Windows Screen import cancelled"
    );
    Ok(())
}
fn source_entry<'a>(
    source: &'a ScreenInspection,
    entry: &str,
    digest: &str,
) -> anyhow::Result<&'a str> {
    let payload = source
        .payloads
        .iter()
        .find(|p| p.entry == entry && p.sha256 == digest)
        .context("prepared Screen payload differs from source inventory")?;
    let name = payload
        .entry
        .rsplit('/')
        .next()
        .context("missing Screen upload name")?;
    ensure!(
        !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
        "invalid Screen upload name"
    );
    Ok(name)
}
fn resource(
    resources: &BTreeMap<String, ResourceBinding>,
    entry: &str,
    kind: Kind,
) -> anyhow::Result<String> {
    let key = match (kind, resources.get(entry)) {
        (Kind::Image, Some(ResourceBinding::Image { asset })) => asset,
        (Kind::Media, Some(ResourceBinding::Video { key })) => key,
        _ => anyhow::bail!("prepared Screen payload has no matching typed resource"),
    };
    validate_upload_key(key)?;
    ensure!(
        key != llcontrol_ipc::BUILTIN_VIDEO_KEY,
        "reserved Screen upload key"
    );
    Ok(key.clone())
}
/// Owned, private preflight result. No target catalogs or settings are written.
/// The upload registry retains this value until explicit Finish or cancellation.
#[derive(Debug)]
pub(crate) struct StagedWindowsScreen {
    manifest: GroupManifest,
    source_sha256: String,
    stage: tempfile::TempDir,
    existing_count: usize,
    existing: PublishedAssets,
}
impl StagedWindowsScreen {
    pub(crate) fn profile(&self) -> &GroupProfile {
        &self.manifest.profile
    }
    pub(crate) fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    pub(crate) fn publish(
        mut self,
        archives: &ProfileArchives,
        publication: Option<Arc<crate::asset_publication::Publication>>,
        cancel: &AtomicBool,
    ) -> anyhow::Result<PreparedGroup> {
        let entries = &self.manifest.assets[self.existing_count..];
        // The private owner crosses an async preview boundary; verify its bytes
        // again before any target publication, then use the same typed mapping.
        for (index, entry) in entries.iter().enumerate() {
            check_cancel(cancel)?;
            let path = self.stage.path().join(index.to_string());
            ensure!(
                fs::metadata(&path)?.len() == entry.bytes
                    && copy_hashed(
                        &mut fs::File::open(&path)?,
                        &mut std::io::sink(),
                        entry.bytes
                    )? == entry.sha256,
                "private Screen staging changed"
            );
            validate_staged_payload(&path, entry.kind)?;
        }
        let mut assets =
            archives.publish_assets_scoped(entries, self.stage.path(), publication.as_ref())?;
        self.existing.mapping.append(&mut assets.mapping);
        assets.mapping = self.existing.mapping;
        assets._images.extend(self.existing._images);
        assets._media.extend(self.existing._media);
        check_cancel(cancel)?;
        let (profile, uploads) = remap_group(
            self.manifest.profile,
            &self.manifest.uploads,
            &assets.mapping,
        )?;
        Ok(PreparedGroup {
            source_sha256: self.source_sha256,
            profile,
            uploads,
            _assets: assets,
        })
    }
}
impl ProfileArchives {
    /// Blocking, device-free preparation. Does not commit settings or Apply.
    /// Keep the result alive until the serialized group owner commits it.
    pub fn prepare_windows_screen(
        &self,
        source: &ScreenInspection,
        plan: ScreenPlan,
        target: &GroupProfile,
        cancel: &AtomicBool,
    ) -> anyhow::Result<PreparedGroup> {
        self.prepare_windows_screen_scoped(source, plan, target, cancel, None)
    }

    pub(crate) fn prepare_windows_screen_scoped(
        &self,
        source: &ScreenInspection,
        plan: ScreenPlan,
        target: &GroupProfile,
        cancel: &AtomicBool,
        publication: Option<Arc<crate::asset_publication::Publication>>,
    ) -> anyhow::Result<PreparedGroup> {
        self.stage_windows_screen(source, plan, target, cancel)?
            .publish(self, publication, cancel)
    }

    pub(crate) fn stage_windows_screen(
        &self,
        source: &ScreenInspection,
        mut plan: ScreenPlan,
        target: &GroupProfile,
        cancel: &AtomicBool,
    ) -> anyhow::Result<StagedWindowsScreen> {
        check_cancel(cancel)?;
        target.validate()?;
        ensure!(
            target.kind() == GroupKind::Screen,
            "Windows Screen requires a Screen target"
        );
        ensure!(
            !plan.runtime_deferred
                && !plan.current_editor_deferred
                && plan.unprepared_entries.is_empty(),
            "Windows Screen contains deferred runtime, editor or uploads"
        );
        let editor = crate::screen_editor::StoredScreenEditor::from_prepared(source, &plan)?;
        let runtime = plan
            .runtime
            .take()
            .context("Windows Screen runtime is not prepared")?;
        let screen = crate::profile_group::ScreenGroup {
            lcd: crate::persistence::LcdSection {
                brightness: source.brightness_percent,
                rotation: source.rotation_degrees.to_string(),
                control_mode: "Application".into(),
            },
            theme: Some(runtime.theme),
            editor: Some(editor),
            media_clip: None,
            collections: Some(plan.collections.clone()),
        };
        // Target named profiles are not the eight source array positions.
        let source_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "llcontrol-windows-screen-v1",
                &screen,
                &plan.staged_resources,
            ))?)
        );
        let profile = GroupProfile {
            version: 1,
            content: GroupSettings::Screen(Box::new(screen)),
            profiles: target.profiles.clone(),
        };
        profile.validate()?;
        let stage = tempfile::tempdir()?;
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for image in &plan.images.images {
            check_cancel(cancel)?;
            let name = source_entry(source, &image.source_entry, &image.source_sha256)?;
            let key = resource(&plan.staged_resources, &image.source_entry, Kind::Image)?;
            ensure!(
                seen.insert(image.source_entry.clone()),
                "duplicate Screen payload"
            );
            ensure!(
                image.png.len() == image.normalized_bytes
                    && format!("{:x}", Sha256::digest(&image.png)) == image.normalized_sha256,
                "prepared Screen image changed"
            );
            let decoded = llcontrol_image::assets::decode_asset(&image.png)?;
            ensure!(
                decoded.width() == image.width
                    && decoded.height() == image.height
                    && image.width <= 960
                    && image.height <= 960,
                "invalid normalized Screen image dimensions"
            );
            let path = stage.path().join(entries.len().to_string());
            fs::write(&path, &image.png)?;
            entries.push(Entry {
                kind: Kind::Image,
                key,
                bytes: image.png.len() as u64,
                sha256: image.normalized_sha256.clone(),
                name: Some(name.into()),
            });
        }
        // Read one access unit at a time; retain private owners until staging ends.
        for item in &mut plan.media.items {
            check_cancel(cancel)?;
            let name = source_entry(source, &item.source_entry, &item.source_sha256)?;
            let key = resource(&plan.staged_resources, &item.source_entry, Kind::Media)?;
            ensure!(
                seen.insert(item.source_entry.clone()),
                "duplicate Screen payload"
            );
            ensure!(
                item.fps == 24 && item.frames == item.file.frames(),
                "prepared Screen video metadata changed"
            );
            let mut writer = MediaFileWriter::new(stage.path(), name.into())?;
            let mut hash = Sha256::new();
            let mut bytes = 0u64;
            for frame in 0..item.frames {
                check_cancel(cancel)?;
                let unit = item.file.read_unit(frame)?;
                bytes = bytes
                    .checked_add(unit.bytes.len() as u64)
                    .context("Screen video size overflow")?;
                hash.update(&unit.bytes);
                writer.append(&unit)?;
            }
            ensure!(
                bytes == item.encoded_bytes
                    && format!("{:x}", hash.finalize()) == item.encoded_sha256,
                "prepared Screen video changed"
            );
            let path = stage.path().join(entries.len().to_string());
            writer.finish(&path)?;
            let bytes = fs::metadata(&path)?.len();
            let sha256 = copy_hashed(&mut fs::File::open(&path)?, &mut std::io::sink(), bytes)?;
            entries.push(Entry {
                kind: Kind::Media,
                key,
                bytes,
                sha256,
                name: None,
            });
        }
        ensure!(
            seen == plan.staged_resources.keys().cloned().collect(),
            "unaccounted Screen staged resource"
        );
        ensure!(
            seen == source.payloads.iter().map(|p| p.entry.clone()).collect(),
            "unprepared Screen source payload"
        );
        let uploads = Uploads {
            images: entries
                .iter()
                .filter(|e| e.kind == Kind::Image)
                .map(|e| e.key.clone())
                .collect(),
            media: entries
                .iter()
                .filter(|e| e.kind == Kind::Media)
                .map(|e| e.key.clone())
                .collect(),
        };
        let mut manifest = GroupManifest {
            version: 1,
            profile,
            uploads,
            assets: vec![],
        };
        let new_keys: BTreeSet<_> = entries.iter().map(|e| (e.kind, e.key.clone())).collect();
        ensure!(
            new_keys.len() == entries.len(),
            "duplicate Screen staged key"
        );
        ensure!(
            !target
                .profiles
                .iter()
                .flat_map(dependencies)
                .any(|key| new_keys.contains(&key)),
            "Screen staged key collides with target named profile"
        );
        let existing: BTreeSet<_> = manifest
            .dependencies()
            .difference(&new_keys)
            .cloned()
            .collect();
        // Pin target dependencies through staging and the eventual owner commit.
        let mut images = Vec::new();
        let mut media = Vec::new();
        for (kind, key) in &existing {
            check_cancel(cancel)?;
            match kind {
                Kind::Image => images.push(self.images.acquire_reference(key)?),
                Kind::Media => media.push(self.media.acquire_source(key)?.pin()),
                Kind::Font => {
                    self.fonts.load(key)?;
                }
            }
        }
        let old_stage = tempfile::tempdir()?;
        manifest.assets = self.stage_assets(&existing, old_stage.path(), MAX_ARCHIVE)?;
        for (index, entry) in manifest.assets.iter().enumerate() {
            check_cancel(cancel)?;
            validate_staged_payload(&old_stage.path().join(index.to_string()), entry.kind)?;
        }
        let mapping: BTreeMap<_, _> = existing.iter().map(|k| (k.clone(), k.1.clone())).collect();
        // Trial remapping checks all typed dependencies and post-remap budgets
        // before publishing the first file. Never remap preserved target assets.
        let mut trial = mapping.clone();
        let mut used: BTreeSet<_> = existing.iter().map(|k| k.1.clone()).collect();
        let mut counter = 0u128;
        for entry in &entries {
            let key = loop {
                counter += 1;
                let key = format!("{counter:032x}");
                if used.insert(key.clone()) {
                    break key;
                }
            };
            trial.insert((entry.kind, entry.key.clone()), key);
        }
        manifest.assets.extend(entries);
        manifest.validate()?;
        ensure!(
            serde_json::to_vec(&manifest)?.len() <= MAX_MANIFEST,
            "Screen group manifest too large"
        );
        remap_group(manifest.profile.clone(), &manifest.uploads, &trial)?;
        check_cancel(cancel)?;
        Ok(StagedWindowsScreen {
            manifest,
            source_sha256,
            stage,
            existing_count: existing.len(),
            existing: PublishedAssets {
                publication: None,
                mapping,
                _images: images,
                _media: media,
            },
        })
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;

#[path = "windows_preview.rs"]
mod preview;
pub(crate) use preview::WindowsPreview;
