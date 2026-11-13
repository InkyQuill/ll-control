//! Prepared collection updates retain assets until the settings owner saves intent.
use crate::{
    media_assets::{MediaAssetStore, MediaPin},
    theme_assets::{ImagePin, ThemeAssetStore},
};
use llcontrol_ipc::ScreenCollections;

#[derive(Debug)]
pub struct PreparedCollections {
    pub(crate) value: ScreenCollections,
    pub(crate) images: Vec<ImagePin>,
    pub(crate) media: Vec<MediaPin>,
}
impl PreparedCollections {
    /// Blocking validation: every member must exist before an update is queued.
    /// Caller supplies the next revision; owner still checks its current revision.
    pub fn prepare(
        value: ScreenCollections,
        images: &ThemeAssetStore,
        media: &MediaAssetStore,
    ) -> anyhow::Result<Self> {
        value.validate()?;
        let image_pins = value
            .image_keys()
            .into_iter()
            .map(|key| images.acquire_reference(key))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let media_pins = value
            .video_keys()
            .into_iter()
            .map(|key| media.acquire_source(key).map(|source| source.pin()))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self {
            value,
            images: image_pins,
            media: media_pins,
        })
    }
}

/// Legacy inventory snapshot, pinned during preparation and checked again under
/// both catalog locks at publication. Lock order is images -> media -> settings.
#[derive(Debug)]
pub struct PreparedMigration {
    pub(crate) prepared: PreparedCollections,
    image_keys: std::collections::BTreeSet<String>,
    media_keys: std::collections::BTreeSet<String>,
}
impl PreparedMigration {
    pub fn prepare(images: &ThemeAssetStore, media: &MediaAssetStore) -> anyhow::Result<Self> {
        let image_keys: std::collections::BTreeSet<_> = images.list()?.into_iter().collect();
        let media_keys: std::collections::BTreeSet<_> =
            media.list()?.into_iter().map(|clip| clip.key).collect();
        let value = legacy_collections(&image_keys, &media_keys)?;
        let prepared = PreparedCollections::prepare(value, images, media)?;
        Ok(Self {
            prepared,
            image_keys,
            media_keys,
        })
    }

    /// The outer error is a rejected snapshot; the inner result is the write
    /// outcome after acceptance (which may need retry after an I/O failure).
    pub(crate) fn commit(
        &self,
        images: &ThemeAssetStore,
        media: &MediaAssetStore,
        write: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<anyhow::Result<()>> {
        images.with_catalog(|image_keys| {
            media.with_catalog(|media_keys| {
                anyhow::ensure!(
                    image_keys
                        .into_iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        == self.image_keys
                        && media_keys
                            .into_iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            == self.media_keys,
                    "legacy catalog changed; prepare migration again"
                );
                Ok(write())
            })
        })
    }
}

/// Shared legacy membership rules for migration and full-group backup.
pub(crate) fn legacy_collections(
    image_keys: &std::collections::BTreeSet<String>,
    media_keys: &std::collections::BTreeSet<String>,
) -> anyhow::Result<ScreenCollections> {
    use llcontrol_ipc::{ImageGalleryContext as Context, ScreenBackgroundAsset};
    let mut value = ScreenCollections::defaults(1);
    for key in image_keys {
        let supports = |context| {
            llcontrol_image::defaults::image(key)
                .is_none_or(|asset| asset.contexts.contains(&context))
        };
        if supports(Context::Image) && !value.images.contains(key) {
            value.images.push(key.clone());
        }
        if supports(Context::Watermark) && !value.watermarks.contains(key) {
            value.watermarks.push(key.clone());
        }
        let background = ScreenBackgroundAsset::Image(key.clone());
        if supports(Context::Background) && !value.backgrounds.contains(&background) {
            value.backgrounds.push(background);
        }
    }
    for key in media_keys {
        let background = ScreenBackgroundAsset::Video(key.clone());
        if !value.backgrounds.contains(&background) {
            value.backgrounds.push(background);
        }
    }

    value.validate()?;
    Ok(value)
}
