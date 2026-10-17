//! Renderer inputs from private preparation; no publication or device commands.
use super::*;
use crate::{sensors::SensorCache, theme_runtime::ThemeRuntime};
use lconnect_ipc::{H264AccessUnit, WindowsScreenPreviewSource};

pub(crate) struct WindowsPreview {
    runtime: Arc<ThemeRuntime>,
    first: Option<H264AccessUnit>,
}
impl WindowsPreview {
    pub(crate) async fn render(self) -> anyhow::Result<Vec<u8>> {
        match self.first {
            Some(unit) => crate::media_composition::preview_unit(self.runtime, unit, 0).await,
            None => Ok(self.runtime.frame_at(0).await?.to_vec()),
        }
    }
}
impl StagedWindowsScreen {
    pub(crate) fn preview_input(
        &self,
        archives: &ProfileArchives,
        sensors: Vec<SensorCache>,
        unit: Arc<AtomicBool>,
        source: WindowsScreenPreviewSource,
        cancel: &AtomicBool,
    ) -> anyhow::Result<WindowsPreview> {
        check_cancel(cancel)?;
        let GroupSettings::Screen(screen) = &self.manifest.profile.content else {
            anyhow::bail!("expected prepared Windows Screen")
        };
        let theme = match source {
            WindowsScreenPreviewSource::Scene => screen.theme.as_ref(),
            WindowsScreenPreviewSource::Editor => screen
                .editor
                .as_ref()
                .and_then(|editor| editor.theme.as_ref()),
        }
        .context("selected prepared scene is unavailable")?
        .clone();
        theme.validate()?;
        let entries = &self.manifest.assets[self.existing_count..];
        let staged: BTreeMap<_, _> = entries
            .iter()
            .enumerate()
            .map(|(index, e)| ((e.kind, e.key.as_str()), (index, e)))
            .collect();
        let keys = theme.image_assets();
        let mut assets = archives.images.load_keys(
            keys.iter()
                .copied()
                .filter(|key| !staged.contains_key(&(Kind::Image, *key))),
        )?;
        let mut total: u64 = assets
            .values()
            .map(|image| u64::from(image.width()) * u64::from(image.height()) * 4)
            .sum();
        for key in keys {
            check_cancel(cancel)?;
            let Some((index, entry)) = staged.get(&(Kind::Image, key)) else {
                continue;
            };
            let path = self.stage.path().join(index.to_string());
            let mut bytes = Vec::new();
            fs::File::open(&path)?
                .take(Kind::Image.limit() + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() as u64 == entry.bytes
                    && format!("{:x}", Sha256::digest(&bytes)) == entry.sha256,
                "private preview image changed"
            );
            let image = lconnect_image::assets::decode_asset(&bytes)?;
            ensure!(
                image.width() <= 960 && image.height() <= 960,
                "private preview image is not normalized"
            );
            total = total
                .checked_add(u64::from(image.width()) * u64::from(image.height()) * 4)
                .context("preview image budget overflow")?;
            ensure!(
                total <= 256 * 1024 * 1024,
                "preview exceeds aggregate decoded image budget"
            );
            assets.insert(key.into(), Arc::new(image));
        }
        let fonts = archives.fonts.load_theme(&theme)?;
        let first = if let Some(key) = &theme.background_video {
            if let Some((index, entry)) = staged.get(&(Kind::Media, key.as_str())) {
                let path = self.stage.path().join(index.to_string());
                ensure!(
                    fs::metadata(&path)?.len() == entry.bytes
                        && copy_hashed(
                            &mut fs::File::open(&path)?,
                            &mut std::io::sink(),
                            entry.bytes
                        )? == entry.sha256,
                    "private preview media changed"
                );
                Some(MediaFile::open(&path)?.read_unit(0)?)
            } else {
                Some(archives.media.acquire_source(key)?.read_unit(0)?)
            }
        } else {
            None
        };
        let rotation =
            lconnect_image::FrameRotation::try_from(screen.lcd.rotation.parse::<u16>()?)?;
        let runtime = Arc::new(
            ThemeRuntime::with_fonts(theme, assets, fonts, sensors)?
                .with_display_unit(unit)
                .with_output_rotation(rotation),
        );
        check_cancel(cancel)?;
        Ok(WindowsPreview { runtime, first })
    }
}
