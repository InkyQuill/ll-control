//! One bounded, read-only editor runtime. Idle expiry releases managed asset pins.
use crate::theme_runtime::ThemeRuntime;
use lconnect_ipc::{DeviceId, LcdTheme};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct EditorCache {
    entry: Mutex<Option<Entry>>,
    cleanup_started: AtomicBool,
    video: tokio::sync::Mutex<Option<VideoEntry>>,
    last_frame: Mutex<Option<(String, u64, lconnect_image::theme::RgbImage)>>,
}
struct Entry {
    device: Option<DeviceId>,
    theme: LcdTheme,
    runtime: Arc<ThemeRuntime>,
    touched: Instant,
}
struct VideoEntry {
    key: String,
    decoder: crate::editor_video::EditorVideo,
    touched: Instant,
    gate: crate::admission::Admission,
}
impl EditorCache {
    pub async fn video_frame(
        &self,
        key: String,
        timeline: u64,
        store: crate::media_assets::MediaAssetStore,
        slots: crate::admission::WorkSlots,
    ) -> anyhow::Result<lconnect_image::theme::RgbImage> {
        if let Ok(last) = self.last_frame.lock() {
            if let Some((previous_key, previous_time, frame)) = last.as_ref() {
                if previous_key == &key && *previous_time == timeline {
                    return Ok(frame.clone());
                }
            }
        }
        let mut video = self.video.lock().await;
        if video.as_ref().is_some_and(|entry| entry.key != key) {
            *video = None;
        }
        if video.is_none() {
            let gate = slots.admission();
            let permit = slots.acquire_owned().await?;
            let source_key = key.clone();
            let source =
                tokio::task::spawn_blocking(move || store.acquire_source(&source_key)).await??;
            *video = Some(VideoEntry {
                key,
                decoder: crate::editor_video::EditorVideo::spawn(source, permit),
                touched: Instant::now(),
                gate,
            });
        }
        let entry = video.as_mut().expect("video initialized");
        entry.touched = Instant::now();
        match entry.decoder.first_frame().await {
            Ok(frame) => {
                if let Ok(mut last) = self.last_frame.lock() {
                    *last = Some((entry.key.clone(), timeline, (*frame).clone()));
                }
                Ok((*frame).clone())
            }
            Err(error) => {
                *video = None;
                Err(error)
            }
        }
    }
    pub fn get(&self, device: &Option<DeviceId>, theme: &LcdTheme) -> Option<Arc<ThemeRuntime>> {
        let mut guard = self.entry.lock().ok()?;
        let entry = guard.as_mut()?;
        if &entry.device != device || &entry.theme != theme {
            return None;
        }
        entry.touched = Instant::now();
        Some(entry.runtime.clone())
    }
    pub fn store(
        self: &Arc<Self>,
        device: Option<DeviceId>,
        theme: LcdTheme,
        runtime: Arc<ThemeRuntime>,
    ) {
        if let Ok(mut entry) = self.entry.lock() {
            *entry = Some(Entry {
                device,
                theme,
                runtime,
                touched: Instant::now(),
            });
        }
        if self.cleanup_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let Some(cache) = weak.upgrade() else {
                    return;
                };
                if let Ok(mut video) = cache.video.try_lock() {
                    if video.as_ref().is_some_and(|entry| {
                        entry.gate.is_closed() || entry.touched.elapsed() >= Duration::from_secs(2)
                    }) {
                        *video = None;
                    }
                }
                if let Ok(mut entry) = cache.entry.lock() {
                    if entry
                        .as_ref()
                        .is_some_and(|entry| entry.touched.elapsed() >= Duration::from_secs(30))
                    {
                        *entry = None;
                    }
                };
            }
        });
    }
}
