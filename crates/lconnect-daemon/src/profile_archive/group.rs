//! Full group container sharing the validated, bounded payload pipeline with slots.
use super::*;
use crate::profile_group::{GroupKind, GroupProfile, GroupSettings};
use lconnect_ipc::{ScreenBackgroundAsset, ScreenCollections};

const GROUP_MAGIC: &[u8; 8] = b"LCGROUP1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupManifest {
    version: u8,
    profile: GroupProfile,
    /// Complete user upload inventory, including files absent from gallery membership.
    uploads: Uploads,
    assets: Vec<Entry>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Uploads {
    images: BTreeSet<String>,
    media: BTreeSet<String>,
}
#[derive(Debug, PartialEq, Eq)]
struct Catalog {
    images: BTreeSet<String>,
    media: BTreeSet<String>,
}
impl Catalog {
    fn capture(stores: &ProfileArchives) -> anyhow::Result<Self> {
        stores.images.with_catalog(|images| {
            stores.media.with_catalog(|media| {
                Ok(Self {
                    images: images.into_iter().collect(),
                    media: media.into_iter().collect(),
                })
            })
        })
    }
    fn uploads(&self) -> Uploads {
        Uploads {
            images: self
                .images
                .iter()
                .filter(|key| lconnect_image::defaults::image(key).is_none())
                .cloned()
                .collect(),
            media: self
                .media
                .iter()
                .filter(|key| key.as_str() != lconnect_ipc::BUILTIN_VIDEO_KEY)
                .cloned()
                .collect(),
        }
    }
    fn legacy_collections(&self) -> anyhow::Result<ScreenCollections> {
        crate::screen_collections::legacy_collections(&self.images, &self.media)
    }
}

/// Validated/published assets stay pinned until their owner commits or drops
/// this preparation. Preparation alone changes no settings or named slots.
#[derive(Debug)]
pub struct PreparedGroup {
    source_sha256: String,
    profile: GroupProfile,
    uploads: Uploads,
    _assets: PublishedAssets,
}
impl PreparedGroup {
    pub(crate) fn control_reset(sensor: &str) -> anyhow::Result<Self> {
        let profile = crate::control_reset::profile(sensor)?;
        let source_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "lconnect-lcdv2-control-reset-v1",
                &profile
            ),)?)
        );
        Ok(Self {
            source_sha256,
            profile,
            uploads: Uploads::default(),
            _assets: PublishedAssets {
                publication: None,
                mapping: BTreeMap::new(),
                _images: vec![],
                _media: vec![],
            },
        })
    }
    pub(crate) fn windows_control(
        draft: &lconnect_profile::control::PreparedControl,
        target: &GroupProfile,
    ) -> anyhow::Result<Self> {
        let profile = crate::windows_control::profile(draft, target)?;
        let source_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&("lconnect-windows-control-v1", draft))?)
        );
        Ok(Self {
            source_sha256,
            profile,
            uploads: Uploads::default(),
            _assets: PublishedAssets {
                publication: None,
                mapping: BTreeMap::new(),
                _images: vec![],
                _media: vec![],
            },
        })
    }
    /// Stable identity of original contents before generated asset-key remapping.
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub fn profile(&self) -> &GroupProfile {
        &self.profile
    }
    pub(crate) fn font_exemptions(&self) -> BTreeSet<String> {
        self._assets
            .publication
            .as_ref()
            .map(|s| s.font_exemptions())
            .unwrap_or_default()
    }
    pub fn uploaded_images(&self) -> &BTreeSet<String> {
        &self.uploads.images
    }
    pub fn uploaded_media(&self) -> &BTreeSet<String> {
        &self.uploads.media
    }
}

impl GroupManifest {
    fn dependencies(&self) -> BTreeSet<(Kind, String)> {
        let mut keys = BTreeSet::new();
        for p in &self.profile.profiles {
            keys.extend(dependencies(p));
        }
        if let GroupSettings::Screen(group) = &self.profile.content {
            for theme in group.theme.iter().chain(
                group
                    .editor
                    .iter()
                    .filter_map(|editor| editor.theme.as_ref()),
            ) {
                add_theme(theme, &mut keys);
            }
            if let Some(editor) = &group.editor {
                keys.extend(
                    editor
                        .image_keys()
                        .into_iter()
                        .map(|key| (Kind::Image, key.to_owned())),
                );
                keys.extend(
                    editor
                        .video_keys()
                        .into_iter()
                        .map(|key| (Kind::Media, key.to_owned())),
                );
                keys.extend(
                    editor
                        .font_keys()
                        .into_iter()
                        .map(|key| (Kind::Font, key.to_owned())),
                );
            }
            if let Some(key) = &group.media_clip {
                keys.insert((Kind::Media, key.clone()));
            }
            if let Some(collections) = &group.collections {
                // Protected versioned defaults keep their identity in galleries.
                // Theme references to the same defaults still carry exact payloads.
                keys.extend(
                    collections
                        .image_keys()
                        .into_iter()
                        .filter(|key| !protected_image(key))
                        .map(|key| (Kind::Image, key.to_owned())),
                );
                keys.extend(
                    collections
                        .video_keys()
                        .into_iter()
                        .filter(|key| *key != lconnect_ipc::BUILTIN_VIDEO_KEY)
                        .map(|key| (Kind::Media, key.to_owned())),
                );
            }
        }
        keys.extend(
            self.uploads
                .images
                .iter()
                .cloned()
                .map(|key| (Kind::Image, key)),
        );
        keys.extend(
            self.uploads
                .media
                .iter()
                .cloned()
                .map(|key| (Kind::Media, key)),
        );
        keys
    }
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.version == 1, "unsupported group archive version");
        self.profile.validate()?;
        if self.profile.kind() == GroupKind::FanAndLighting {
            ensure!(
                self.uploads.images.is_empty() && self.uploads.media.is_empty(),
                "cooling group cannot carry screen uploads"
            );
        }
        for key in self.uploads.images.iter().chain(&self.uploads.media) {
            validate_upload_key(key)?;
        }
        if let GroupSettings::Screen(group) = &self.profile.content {
            ensure!(
                group.collections.is_some(),
                "group archive must materialize legacy collections"
            );
        }
        validate_entries(&self.assets, &self.dependencies(), MAX_ARCHIVE)?;
        let lengths: BTreeMap<_, _> = self
            .assets
            .iter()
            .filter(|e| e.kind == Kind::Font)
            .map(|e| (e.key.as_str(), e.bytes))
            .collect();
        let check_theme = |theme: &lconnect_ipc::LcdTheme| -> anyhow::Result<()> {
            let total: u64 = theme
                .font_assets()
                .iter()
                .map(|key| lengths.get(key).copied().unwrap_or(0))
                .sum();
            ensure!(
                total <= 64 * 1024 * 1024,
                "one group theme exceeds64MiB fonts"
            );
            Ok(())
        };
        if let GroupSettings::Screen(group) = &self.profile.content {
            for theme in group.theme.iter().chain(
                group
                    .editor
                    .iter()
                    .filter_map(|editor| editor.theme.as_ref()),
            ) {
                check_theme(theme)?;
            }
        }
        for p in &self.profile.profiles {
            if let ProfileContent::Screen(theme) = &p.content {
                check_theme(theme)?;
            }
        }
        Ok(())
    }
}
impl ProfileArchives {
    /// Snapshot every group dependency and user screen upload on disk. A raw
    /// writer error means incomplete output; discard it, as with slot export.
    pub fn export_group(
        &self,
        profile: &GroupProfile,
        output: &mut impl Write,
    ) -> anyhow::Result<()> {
        profile.validate()?;
        let catalog = if profile.kind() == GroupKind::Screen {
            Some(Catalog::capture(self)?)
        } else {
            None
        };
        let mut profile = profile.clone();
        if let GroupSettings::Screen(group) = &mut profile.content {
            if group.collections.is_none() {
                group.collections = Some(
                    catalog
                        .as_ref()
                        .context("missing screen catalog")?
                        .legacy_collections()?,
                );
            }
        }
        let mut manifest = GroupManifest {
            version: 1,
            profile,
            uploads: catalog
                .as_ref()
                .map_or_else(Uploads::default, Catalog::uploads),
            assets: Vec::new(),
        };
        let stage = tempfile::tempdir()?;
        manifest.assets = self.stage_assets(&manifest.dependencies(), stage.path(), MAX_ARCHIVE)?;
        manifest.validate()?;
        if let Some(catalog) = catalog {
            // Revalidate at the snapshot point without retaining catalog locks
            // while copying up to8GiB to an arbitrary caller-owned writer.
            self.images.with_catalog(|images| {
                self.media.with_catalog(|media| {
                    ensure!(
                        catalog
                            == Catalog {
                                images: images.into_iter().collect(),
                                media: media.into_iter().collect()
                            },
                        "screen upload catalog changed; export again"
                    );
                    Ok(())
                })
            })?;
        }
        write_container(
            GROUP_MAGIC,
            &manifest,
            &manifest.assets,
            stage.path(),
            output,
        )
    }

    pub fn export_group_file(&self, profile: &GroupProfile, path: &Path) -> anyhow::Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        self.export_group(profile, &mut temporary)?;
        temporary.as_file().sync_all()?;
        temporary.persist_noclobber(path).map_err(|e| e.error)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }
    /// Manifest inspection alone never confirms valid payload bytes.
    pub fn inspect_group(&self, input: &mut impl Read) -> anyhow::Result<GroupProfile> {
        Ok(read_group_manifest(input)?.0.profile)
    }
    /// Decode and validate all payloads before publishing any asset. Does not
    /// commit settings or Apply; keep the result alive through the owner commit.
    pub fn prepare_group(&self, input: &mut impl Read) -> anyhow::Result<PreparedGroup> {
        self.prepare_group_scoped(input, None)
    }
    pub(crate) fn prepare_group_scoped(
        &self,
        input: &mut impl Read,
        publication: Option<std::sync::Arc<crate::asset_publication::Publication>>,
    ) -> anyhow::Result<PreparedGroup> {
        let (manifest, source_sha256) = read_group_manifest(input)?;
        // Validate the complete post-remap shape (including manifest size) before
        // the first managed file is published. Generated keys all have32 bytes.
        let trial_mapping = manifest
            .assets
            .iter()
            .enumerate()
            .map(|(i, e)| ((e.kind, e.key.clone()), format!("{:032x}", i + 1)))
            .collect();
        remap_group(manifest.profile.clone(), &manifest.uploads, &trial_mapping)?;
        let private = publication.as_ref().and_then(|p| p.staging_directory());
        let temporary = if private.is_none() {
            Some(tempfile::tempdir()?)
        } else {
            None
        };
        let directory = private.unwrap_or_else(|| temporary.as_ref().unwrap().path());
        read_payloads(input, directory, &manifest.assets)?;
        let assets =
            self.publish_assets_scoped(&manifest.assets, directory, publication.as_ref())?;
        let (profile, uploads) = remap_group(manifest.profile, &manifest.uploads, &assets.mapping)?;
        Ok(PreparedGroup {
            source_sha256,
            profile,
            uploads,
            _assets: assets,
        })
    }
}
fn read_group_manifest(input: &mut impl Read) -> anyhow::Result<(GroupManifest, String)> {
    let header = read_header(input, GROUP_MAGIC)?;
    let source_sha256 = format!("{:x}", Sha256::digest(&header));
    let manifest: GroupManifest = serde_json::from_slice(&header)?;
    manifest.validate()?;
    Ok((manifest, source_sha256))
}
fn add_theme(theme: &lconnect_ipc::LcdTheme, keys: &mut BTreeSet<(Kind, String)>) {
    keys.extend(
        theme
            .image_assets()
            .into_iter()
            .map(|key| (Kind::Image, key.into())),
    );
    keys.extend(
        theme
            .font_assets()
            .into_iter()
            .map(|key| (Kind::Font, key.into())),
    );
    keys.extend(
        theme
            .video_assets()
            .into_iter()
            .map(|key| (Kind::Media, key.into())),
    );
}
fn protected_image(key: &str) -> bool {
    matches!(
        key,
        lconnect_ipc::BUILTIN_IMAGE_KEY | lconnect_ipc::BUILTIN_MARK_KEY
    )
}
fn validate_upload_key(key: &str) -> anyhow::Result<()> {
    ensure!(
        key.len() == 32
            && key
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "invalid group upload key"
    );
    Ok(())
}

fn remap_group(
    mut profile: GroupProfile,
    source_uploads: &Uploads,
    mapping: &BTreeMap<(Kind, String), String>,
) -> anyhow::Result<(GroupProfile, Uploads)> {
    for p in &mut profile.profiles {
        remap(p, mapping)?;
    }
    if let GroupSettings::Screen(group) = &mut profile.content {
        if let Some(theme) = &mut group.theme {
            remap_page(theme, mapping)?;
        }
        if let Some(theme) = group
            .editor
            .as_mut()
            .and_then(|editor| editor.theme.as_mut())
        {
            remap_page(theme, mapping)?;
        }
        if let Some(editor) = &mut group.editor {
            for asset in editor.resources.values_mut() {
                match asset {
                    ScreenBackgroundAsset::Image(key) => mapped(key, Kind::Image, mapping)?,
                    ScreenBackgroundAsset::Video(key) => mapped(key, Kind::Media, mapping)?,
                }
            }
        }
        if let Some(key) = &mut group.media_clip {
            mapped(key, Kind::Media, mapping)?;
        }
        let collections = group
            .collections
            .as_mut()
            .context("missing archive collections")?;
        for key in collections
            .images
            .iter_mut()
            .chain(&mut collections.watermarks)
        {
            if !protected_image(key) {
                mapped(key, Kind::Image, mapping)?;
            }
        }
        for bg in &mut collections.backgrounds {
            match bg {
                ScreenBackgroundAsset::Image(key) if !protected_image(key) => {
                    mapped(key, Kind::Image, mapping)?
                }
                ScreenBackgroundAsset::Video(key) if key != lconnect_ipc::BUILTIN_VIDEO_KEY => {
                    mapped(key, Kind::Media, mapping)?
                }
                _ => {}
            }
        }
    }
    let uploads = Uploads {
        images: source_uploads
            .images
            .iter()
            .cloned()
            .map(|mut key| {
                mapped(&mut key, Kind::Image, mapping)?;
                Ok(key)
            })
            .collect::<anyhow::Result<_>>()?,
        media: source_uploads
            .media
            .iter()
            .cloned()
            .map(|mut key| {
                mapped(&mut key, Kind::Media, mapping)?;
                Ok(key)
            })
            .collect::<anyhow::Result<_>>()?,
    };
    profile.validate()?;

    Ok((profile, uploads))
}

mod windows;
pub(crate) use windows::{StagedWindowsScreen, WindowsPreview};
