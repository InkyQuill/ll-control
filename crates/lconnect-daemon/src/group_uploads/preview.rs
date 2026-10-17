use super::*;
use lconnect_ipc::WindowsScreenPreviewSource;

pub(crate) struct PreviewContext {
    pub archives: ProfileArchives,
    pub sensors: Vec<crate::sensors::SensorCache>,
    pub display_unit: Arc<AtomicBool>,
    pub permit: crate::admission::WorkPermit,
}
impl GroupUploads {
    /// A detached owner retains entry/global/render admission through decoding
    /// and rasterization after RPC loss. Cancel/Finish share the entry barrier.
    pub(crate) async fn preview_windows(
        &self,
        dev: &DeviceId,
        id: &str,
        source: WindowsScreenPreviewSource,
        context: PreviewContext,
    ) -> Result<Vec<u8>> {
        let active = self.get(dev, FullProfileGroup::Screen, id).await?;
        if state(&active.0).phase != Phase::Prepared || active.0.windows.is_none() {
            return Err(invalid(
                "preview requires a privately prepared Windows Screen upload",
            ));
        }
        let permit = active
            .0
            .busy
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy("group upload operation busy"))?;
        let job = self
            .jobs
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy("group upload workers busy"))?;
        tokio::spawn(async move {
            let _active = active;
            let _permit = permit;
            let _job = job;
            let _render_permit = context.permit;
            let entry = _active.0.clone();
            let checked = entry.clone();
            let prepared = tokio::task::spawn_blocking(
                move || -> Result<crate::profile_archive::WindowsPreview> {
                    if checked.cancel.load(Ordering::Acquire)
                        || state(&checked).phase != Phase::Prepared
                    {
                        return Err(invalid("prepared preview was cancelled"));
                    }
                    let data = lock(&checked.data);
                    data.windows
                        .as_ref()
                        .ok_or_else(|| invalid("missing private Screen preparation"))?
                        .preview_input(
                            &context.archives,
                            context.sensors,
                            context.display_unit,
                            source,
                            &checked.cancel,
                        )
                        .map_err(invalid)
                },
            )
            .await
            .map_err(busy)??;
            let bytes = prepared.render().await.map_err(invalid)?;
            if entry.cancel.load(Ordering::Acquire) {
                return Err(invalid("prepared preview was cancelled"));
            }
            if bytes.len() > 1024 * 1024 {
                return Err(invalid("prepared preview exceeds byte budget"));
            }
            Ok(bytes)
        })
        .await
        .map_err(busy)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        persistence::DeviceSettings, profile_group::GroupProfile, windows_screen_fixture as fixture,
    };

    #[tokio::test]
    async fn lost_preview_caller_retains_render_capacity_and_cancel_joins_private_owner() {
        let root = tempfile::tempdir().unwrap();
        let store = crate::persistence::PersistenceStore::new_with_root(root.path().into());
        let archives = ProfileArchives::new(
            store.theme_assets(),
            store.theme_fonts(),
            store.media_assets(),
        );
        let profile = GroupProfile::capture(
            &DeviceSettings::default(),
            &Default::default(),
            GroupKind::Screen,
        )
        .unwrap();
        let target = GroupArchiveSnapshot {
            dev: DeviceId::new("preview-owner"),
            group: FullProfileGroup::Screen,
            expected: serde_json::to_string(&profile.revision).unwrap(),
            summary: crate::group_exports::summary(&profile.profile).unwrap(),
        };
        let source = lconnect_ipc::WindowsScreenSource {
            record: 0,
            bindings_json: fixture::bindings().to_string(),
            resources_json: fixture::mapping().to_string(),
        };
        let bytes = fixture::archive(|_| {}, &[]);
        let uploads = GroupUploads::new();
        let id = "41111111111111111111111111111111";
        uploads
            .begin_source(
                id.into(),
                target.clone(),
                bytes.len() as u64,
                Some((source, profile.profile)),
            )
            .await
            .unwrap();
        for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
            uploads
                .chunk(
                    &target.dev,
                    target.group,
                    id,
                    (index * CHUNK) as u64,
                    chunk.to_vec(),
                )
                .await
                .unwrap();
        }
        uploads
            .inspect_source(&target.dev, target.group, id, Some(archives.clone()))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if uploads
                    .status(&target.dev, target.group, id)
                    .await
                    .unwrap()
                    .phase
                    == Phase::Prepared
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let catalog = store.theme_assets().list().unwrap();
        let active = uploads.get(&target.dev, target.group, id).await.unwrap();
        let entry = active.0.clone();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let gate = tokio::task::spawn_blocking(move || {
            let _data = lock(&entry.data);
            started.send(()).unwrap();
            held.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        ready.await.unwrap();
        let render_slots = crate::admission::WorkSlots::new(1, Default::default());
        let context = PreviewContext {
            archives,
            sensors: vec![],
            display_unit: Arc::new(AtomicBool::new(false)),
            permit: render_slots.clone().try_acquire_owned().unwrap(),
        };
        let copy = uploads.clone();
        let dev = target.dev.clone();
        let caller = tokio::spawn(async move {
            copy.preview_windows(&dev, id, WindowsScreenPreviewSource::Scene, context)
                .await
        });
        tokio::time::timeout(Duration::from_secs(3), async {
            while active.0.busy.available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        assert_eq!(render_slots.available_permits(), 0);
        assert_eq!(
            uploads
                .cancel(&target.dev, target.group, id)
                .await
                .unwrap()
                .phase,
            Phase::Cancelling
        );
        // Cancellation may be observed before the renderer takes the data lock.
        // The private owner itself must still wait for that lock before cleanup.
        assert!(lock(&active.0.state).phase == Phase::Cancelling);
        release.send(()).unwrap();
        gate.await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if uploads
                    .status(&target.dev, target.group, id)
                    .await
                    .unwrap()
                    .phase
                    == Phase::Cancelled
                    && render_slots.available_permits() == 1
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(lock(&active.0.data).windows.is_none());
        assert_eq!(store.theme_assets().list().unwrap(), catalog);
    }
}
