//! Public full-group Finish/readback; device mutations remain actor-owned.
use super::*;
use crate::{
    actor::{
        control_group::{ControlGroupCommand, ControlGroupState},
        screen_group::PreparedScreenGroup,
    },
    group_uploads::FinishSource,
    profile_group::{GroupImportReadback, GroupKind, GroupProfile, GroupRevision},
};
use lconnect_ipc::{
    FullGroupCommit, FullGroupReadback, FullGroupRuntime, FullProfileGroup as Group,
    GroupUploadPhase as Phase, GroupUploadState,
};
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "group",
    content = "revision",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum RuntimeToken {
    Screen(crate::actor::group_barrier::RuntimeRevision),
    FanAndLighting(crate::actor::control_group::ControlRevision),
}
fn screen_runtime(state: crate::actor::screen_group::ScreenGroupState) -> FullGroupRuntime {
    FullGroupRuntime {
        revision: serde_json::to_string(&RuntimeToken::Screen(state.revision)).ok(),
        last_error: state.lcd.last_error.clone(),
        lcd: Some(state.lcd),
        recovery_required: Some(state.recovery_required),
        ..Default::default()
    }
}
async fn retain_assets(publication: Arc<crate::asset_publication::Publication>) -> Result<()> {
    tokio::task::spawn_blocking(move || publication.retain())
        .await
        .map_err(lcd_closed)?
        .map_err(lcd_closed)
}
async fn retain_unknown_assets(
    publication: Arc<crate::asset_publication::Publication>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || publication.retain_unknown())
        .await
        .map_err(lcd_closed)?
        .map_err(lcd_closed)
}
async fn receipt(handles: &DeviceHandles, id: String) -> Result<Option<GroupImportReadback>> {
    let (reply, rx) = oneshot::channel();
    handles
        .settings_tx
        .send(SettingsUpdate::ReadGroupImport {
            operation_id: id,
            reply,
        })
        .await
        .map_err(lcd_closed)?;
    rx.await.map_err(lcd_closed)?.map_err(|e| match e {
        crate::profile_group::GroupImportError::Rejected(e) => IpcError::InvalidArgument(e),
        crate::profile_group::GroupImportError::RecoveryRequired(e) => lcd_closed(e),
    })
}
fn kind(group: Group) -> GroupKind {
    match group {
        Group::Screen => GroupKind::Screen,
        Group::FanAndLighting => GroupKind::FanAndLighting,
    }
}
fn control_runtime(state: ControlGroupState) -> FullGroupRuntime {
    FullGroupRuntime {
        revision: state
            .revision
            .and_then(|r| serde_json::to_string(&RuntimeToken::FanAndLighting(r)).ok()),
        cooling: state.cooling,
        thermal: state.lighting.as_ref().map(|s| s.thermal.clone()),
        dynamic: state.lighting.map(|s| s.dynamic),
        recovery_required: Some(state.recovery_required),
        last_error: state.last_error,
        lcd: None,
    }
}
async fn runtime(handles: &DeviceHandles, group: Group) -> Result<FullGroupRuntime> {
    match group {
        Group::Screen => {
            let (reply, rx) = oneshot::channel();
            handles
                .lcd_tx
                .send(LcdCommand::GroupState { reply })
                .await
                .map_err(lcd_closed)?;
            Ok(screen_runtime(rx.await.map_err(lcd_closed)?))
        }
        Group::FanAndLighting => {
            let (reply, rx) = oneshot::channel();
            handles
                .control_group_tx
                .send(ControlGroupCommand::State(reply))
                .await
                .map_err(lcd_closed)?;
            Ok(control_runtime(rx.await.map_err(lcd_closed)?))
        }
    }
}
fn project(
    dev: DeviceId,
    group: Group,
    readback: GroupImportReadback,
    runtime: FullGroupRuntime,
) -> Result<FullGroupReadback> {
    if readback.receipt.kind != kind(group) {
        return Err(IpcError::InvalidArgument(
            "receipt belongs to a different group".into(),
        ));
    }
    let profile = GroupProfile::capture(&readback.settings, &readback.profiles, kind(group))
        .map_err(lcd_closed)?;
    Ok(FullGroupReadback {
        dev,
        group,
        operation_id: readback.receipt.id.clone(),
        receipt: serde_json::to_string(&readback.receipt).map_err(lcd_closed)?,
        selected_content_current: readback.selected_content_current,
        summary: crate::group_exports::summary(&profile.profile)?,
        runtime,
    })
}
impl IpcServerImpl {
    pub(super) async fn control_group_reset(
        &self,
        dev: DeviceId,
        id: String,
        expected: String,
        sensor: String,
    ) -> Result<FullGroupCommit> {
        crate::profile_group::validate_operation_id(&id)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        if expected.len() > 1024 {
            return Err(IpcError::InvalidArgument(
                "group revision token too long".into(),
            ));
        }
        let expected: GroupRevision = serde_json::from_str(&expected)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let prepared = Arc::new(
            crate::profile_archive::PreparedGroup::control_reset(&sensor)
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))?,
        );
        self.commit_control_profile(dev, id, expected, prepared)
            .await
    }
    pub(super) async fn commit_control_profile(
        &self,
        dev: DeviceId,
        id: String,
        expected: GroupRevision,
        prepared: Arc<crate::profile_archive::PreparedGroup>,
    ) -> Result<FullGroupCommit> {
        let handles = self.handles(&dev).await?;
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        // Own admission and the coordinator reply through caller/connection loss.
        // No asset publication is needed: defaults contain no managed files.
        tokio::spawn(async move {
            let _permit = permit;
            if let Some(previous) = receipt(&handles, id.clone()).await? {
                if previous.receipt.kind != GroupKind::FanAndLighting
                    || previous.receipt.expected != expected
                    || previous.receipt.source_sha256 != prepared.source_sha256()
                {
                    return Err(IpcError::InvalidArgument(
                        "control operation id already has different parameters".into(),
                    ));
                }
                return Ok(FullGroupCommit {
                    readback: project(
                        dev,
                        Group::FanAndLighting,
                        previous,
                        runtime(&handles, Group::FanAndLighting).await?,
                    )?,
                    repeated: true,
                });
            }
            let (reply, rx) = oneshot::channel();
            handles
                .control_group_tx
                .send(ControlGroupCommand::Commit {
                    operation_id: id,
                    expected,
                    prepared,
                    reply,
                })
                .await
                .map_err(lcd_closed)?;
            let result = rx.await.map_err(lcd_closed)??;
            Ok(FullGroupCommit {
                readback: project(
                    dev,
                    Group::FanAndLighting,
                    result.commit.readback,
                    control_runtime(result.runtime),
                )?,
                repeated: result.commit.repeated,
            })
        })
        .await
        .map_err(lcd_closed)?
    }
    pub(super) async fn group_import_readback(
        &self,
        dev: DeviceId,
        group: Group,
        id: String,
    ) -> Result<FullGroupReadback> {
        crate::profile_group::validate_operation_id(&id)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let handles = self.handles(&dev).await?;
        let readback = match receipt(&handles, id.clone()).await? {
            Some(readback) => readback,
            None => {
                let store = self.persistence.clone();
                let key = handles.persistence_key.clone();
                let retained = tokio::task::spawn_blocking(move || {
                    store.import_recovery_notices().map(|notices| {
                        notices.iter().any(|n| {
                            n.operation == id && n.device_key == key && n.group == kind(group)
                        })
                    })
                })
                .await
                .map_err(lcd_closed)?
                .map_err(lcd_closed)?;
                return Err(IpcError::InvalidArgument(if retained {"Import commit is unconfirmed; its files were retained during recovery. Apply was not repeated."} else {"group import receipt unavailable"}.into()));
            }
        };
        project(dev, group, readback, runtime(&handles, group).await?)
    }
    pub(super) async fn group_import_reapply(
        &self,
        dev: DeviceId,
        group: Group,
        id: String,
        expected: String,
    ) -> Result<FullGroupReadback> {
        crate::profile_group::validate_operation_id(&id)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        if expected.len() > 4096 {
            return Err(IpcError::InvalidArgument("runtime token too long".into()));
        }
        let token: RuntimeToken = serde_json::from_str(&expected)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        if !matches!(
            (&token, group),
            (RuntimeToken::Screen(_), Group::Screen)
                | (RuntimeToken::FanAndLighting(_), Group::FanAndLighting)
        ) {
            return Err(IpcError::InvalidArgument(
                "runtime token belongs to a different group".into(),
            ));
        }
        let handles = self.handles(&dev).await?;
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.persistence.clone();
        let sensors = self.sensors.clone();
        let (result, _permit) = join_theme_job(permit, async move {
            match token {
                RuntimeToken::FanAndLighting(expected) => {
                    let slot = handles
                        .control_group_tx
                        .clone()
                        .reserve_owned()
                        .await
                        .map_err(lcd_closed)?;
                    let (reply, rx) = oneshot::channel();
                    slot.send(ControlGroupCommand::Reapply {
                        operation_id: id,
                        expected,
                        reply,
                    });
                    let result = rx.await.map_err(lcd_closed)??;
                    project(dev, group, result.readback, control_runtime(result.runtime))
                }
                RuntimeToken::Screen(expected) => {
                    let (reply, rx) = oneshot::channel();
                    handles
                        .lcd_tx
                        .send(LcdCommand::BeginScreenGroupReapply {
                            operation_id: id.clone(),
                            expected,
                            reply,
                        })
                        .await
                        .map_err(lcd_closed)?;
                    let admission = rx.await.map_err(lcd_closed)??;
                    let slot = handles
                        .lcd_tx
                        .clone()
                        .reserve_owned()
                        .await
                        .map_err(lcd_closed)?;
                    let prepared = crate::actor::screen_group::PreparedScreen::prepare(
                        &admission.readback.settings,
                        store,
                        vec![sensors, handles.sensor_cache],
                    )
                    .await?;
                    let (reply, rx) = oneshot::channel();
                    slot.send(LcdCommand::ReapplyScreenGroupChecked {
                        operation_id: id,
                        expected: admission.revision,
                        expected_generation: admission.generation,
                        preparation: admission.preparation,
                        prepared,
                        reply,
                    });
                    let result = rx.await.map_err(lcd_closed)??;
                    project(dev, group, result.readback, screen_runtime(result.runtime))
                }
            }
        })
        .await?;
        Ok(result)
    }
    pub(super) async fn finish_group_upload(
        &self,
        dev: DeviceId,
        group: Group,
        id: String,
    ) -> Result<GroupUploadState> {
        let mut current = self.group_uploads.status(&dev, group, &id).await?;
        if current.phase == Phase::Imported {
            // Repeated Finish reads current desired/runtime state and never
            // reapplies a saved payload over newer user intent.
            let readback = self.group_import_readback(dev, group, id).await?;
            current.committed = Some(FullGroupCommit {
                readback,
                repeated: true,
            });
            return Ok(current);
        }
        if matches!(current.phase, Phase::Importing | Phase::RecoveryRequired) {
            return Ok(current);
        }
        let handles = self.handles(&dev).await?;
        // Share existing render/preparation admission with ordinary LCD jobs;
        // a second endpoint must not bypass the daemon's runtime work budget.
        let runtime_permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.persistence.clone();
        let sensors = self.sensors.clone();
        let callback_dev = dev.clone();
        self.group_uploads
            .finish(&dev, group, &id, move |source: FinishSource| async move {
                let _runtime_permit = runtime_permit;
                let descriptor = source.descriptor();
                let id = descriptor.id;
                let expected: GroupRevision = serde_json::from_str(&descriptor.target.expected)
                    .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
                if let Some(previous) = receipt(&handles, id.clone()).await? {
                    if previous.receipt.kind != kind(group)
                        || previous.receipt.expected != expected
                        || Some(previous.receipt.source_sha256.as_str())
                            != descriptor.source_sha256.as_deref()
                    {
                        return Err(IpcError::InvalidArgument(
                            "group import id already has different parameters".into(),
                        ));
                    }
                    let runtime = runtime(&handles, group).await?;
                    return Ok(FullGroupCommit {
                        readback: project(callback_dev, group, previous, runtime)?,
                        repeated: true,
                    });
                }
                let archives = crate::profile_archive::ProfileArchives::new(
                    store.theme_assets(),
                    store.theme_fonts(),
                    store.media_assets(),
                );
                let binding = crate::asset_publication::journal::Binding {
                    device_key: handles.persistence_key.clone(),
                    operation: id.clone(),
                    group: kind(group),
                    expected: expected.clone(),
                    source_sha256: descriptor
                        .source_sha256
                        .clone()
                        .ok_or_else(|| lcd_closed("missing archive digest"))?,
                };
                let publication_store = store.clone();
                let publication = tokio::task::spawn_blocking(move || {
                    crate::asset_publication::Publication::begin(publication_store, binding)
                })
                .await
                .map_err(lcd_closed)?
                .map_err(lcd_closed)?;
                let cleanup_handles = handles.clone();
                let cleanup_id = id.clone();
                let outcome = async {
                    match group {
                        Group::FanAndLighting => {
                            let permit = handles
                                .control_group_tx
                                .clone()
                                .reserve_owned()
                                .await
                                .map_err(lcd_closed)?;
                            let prepared = source.prepare(archives, publication.clone()).await?;
                            let arm = publication.clone();
                            tokio::task::spawn_blocking(move || arm.arm())
                                .await
                                .map_err(lcd_closed)?
                                .map_err(lcd_closed)?;
                            let (reply, rx) = oneshot::channel();
                            permit.send(ControlGroupCommand::Commit {
                                operation_id: id,
                                expected,
                                prepared,
                                reply,
                            });
                            let result = rx.await.map_err(lcd_closed)??;
                            Ok(FullGroupCommit {
                                readback: project(
                                    callback_dev,
                                    group,
                                    result.commit.readback,
                                    control_runtime(result.runtime),
                                )?,
                                repeated: result.commit.repeated,
                            })
                        }
                        Group::Screen => {
                            let (reply, ticket) = oneshot::channel();
                            handles
                                .lcd_tx
                                .send(LcdCommand::BeginThemePreparation { reply })
                                .await
                                .map_err(lcd_closed)?;
                            let preparation = ticket.await.map_err(lcd_closed)?;
                            // An outstanding reservation keeps LCD shutdown's closed
                            // receiver drain alive through preparation and admission.
                            let permit = handles
                                .lcd_tx
                                .clone()
                                .reserve_owned()
                                .await
                                .map_err(lcd_closed)?;
                            let prepared = source.prepare(archives, publication.clone()).await?;
                            let arm = publication.clone();
                            tokio::task::spawn_blocking(move || arm.arm())
                                .await
                                .map_err(lcd_closed)?
                                .map_err(lcd_closed)?;
                            let prepared = PreparedScreenGroup::prepare(
                                prepared,
                                store,
                                vec![sensors, handles.sensor_cache],
                            )
                            .await?;
                            let (reply, rx) = oneshot::channel();
                            permit.send(LcdCommand::CommitScreenGroup {
                                operation_id: id,
                                expected,
                                preparation: Some(preparation),
                                prepared,
                                reply,
                            });
                            let result = rx.await.map_err(lcd_closed)??;
                            let runtime = screen_runtime(result.runtime);
                            Ok(FullGroupCommit {
                                readback: project(
                                    callback_dev,
                                    group,
                                    result.commit.readback,
                                    runtime,
                                )?,
                                repeated: result.commit.repeated,
                            })
                        }
                    }
                }
                .await;
                let original_error = match outcome {
                    Ok(commit) => {
                        retain_assets(publication).await?;
                        return Ok(commit);
                    }
                    Err(error) => error,
                };
                match receipt(&cleanup_handles, cleanup_id).await {
                    Ok(None) => {
                        let cleanup =
                            tokio::task::spawn_blocking(move || publication.rollback()).await;
                        if let Err(cleanup) = cleanup
                            .map_err(|e| e.to_string())
                            .and_then(|r| r.map_err(|e| format!("{e:#}")))
                        {
                            return Err(match original_error {
                                IpcError::InvalidArgument(e) => {
                                    IpcError::InvalidArgument(format!("{e}; {cleanup}"))
                                }
                                e => lcd_closed(format!("{e}; {cleanup}")),
                            });
                        }
                    }
                    Ok(Some(_)) => retain_assets(publication).await?,
                    Err(error) => {
                        retain_unknown_assets(publication).await?;
                        return Err(lcd_closed(format!(
                            "{}; asset ownership retained because readback is unavailable: {error}",
                            original_error
                        )));
                    }
                }
                Err(original_error)
            })
            .await
    }
}

#[cfg(test)]
use crate::windows_screen_fixture as windows_fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_archive::ProfileArchives;
    use crate::{
        config::Config,
        manager::{DeviceManager, ManagerHandle},
        persistence::{settings_key, DeviceSettings},
    };
    use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
    use lconnect_ipc::{
        client::IpcClient, server::IpcServer, FullProfileGroup as G, GroupArchiveReply,
        GroupUploadPhase as P, GroupUploadRequest as U, GroupUploadState as S,
    };
    use std::path::{Path, PathBuf};
    struct Harness {
        manager: ManagerHandle,
        server: tokio::task::JoinHandle<std::io::Result<()>>,
        socket: PathBuf,
        dev: DeviceId,
    }
    impl Harness {
        async fn start(runtime: &Path, store: Arc<PersistenceStore>, mock: MockBackend) -> Self {
            std::fs::create_dir_all(runtime).unwrap();
            let socket = runtime.join("lconnectd.sock");
            let cfg = Config::default();
            let stats = Arc::new(Stats::new());
            let manager =
                DeviceManager::new(Arc::new(mock), store.clone(), cfg.clone(), stats.clone())
                    .spawn()
                    .await;
            let dev = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Some(d) = manager.list().await.first() {
                        break d.id.clone();
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store);
            let path = socket.clone();
            let server =
                tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&path).await });
            tokio::time::timeout(Duration::from_secs(3), async {
                while !socket.exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            Self {
                manager,
                server,
                socket,
                dev,
            }
        }
        async fn client(&self) -> IpcClient {
            IpcClient::connect(&self.socket).await.unwrap()
        }
        async fn stop(self) {
            self.server.abort();
            let _ = self.server.await;
            self.manager.shutdown().await;
        }
    }
    async fn upload(c: &IpcClient, d: &DeviceId, g: G, u: U) -> S {
        serde_json::from_value(
            c.call("group_upload", json!({"dev":d,"group":g,"request":u}))
                .await
                .unwrap(),
        )
        .unwrap()
    }
    async fn wait(c: &IpcClient, d: &DeviceId, g: G, id: &str, wanted: P) -> S {
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let state = upload(c, d, g, U::Status { id: id.into() }).await;
                if state.phase == wanted {
                    return state;
                }
                assert!(
                    !matches!(state.phase, P::Failed | P::RecoveryRequired | P::Cancelled),
                    "{state:?}"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn prepare(c: &IpcClient, d: &DeviceId, g: G, id: &str, bytes: &[u8]) {
        let snap: GroupArchiveReply = serde_json::from_value(
            c.call(
                "group_archive",
                json!({"dev":d,"group":g,"request":{"action":"snapshot"}}),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        let GroupArchiveReply::Snapshot { snapshot } = snap else {
            panic!()
        };
        upload(
            c,
            d,
            g,
            U::Begin {
                id: id.into(),
                size: bytes.len() as u64,
                expected: snapshot.expected,
            },
        )
        .await;
        for (i, chunk) in bytes
            .chunks(lconnect_ipc::PROFILE_ARCHIVE_CHUNK_BYTES)
            .enumerate()
        {
            upload(
                c,
                d,
                g,
                U::Chunk {
                    id: id.into(),
                    offset: (i * lconnect_ipc::PROFILE_ARCHIVE_CHUNK_BYTES) as u64,
                    bytes: chunk.to_vec(),
                },
            )
            .await;
        }
        upload(c, d, g, U::Inspect { id: id.into() }).await;
        wait(c, d, g, id, P::Prepared).await;
    }
    async fn reapply(
        c: &IpcClient,
        d: &DeviceId,
        g: G,
        id: &str,
        expected: &str,
    ) -> lconnect_ipc::Result<FullGroupReadback> {
        serde_json::from_value(
            c.call(
                "reapply_group_import",
                json!({"dev":d,"group":g,"operation_id":id,"expected":expected}),
            )
            .await?,
        )
        .map_err(|e| lconnect_ipc::IpcError::InvalidArgument(e.to_string()))
    }
    async fn readback(c: &IpcClient, d: &DeviceId, g: G, id: &str) -> FullGroupReadback {
        serde_json::from_value(
            c.call(
                "read_group_import",
                json!({"dev":d,"group":g,"operation_id":id}),
            )
            .await
            .unwrap(),
        )
        .unwrap()
    }
    #[tokio::test]
    async fn accepted_public_screen_reapply_survives_connection_loss_while_asset_preparation_is_blocked(
    ) {
        let root = tempfile::tempdir().unwrap();
        let settings_root = root.path().join("settings");
        let store = Arc::new(PersistenceStore::new_with_root(settings_root.clone()));
        let source_root = tempfile::tempdir().unwrap();
        let source = PersistenceStore::new_with_root(source_root.path().into());
        let image = source
            .theme_assets()
            .import(lconnect_image::defaults::IMAGES[0].png)
            .unwrap();
        let mut desired = DeviceSettings::default();
        desired.lcd.control_mode = "Application".into();
        desired.lcd.brightness = 73;
        desired.theme=Some(serde_json::from_value(json!({"version":1,"name":"Reconnect retry","background":[0,0,0],"background_image":{"asset":image,"fit":"crop"},"layers":[]})).unwrap());
        let profile = GroupProfile::capture(&desired, &Default::default(), GroupKind::Screen)
            .unwrap()
            .profile;
        let archives = ProfileArchives::new(
            source.theme_assets(),
            source.theme_fonts(),
            source.media_assets(),
        );
        let mut bytes = Vec::new();
        archives.export_group(&profile, &mut bytes).unwrap();
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("reapply-disconnect".into()),
            PathBuf::from("/mock/reapply-disconnect"),
        );
        let key = settings_key(&info);
        let mock = MockBackend::new();
        mock.add_device(info);
        let h = Harness::start(&root.path().join("runtime"), store.clone(), mock).await;
        let c = h.client().await;
        let id = "66666666666666666666666666666666";
        prepare(&c, &h.dev, G::Screen, id, &bytes).await;
        upload(&c, &h.dev, G::Screen, U::Finish { id: id.into() }).await;
        let committed = wait(&c, &h.dev, G::Screen, id, P::Imported)
            .await
            .committed
            .unwrap();
        let token = committed.readback.runtime.revision.unwrap();
        let generation = committed.readback.runtime.lcd.unwrap().generation;
        let device = settings_root.join("devices").join(format!("{key}.toml"));
        let profiles = settings_root.join("profiles.json");
        let previous = (
            std::fs::read(&device).unwrap(),
            std::fs::read(&profiles).unwrap(),
        );
        let assets = store.theme_assets();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (release, hold) = std::sync::mpsc::channel();
        let gate = tokio::task::spawn_blocking(move || {
            assets.with_catalog(move |_| {
                entered.send(()).unwrap();
                hold.recv().unwrap();
                Ok(())
            })
        });
        ready.await.unwrap();
        let lost = h.client().await;
        let dev = h.dev.clone();
        let expected = token.clone();
        let waiter =
            tokio::spawn(async move { reapply(&lost, &dev, G::Screen, id, &expected).await });
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let current = readback(&c, &h.dev, G::Screen, id).await;
                if current.runtime.revision.as_deref() != Some(token.as_str()) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        release.send(()).unwrap();
        gate.await.unwrap().unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let current = readback(&c, &h.dev, G::Screen, id).await;
                if current.runtime.lcd.as_ref().unwrap().generation > generation {
                    assert_eq!(
                        current.runtime.lcd.as_ref().unwrap().source,
                        lconnect_ipc::LcdSourceKind::Theme
                    );
                    assert!(current.runtime.last_error.is_none());
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(reapply(&c, &h.dev, G::Screen, id, &token).await.is_err());
        assert_eq!(
            previous,
            (
                std::fs::read(device).unwrap(),
                std::fs::read(profiles).unwrap()
            )
        );
        assert_eq!(store.read(&key).unwrap().group_imports.len(), 1);
        drop(c);
        h.stop().await;
    }
    async fn reset_snapshot(c: &IpcClient, d: &DeviceId) -> String {
        let reply: GroupArchiveReply = serde_json::from_value(
            c.call(
                "group_archive",
                json!({"dev":d,"group":"fan_and_lighting","request":{"action":"snapshot"}}),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        let GroupArchiveReply::Snapshot { snapshot } = reply else {
            panic!()
        };
        snapshot.expected
    }
    #[tokio::test]
    async fn full_control_reset_rpc_preserves_screen_globals_and_never_replays_over_later_edits() {
        use lconnect_ipc::{
            CoolingPolicy, FullGroupCommit, NamedProfile, ProfileEdit, ProfileGroup,
        };
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(
            root.path().join("settings"),
        ));
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("reset".into()),
            PathBuf::from("/mock/reset"),
        );
        let key = settings_key(&info);
        let mut initial = DeviceSettings::default();
        initial.lcd.brightness = 41;
        initial.lcd.rotation = "90".into();
        initial.lcd.control_mode = "LocalUI".into();
        initial.fan.pwm_percent = 38;
        initial.pump.pwm_percent = 55;
        initial.pump.source = "mcu".into();
        initial.lighting.fan_topology.sync_to_pump = true;
        initial.thermal.cpu = Some(lconnect_ipc::ThermalRule {
            sensor_id: "cpu:test".into(),
            threshold_c: 92.0,
            color: lconnect_ipc::Rgb { r: 255, g: 0, b: 0 },
        });
        let mut custom_banks = lconnect_ipc::lcdv2_default_cooling_modes("cpu:old").unwrap();
        custom_banks.fan[4].name = "Old renamed mode".into();
        custom_banks.fan[4].use_default_name = false;
        custom_banks.pump[0].policy = CoolingPolicy::FixedRpm { rpm: 3000 };
        initial.cooling_banks = Some(custom_banks);
        store.write(&key, &initial).unwrap();
        let screen = NamedProfile::defaults(ProfileGroup::Screen, 3, None).unwrap();
        store
            .profiles()
            .edit(
                0,
                ProfileEdit::Save {
                    profile: screen.clone(),
                },
            )
            .unwrap();
        let original_screen = serde_json::to_value(
            GroupProfile::capture(
                &initial,
                &store.profiles().list().unwrap(),
                GroupKind::Screen,
            )
            .unwrap()
            .profile,
        )
        .unwrap();
        let mock = MockBackend::new();
        mock.add_device(info);
        let h = Harness::start(&root.path().join("runtime"), store.clone(), mock.clone()).await;
        let c = h.client().await;
        let dev = h.dev.clone();
        let expected = reset_snapshot(&c, &dev).await;
        let old_bank = c
            .call("get_cooling_modes", json!({"dev":dev}))
            .await
            .unwrap();
        let id = "88888888888888888888888888888888";
        let params =
            json!({"dev":dev,"operation_id":id,"expected":expected,"sensor_id":"cpu:test"});
        let reset: FullGroupCommit = serde_json::from_value(
            c.call("reset_fan_and_lighting", params.clone())
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(!reset.repeated && reset.readback.selected_content_current);
        assert_eq!(reset.readback.runtime.recovery_required, Some(false));
        let cooling = reset.readback.runtime.cooling.as_ref().unwrap();
        assert_eq!(
            cooling.fan.requested,
            Some(lconnect_ipc::FanPreset::Standard.policy("cpu:test".into(), 0.0, 100))
        );
        assert_eq!(
            cooling.pump.requested,
            Some(lconnect_ipc::PumpPreset::Quiet.policy("cpu:test".into(), 0.0, 100))
        );
        assert!(cooling.fan.fallback_active && cooling.pump.fallback_active);
        assert_eq!(
            (cooling.fan.applied_percent, cooling.pump.applied_percent),
            (Some(100), Some(100))
        );
        let saved = store.read(&key).unwrap();
        assert_eq!(
            saved.cooling_banks,
            Some(lconnect_ipc::lcdv2_default_cooling_modes("cpu:test").unwrap())
        );
        assert_eq!(cooling.fan.mode_id.as_deref(), Some("StandardSpeed"));
        assert_eq!(cooling.pump.mode_id.as_deref(), Some("PWMQuiet"));
        assert_eq!(saved.cooling.fan_mode.as_deref(), Some("StandardSpeed"));
        assert_eq!(saved.cooling.pump_mode.as_deref(), Some("PWMQuiet"));
        assert!(c.call("save_cooling_mode", json!({
            "dev":dev,"target":"pump","expected_revision":old_bank["revision"],
            "profile":{"id":"FixRPM","name":"Stale editor","policy":{"mode":"fixed_rpm","rpm":3000}}
        })).await.is_err(), "reset must invalidate pre-reset editor snapshots");
        let library = store.profiles().list().unwrap();
        assert_eq!(library.revision, 2);
        assert_eq!(library.profiles.len(), 9);
        assert!(library.profiles.contains(&screen));
        assert_eq!(
            serde_json::to_value(
                GroupProfile::capture(&saved, &library, GroupKind::Screen)
                    .unwrap()
                    .profile
            )
            .unwrap(),
            original_screen
        );
        assert_eq!(saved.thermal, initial.thermal);
        assert_eq!(saved.lighting.fan.as_ref().unwrap().mode, "rainbow");
        assert_eq!(saved.lighting.pump.as_ref().unwrap().mode, "rainbow");
        assert!(!saved.lighting.fan_topology.sync_to_pump);
        assert_eq!(saved.lighting.fan_modes.len(), 6);
        assert_eq!(saved.lighting.pump_modes.len(), 16);
        c.call(
            "set_cooling_policy",
            json!({"dev":dev,"target":"fan","policy":CoolingPolicy::Fixed{percent:37}}),
        )
        .await
        .unwrap();
        store
            .profiles()
            .edit(
                2,
                ProfileEdit::Rename {
                    group: ProfileGroup::Pump,
                    slot: 1,
                    name: "Newer name".into(),
                },
            )
            .unwrap();
        let newer = store.profiles().list().unwrap();
        let repeated: FullGroupCommit = serde_json::from_value(
            c.call("reset_fan_and_lighting", params.clone())
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(repeated.repeated && !repeated.readback.selected_content_current);
        assert_eq!(
            repeated.readback.runtime.cooling.unwrap().fan.requested,
            Some(CoolingPolicy::Fixed { percent: 37 })
        );
        assert_eq!(store.profiles().list().unwrap(), newer);
        let mut changed = params.clone();
        changed["sensor_id"] = json!("cpu:other");
        assert!(c.call("reset_fan_and_lighting", changed).await.is_err());
        let mut stale = params.clone();
        stale["operation_id"] = json!("99999999999999999999999999999999");
        assert!(c.call("reset_fan_and_lighting", stale).await.is_err());
        assert_eq!(store.profiles().list().unwrap(), newer);
        drop(c);
        h.stop().await;
        let h = Harness::start(&root.path().join("restart"), store.clone(), mock).await;
        let c = h.client().await;
        let recovered = readback(&c, &h.dev, G::FanAndLighting, id).await;
        assert!(!recovered.selected_content_current);
        assert_eq!(store.profiles().list().unwrap(), newer);
        assert_eq!(
            store.read(&key).unwrap().cooling.fan,
            Some(CoolingPolicy::Fixed { percent: 37 })
        );
        assert_eq!(store.read(&key).unwrap().thermal, initial.thermal);
        drop(c);
        h.stop().await;
    }
    fn windows_options() -> lconnect_ipc::WindowsScreenSource {
        lconnect_ipc::WindowsScreenSource {
            record: 0,
            bindings_json: windows_fixture::bindings().to_string(),
            resources_json: windows_fixture::mapping().to_string(),
        }
    }
    async fn windows_upload(c: &IpcClient, d: &DeviceId, id: &str, bytes: &[u8]) -> S {
        let snap: GroupArchiveReply = serde_json::from_value(
            c.call(
                "group_archive",
                json!({"dev":d,"group":"screen","request":{"action":"snapshot"}}),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        let GroupArchiveReply::Snapshot { snapshot } = snap else {
            panic!()
        };
        upload(
            c,
            d,
            G::Screen,
            U::BeginWindowsScreen {
                id: id.into(),
                size: bytes.len() as u64,
                expected: snapshot.expected,
                source: windows_options(),
            },
        )
        .await;
        for (index, bytes) in bytes
            .chunks(lconnect_ipc::PROFILE_ARCHIVE_CHUNK_BYTES)
            .enumerate()
        {
            upload(
                c,
                d,
                G::Screen,
                U::Chunk {
                    id: id.into(),
                    offset: (index * lconnect_ipc::PROFILE_ARCHIVE_CHUNK_BYTES) as u64,
                    bytes: bytes.to_vec(),
                },
            )
            .await;
        }
        upload(c, d, G::Screen, U::Inspect { id: id.into() }).await;
        wait(c, d, G::Screen, id, P::Prepared).await
    }
    #[tokio::test]
    async fn windows_screen_public_upload_preview_finish_retry_and_restart_keep_editor_and_library()
    {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(
            root.path().join("settings"),
        ));
        let profile: lconnect_ipc::NamedProfile=serde_json::from_value(json!({"slot":2,"name":"Native library","content":{"kind":"screen","value":{"version":1,"name":"Keep me","background":[4,5,6],"layers":[]}}})).unwrap();
        let library = store
            .profiles()
            .edit(0, lconnect_ipc::ProfileEdit::Save { profile })
            .unwrap();
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("windows-screen".into()),
            PathBuf::from("/mock/windows-screen"),
        );
        let key = settings_key(&info);
        let mock = MockBackend::new();
        mock.add_device(info);
        let h = Harness::start(&root.path().join("runtime"), store.clone(), mock.clone()).await;
        let c = h.client().await;
        assert!(c
            .call("get_imported_lcd_editor", json!({"dev":h.dev}))
            .await
            .unwrap()
            .is_null());
        let catalog = store.theme_assets().list().unwrap();
        let before = serde_json::to_value(store.read(&key).unwrap()).unwrap();
        let id = "b1111111111111111111111111111111";
        let bytes = windows_fixture::archive(|_| {}, &[]);
        assert!(c
            .call(
                "preview_windows_screen_upload",
                json!({"dev":h.dev,"id":id,"source":"scene"})
            )
            .await
            .is_err());
        let ready = windows_upload(&c, &h.dev, id, &bytes).await;
        assert!(ready.summary.is_some() && ready.source_sha256.is_some());
        let lcd_before = c.call("get_lcd_state", json!({"dev":h.dev})).await.unwrap();
        let mut frames = Vec::new();
        for source in ["scene", "editor"] {
            let bytes: Vec<u8> = serde_json::from_value(
                c.call(
                    "preview_windows_screen_upload",
                    json!({"dev":h.dev,"id":id,"source":source}),
                )
                .await
                .unwrap(),
            )
            .unwrap();
            let frame = lconnect_image::assets::decode_asset(&bytes).unwrap();
            assert_eq!((frame.width(), frame.height()), (480, 480));
            frames.push(bytes);
        }
        assert_ne!(frames[0], frames[1]);
        assert_eq!(
            c.call("get_lcd_state", json!({"dev":h.dev})).await.unwrap(),
            lcd_before
        );
        assert!(c
            .call(
                "preview_windows_screen_upload",
                json!({"dev":h.dev,"id":id,"source":"other"})
            )
            .await
            .is_err());
        assert!(c
            .call(
                "preview_windows_screen_upload",
                json!({"dev":"unrelated-device","id":id,"source":"scene"})
            )
            .await
            .is_err());
        assert_eq!(store.theme_assets().list().unwrap(), catalog);
        assert_eq!(
            serde_json::to_value(store.read(&key).unwrap()).unwrap(),
            before
        );
        assert_eq!(store.profiles().list().unwrap().profiles, library.profiles);
        // Format and binding changes cannot reuse a previously admitted id.
        for request in [
            U::Begin {
                id: id.into(),
                size: bytes.len() as u64,
                expected: ready.target.expected.clone(),
            },
            U::BeginWindowsScreen {
                id: id.into(),
                size: bytes.len() as u64,
                expected: ready.target.expected.clone(),
                source: lconnect_ipc::WindowsScreenSource {
                    record: 1,
                    ..windows_options()
                },
            },
        ] {
            assert!(c
                .call(
                    "group_upload",
                    json!({"dev":h.dev,"group":"screen","request":request})
                )
                .await
                .is_err());
        }
        // Hold publication after preview. Finish's admitted worker must survive
        // loss of the connection while waiting for the target asset catalog.
        let assets = store.theme_assets();
        let (entered, ready) = oneshot::channel();
        let (release, hold) = std::sync::mpsc::channel();
        let gate = tokio::task::spawn_blocking(move || {
            assets.with_catalog(move |_| {
                entered.send(()).unwrap();
                hold.recv_timeout(Duration::from_secs(8)).unwrap();
                Ok(())
            })
        });
        ready.await.unwrap();
        upload(&c, &h.dev, G::Screen, U::Finish { id: id.into() }).await;
        drop(c);
        release.send(()).unwrap();
        gate.await.unwrap().unwrap();
        let c = h.client().await;
        let state = wait(&c, &h.dev, G::Screen, id, P::Imported).await;
        let committed = state.committed.unwrap();
        assert!(!committed.repeated);
        assert!(committed.readback.selected_content_current);
        assert_eq!(committed.readback.runtime.recovery_required, Some(false));
        let generation = committed.readback.runtime.lcd.unwrap().generation;
        let saved = store.read(&key).unwrap();
        assert!(saved.theme.is_some() && saved.screen_editor.as_ref().unwrap().theme.is_some());
        assert!(c
            .call(
                "preview_windows_screen_upload",
                json!({"dev":h.dev,"id":id,"source":"scene"})
            )
            .await
            .is_err());
        assert_ne!(saved.theme, saved.screen_editor.as_ref().unwrap().theme);
        let editable: Option<lconnect_ipc::LcdTheme> = serde_json::from_value(
            c.call("get_imported_lcd_editor", json!({"dev":h.dev}))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(editable, saved.screen_editor.as_ref().unwrap().theme);
        assert_eq!(store.read(&key).unwrap().theme, saved.theme);
        let observed = readback(&c, &h.dev, G::Screen, id).await;
        assert_eq!(observed.runtime.lcd.unwrap().generation, generation);

        assert_eq!(store.profiles().list().unwrap().profiles, library.profiles);
        drop(c);
        let c = h.client().await;
        let repeat = upload(&c, &h.dev, G::Screen, U::Finish { id: id.into() })
            .await
            .committed
            .unwrap();
        assert!(repeat.repeated);
        assert_eq!(repeat.readback.runtime.lcd.unwrap().generation, generation);
        drop(c);
        h.stop().await;
        let h = Harness::start(&root.path().join("restart"), store.clone(), mock).await;
        let c = h.client().await;
        let recovered = readback(&c, &h.dev, G::Screen, id).await;
        assert!(recovered.selected_content_current);
        let editable: Option<lconnect_ipc::LcdTheme> = serde_json::from_value(
            c.call("get_imported_lcd_editor", json!({"dev":h.dev}))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(editable, saved.screen_editor.as_ref().unwrap().theme);

        assert_eq!(store.read(&key).unwrap().screen_editor, saved.screen_editor);
        assert_eq!(store.profiles().list().unwrap().profiles, library.profiles);
        drop(c);
        h.stop().await;
    }
    #[tokio::test]
    async fn windows_screen_preview_cancel_and_stale_finish_leave_target_catalog_and_settings_unchanged(
    ) {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(
            root.path().join("settings"),
        ));
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("windows-cancel".into()),
            PathBuf::from("/mock/windows-cancel"),
        );
        let key = settings_key(&info);
        let mock = MockBackend::new();
        mock.add_device(info);
        let h = Harness::start(&root.path().join("runtime"), store.clone(), mock).await;
        let c = h.client().await;
        let catalog = store.theme_assets().list().unwrap();
        let before = serde_json::to_value(store.read(&key).unwrap()).unwrap();
        let bytes = windows_fixture::archive(|_| {}, &[]);
        let cancelled = "c1111111111111111111111111111111";
        windows_upload(&c, &h.dev, cancelled, &bytes).await;
        upload(
            &c,
            &h.dev,
            G::Screen,
            U::Cancel {
                id: cancelled.into(),
            },
        )
        .await;
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let state = upload(
                    &c,
                    &h.dev,
                    G::Screen,
                    U::Status {
                        id: cancelled.into(),
                    },
                )
                .await;
                if state.phase == P::Cancelled {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(store.theme_assets().list().unwrap(), catalog);
        assert_eq!(
            serde_json::to_value(store.read(&key).unwrap()).unwrap(),
            before
        );
        assert!(c
            .call(
                "preview_windows_screen_upload",
                json!({"dev":h.dev,"id":cancelled,"source":"scene"})
            )
            .await
            .is_err());
        let stale = "d1111111111111111111111111111111";
        windows_upload(&c, &h.dev, stale, &bytes).await;
        c.call(
            "patch_lcd_config",
            json!({"dev":h.dev,"patch":{"brightness_percent":47}}),
        )
        .await
        .unwrap();
        let newer = serde_json::to_value(store.read(&key).unwrap()).unwrap();
        upload(&c, &h.dev, G::Screen, U::Finish { id: stale.into() }).await;
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let state = upload(&c, &h.dev, G::Screen, U::Status { id: stale.into() }).await;
                if state.phase == P::Failed {
                    break;
                }
                assert_ne!(state.phase, P::Imported);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(store.theme_assets().list().unwrap(), catalog);
        assert_eq!(
            serde_json::to_value(store.read(&key).unwrap()).unwrap(),
            newer
        );
        drop(c);
        h.stop().await;
    }
}
