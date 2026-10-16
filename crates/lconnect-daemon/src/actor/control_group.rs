//! Serial FanAndLighting import: freeze -> preflight -> durable receipt ->
//! install both runtimes -> apply. A caller dropping its reply never cancels commit.
use super::{
    command_dispatch::SettingsUpdate,
    cooling::CoolingCommand,
    group_barrier::{GroupBarrierOwner, RuntimeRevision},
    LightingRuntimeState, LightingStateMsg, Shutdown,
};
use crate::{
    profile_archive::PreparedGroup,
    profile_group::{
        GroupImportCommit, GroupImportError, GroupImportIntent, GroupImportReadback, GroupKind,
        GroupRevision, GroupSettings,
    },
};
use lconnect_ipc::{CoolingState, IpcError, Result};
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlRevision {
    pub cooling: RuntimeRevision,
    pub lighting: RuntimeRevision,
}
#[derive(Debug)]
pub struct ControlGroupState {
    pub revision: Option<ControlRevision>,
    pub cooling: Option<CoolingState>,
    pub lighting: Option<LightingRuntimeState>,
    pub recovery_required: bool,
    pub last_error: Option<String>,
}
#[derive(Debug)]
pub struct ControlGroupResult {
    pub commit: GroupImportCommit,
    pub runtime: ControlGroupState,
}
#[derive(Debug)]
pub struct ControlReapplyResult {
    pub readback: GroupImportReadback,
    pub runtime: ControlGroupState,
}
#[derive(Debug)]
pub enum ControlGroupCommand {
    Reapply {
        operation_id: String,
        expected: ControlRevision,
        reply: oneshot::Sender<Result<ControlReapplyResult>>,
    },
    Commit {
        operation_id: String,
        expected: GroupRevision,
        prepared: Arc<PreparedGroup>,
        reply: oneshot::Sender<Result<ControlGroupResult>>,
    },
    State(oneshot::Sender<ControlGroupState>),
}
struct Coordinator {
    cooling: mpsc::Sender<CoolingCommand>,
    lighting: mpsc::Sender<LightingStateMsg>,
    settings: mpsc::Sender<SettingsUpdate>,
    shutdown: Shutdown,
    recovery: Option<String>,
}
async fn ask<T, C>(tx: &mpsc::Sender<C>, make: impl FnOnce(oneshot::Sender<T>) -> C) -> Result<T> {
    let (reply, rx) = oneshot::channel();
    tx.send(make(reply)).await.map_err(busy)?;
    rx.await.map_err(busy)
}
fn busy(error: impl std::fmt::Display) -> IpcError {
    IpcError::DeviceBusy(error.to_string())
}
fn invalid(error: impl std::fmt::Display) -> IpcError {
    IpcError::InvalidArgument(error.to_string())
}
impl Coordinator {
    fn stopping(&self) -> bool {
        *self.shutdown.borrow() || self.shutdown.has_changed().is_err()
    }
    fn uncertain(
        &mut self,
        owner: &mut GroupBarrierOwner,
        error: impl std::fmt::Display,
    ) -> IpcError {
        owner.seal();
        self.recovery = Some(error.to_string());
        busy(error)
    }
    async fn state(&self) -> ControlGroupState {
        let (cooling, lighting) = tokio::join!(
            ask(&self.cooling, CoolingCommand::GroupState),
            ask(&self.lighting, LightingStateMsg::GroupState)
        );
        let (cooling_revision, cooling) = match cooling {
            Ok((revision, state)) => (Some(revision), Some(state)),
            Err(_) => (None, None),
        };
        let (lighting_revision, lighting) = match lighting {
            Ok((revision, state)) => (Some(revision), Some(state)),
            Err(_) => (None, None),
        };
        let revision = cooling_revision
            .zip(lighting_revision)
            .map(|(cooling, lighting)| ControlRevision { cooling, lighting });
        let mut errors = Vec::new();
        if let Some(error) = &self.recovery {
            errors.push(error.clone());
        }
        if let Some(state) = &cooling {
            errors.extend(
                [&state.fan.last_error, &state.pump.last_error]
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        } else {
            errors.push("cooling actor unavailable".into());
        }
        if let Some(state) = &lighting {
            errors.extend(
                [
                    &state.thermal.fan.last_error,
                    &state.thermal.pump.last_error,
                    &state.dynamic.last_error,
                ]
                .into_iter()
                .flatten()
                .cloned(),
            );
        } else {
            errors.push("lighting actor unavailable".into());
        }
        ControlGroupState {
            revision,
            cooling,
            lighting,
            recovery_required: self.recovery.is_some(),
            last_error: (!errors.is_empty()).then(|| errors.join("; ")),
        }
    }
    async fn receipt(
        &mut self,
        owner: &mut GroupBarrierOwner,
        id: String,
    ) -> Result<Option<GroupImportReadback>> {
        match ask(&self.settings, |reply| SettingsUpdate::ReadGroupImport {
            operation_id: id,
            reply,
        })
        .await
        {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(GroupImportError::Rejected(error))) => Err(invalid(error)),
            Ok(Err(error)) => Err(self.uncertain(owner, error)),
            Err(error) => Err(self.uncertain(owner, error)),
        }
    }
    async fn commit(
        &mut self,
        id: String,
        expected: GroupRevision,
        prepared: Arc<PreparedGroup>,
    ) -> Result<ControlGroupResult> {
        if let Some(error) = &self.recovery {
            return Err(busy(format!(
                "fan+lighting recovery requires restart: {error}"
            )));
        }
        if self.stopping() {
            return Err(busy("fan+lighting coordinator shutting down"));
        }
        let intent =
            GroupImportIntent::new(id.clone(), expected.clone(), &prepared).map_err(invalid)?;
        let GroupSettings::FanAndLighting(group) = &prepared.profile().content else {
            return Err(invalid(
                "fan+lighting coordinator requires that profile group",
            ));
        };
        debug_assert_eq!(intent.kind, GroupKind::FanAndLighting);
        let mut owner = GroupBarrierOwner::new();
        let (a, b) = tokio::join!(
            ask(&self.cooling, |reply| CoolingCommand::FreezeGroup {
                barrier: owner.ticket(),
                reply
            }),
            ask(&self.lighting, |reply| LightingStateMsg::FreezeGroup {
                barrier: owner.ticket(),
                reply
            }),
        );
        a??;
        b??;
        if let Some(readback) = self.receipt(&mut owner, id.clone()).await? {
            if !readback.receipt.matches(&intent) {
                return Err(invalid("group import id already has different parameters"));
            }
            let runtime = self.state().await;
            owner.resume();
            return Ok(ControlGroupResult {
                commit: GroupImportCommit {
                    readback,
                    repeated: true,
                },
                runtime,
            });
        }
        let group = Arc::new((**group).clone());
        let (a, b) = tokio::join!(
            ask(&self.cooling, |reply| CoolingCommand::PrepareGroup {
                barrier: owner.ticket(),
                group: group.clone(),
                reply
            }),
            ask(&self.lighting, |reply| LightingStateMsg::PrepareGroup {
                barrier: owner.ticket(),
                group: group.clone(),
                reply
            }),
        );
        a??;
        b??;
        if self.stopping() {
            return Err(busy("fan+lighting coordinator shutting down before commit"));
        }
        // From admission onward, dropping a reply or this owner must not release
        // old producers until the durable outcome and both runtimes are known.
        owner.seal();
        let commit = match ask(&self.settings, |reply| SettingsUpdate::CommitProfileGroup {
            operation_id: id,
            expected,
            prepared,
            reply,
        })
        .await
        {
            Ok(Ok(commit)) => commit,
            Ok(Err(GroupImportError::Rejected(error))) => {
                owner.resume();
                return Err(invalid(error));
            }
            Ok(Err(error)) => return Err(self.uncertain(&mut owner, error)),
            Err(error) => return Err(self.uncertain(&mut owner, error)),
        };
        if commit.repeated {
            let runtime = self.state().await;
            owner.resume();
            return Ok(ControlGroupResult { commit, runtime });
        }
        if self.stopping() {
            let mut runtime = self.state().await;
            runtime.last_error =
                Some("durable fan+lighting import accepted; Apply deferred by shutdown".into());
            return Ok(ControlGroupResult { commit, runtime });
        }
        let runtime = self.install_apply(owner).await;
        Ok(ControlGroupResult { commit, runtime })
    }
    async fn reapply(
        &mut self,
        id: String,
        expected: ControlRevision,
    ) -> Result<ControlReapplyResult> {
        if self.recovery.is_some() {
            return Err(busy("fan+lighting recovery requires device-tree restart"));
        }
        if self.stopping() {
            return Err(busy("fan+lighting coordinator shutting down"));
        }
        crate::profile_group::validate_operation_id(&id).map_err(invalid)?;
        let mut owner = GroupBarrierOwner::new();
        let (a, b) = tokio::join!(
            ask(&self.cooling, |reply| CoolingCommand::FreezeGroup {
                barrier: owner.ticket(),
                reply
            }),
            ask(&self.lighting, |reply| LightingStateMsg::FreezeGroup {
                barrier: owner.ticket(),
                reply
            }),
        );
        a??;
        b??;
        let current = self.state().await;
        if current.revision.as_ref() != Some(&expected) {
            return Err(busy(
                "fan+lighting reapply superseded; read current runtime",
            ));
        }
        let readback = self
            .receipt(&mut owner, id)
            .await?
            .ok_or_else(|| busy("fan+lighting import receipt unavailable"))?;
        if readback.receipt.kind != GroupKind::FanAndLighting || !readback.selected_content_current
        {
            return Err(busy("fan+lighting imported content is no longer current"));
        }
        let settings = &readback.settings;
        let group = Arc::new(crate::profile_group::FanAndLightingGroup {
            fan: settings.fan.clone(),
            pump: settings.pump.clone(),
            cooling: settings.cooling.clone(),
            lighting: settings.lighting.clone(),
            dynamic_rgb: settings.dynamic_rgb.clone(),
            dynamic_rgb_profiles: settings.dynamic_rgb_profiles.clone(),
        });
        let (a, b) = tokio::join!(
            ask(&self.cooling, |reply| CoolingCommand::PrepareGroup {
                barrier: owner.ticket(),
                group: group.clone(),
                reply
            }),
            ask(&self.lighting, |reply| LightingStateMsg::PrepareGroup {
                barrier: owner.ticket(),
                group: group.clone(),
                reply
            }),
        );
        a??;
        b??;
        if self.stopping() {
            return Err(busy("fan+lighting reapply interrupted by shutdown"));
        }
        // Reapply changes no durable settings. From installation onward an
        // ambiguous runtime outcome still must not resume old producers.
        owner.seal();
        let runtime = self.install_apply(owner).await;
        Ok(ControlReapplyResult { readback, runtime })
    }
    async fn install_apply(&mut self, mut owner: GroupBarrierOwner) -> ControlGroupState {
        let (a, b) = tokio::join!(
            ask(&self.cooling, |reply| CoolingCommand::InstallGroup {
                barrier: owner.ticket(),
                reply
            }),
            ask(&self.lighting, |reply| LightingStateMsg::InstallGroup {
                barrier: owner.ticket(),
                reply
            }),
        );
        if let Err(error) = a.and_then(|r| r).and(b.and_then(|r| r)) {
            self.uncertain(&mut owner, error);
            return self.state().await;
        }
        // No producer can output old intent here: both are installed and held.
        let (a, b) = tokio::join!(
            ask(&self.cooling, |reply| CoolingCommand::ApplyGroup {
                barrier: owner.ticket(),
                reply
            }),
            ask(&self.lighting, |reply| LightingStateMsg::ApplyGroup {
                barrier: owner.ticket(),
                reply
            }),
        );
        if let Err(error) = a
            .and_then(|r| r)
            .map(|_| ())
            .and(b.and_then(|r| r).map(|_| ()))
        {
            self.uncertain(&mut owner, error);
            return self.state().await;
        }
        let runtime = self.state().await;
        owner.resume();
        runtime
    }
}
pub fn spawn(
    cooling: mpsc::Sender<CoolingCommand>,
    lighting: mpsc::Sender<LightingStateMsg>,
    settings: mpsc::Sender<SettingsUpdate>,
    mut shutdown: Shutdown,
) -> (mpsc::Sender<ControlGroupCommand>, JoinHandle<()>) {
    let (tx, mut rx) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        let mut c = Coordinator {
            cooling,
            lighting,
            settings,
            shutdown: shutdown.clone(),
            recovery: None,
        };
        loop {
            if c.stopping() {
                break;
            }
            tokio::select! {
                biased;
                _=shutdown.changed()=>break,
                command=rx.recv()=>match command {
                    Some(ControlGroupCommand::Commit {operation_id,expected,prepared,reply})=> {
                        let result=c.commit(operation_id,expected,prepared).await;
                        let _=reply.send(result);
                    }
                    Some(ControlGroupCommand::Reapply {operation_id,expected,reply})=>{
                        let result=c.reapply(operation_id,expected).await;let _=reply.send(result);
                    }
                    Some(ControlGroupCommand::State(reply))=>{let _=reply.send(c.state().await);}
                    None=>break,
                }
            }
        }
        rx.close();
        while let Some(command) = rx.recv().await {
            match command {
                ControlGroupCommand::Commit { reply, .. } => {
                    let _ = reply.send(Err(busy("fan+lighting coordinator shutting down")));
                }
                ControlGroupCommand::Reapply { reply, .. } => {
                    let _ = reply.send(Err(busy("fan+lighting coordinator shutting down")));
                }
                ControlGroupCommand::State(reply) => {
                    let _ = reply.send(c.state().await);
                }
            }
        }
    });
    (tx, task)
}
