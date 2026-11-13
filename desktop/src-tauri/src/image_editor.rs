//! Bounded, read-only image preparation. Publication uses the existing import path.
use llcontrol_image::editor::{prepare_edited, SquareCrop, Transform};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub side: u32,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    #[serde(default)]
    pub filters: Filters,
    pub crop: Option<Crop>,
    pub transforms: Vec<Action>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    #[serde(default)]
    pub hue: f32,
    #[serde(default)]
    pub saturation: f32,
    #[serde(default)]
    pub brightness: f32,
    #[serde(default)]
    pub contrast: f32,
    #[serde(default)]
    pub sharpen: f32,
    #[serde(default)]
    pub blur: f32,
}
impl From<Filters> for llcontrol_image::editor_filters::ColorFilters {
    fn from(value: Filters) -> Self {
        Self {
            hue: value.hue,
            saturation: value.saturation,
            brightness: value.brightness,
            contrast: value.contrast,
            sharpen: value.sharpen,
            blur: value.blur,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prepared {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}
#[derive(Clone)]
pub struct Editor(Arc<Semaphore>);
impl Default for Editor {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(1)))
    }
}
impl Editor {
    /// No queue of large sources; a dropped caller keeps its worker permit until
    /// preparation ends. Inspect returns oriented dimensions and a bounded preview.
    pub async fn prepare(&self, bytes: Vec<u8>, edit: Option<Edit>) -> anyhow::Result<Prepared> {
        anyhow::ensure!(
            !bytes.is_empty() && bytes.len() <= llcontrol_image::assets::MAX_ASSET_BYTES,
            "Image must be within 16 MiB"
        );
        if let Some(edit) = &edit {
            anyhow::ensure!(
                edit.transforms.len() <= llcontrol_image::editor::MAX_TRANSFORMS,
                "Too many geometry actions"
            );
        }
        let permit = self.0.clone().try_acquire_owned()?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if let Some(edit) = edit {
                let transforms: Vec<_> = edit
                    .transforms
                    .into_iter()
                    .map(|a| match a {
                        Action::RotateLeft => Transform::RotateLeft,
                        Action::RotateRight => Transform::RotateRight,
                        Action::FlipHorizontal => Transform::FlipHorizontal,
                        Action::FlipVertical => Transform::FlipVertical,
                    })
                    .collect();
                let crop = edit.crop.map(|c| SquareCrop {
                    x: c.x,
                    y: c.y,
                    side: c.side,
                });
                Ok(Prepared {
                    width: 480,
                    height: 480,
                    png: prepare_edited(&bytes, crop, &transforms, edit.filters.into())?,
                })
            } else {
                let source = llcontrol_image::assets::decode_asset(&bytes)?;
                let (width, height) = (source.width(), source.height());
                drop(source);
                Ok(Prepared {
                    width,
                    height,
                    png: llcontrol_image::assets::normalize_asset(&bytes)?,
                })
            }
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn original_dimensions_survive_preview_and_invalid_edit_does_not_poison_worker() {
        let source = include_bytes!("../tests/fixtures/editor-wide.png").to_vec();
        let editor = Editor::default();
        let inspected = editor.prepare(source.clone(), None).await.unwrap();
        assert_eq!((inspected.width, inspected.height), (1920, 960));
        let preview = llcontrol_image::assets::decode_asset(&inspected.png).unwrap();
        assert_eq!((preview.width(), preview.height()), (960, 480));
        assert!(editor
            .prepare(
                source.clone(),
                Some(Edit {
                    filters: Default::default(),
                    crop: Some(Crop {
                        x: 1920,
                        y: 0,
                        side: 1
                    }),
                    transforms: vec![]
                })
            )
            .await
            .is_err());
        let prepared = editor
            .prepare(
                source,
                Some(Edit {
                    filters: Default::default(),
                    crop: Some(Crop {
                        x: 0,
                        y: 0,
                        side: 240,
                    }),
                    transforms: vec![Action::RotateRight],
                }),
            )
            .await
            .unwrap();
        assert_eq!((prepared.width, prepared.height), (480, 480));
        assert!(prepared.png.len() <= 1024 * 1024);
    }
    #[tokio::test]
    async fn busy_worker_rejects_new_source_without_queueing() {
        let editor = Editor::default();
        let permit = editor.0.clone().acquire_owned().await.unwrap();
        assert!(editor.prepare(vec![1], None).await.is_err());
        drop(permit);
        assert!(editor.prepare(vec![], None).await.is_err());
    }
}
