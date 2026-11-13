use super::*;
use crate::{
    actor::screen_group::{PreparedScreen, PreparedScreenGroup, PreparedSource, ScreenGroupResult},
    profile_group::{
        GroupImportCommit, GroupImportError, GroupImportIntent, GroupImportReadback, GroupRevision,
    },
};

impl Display {
    pub(super) async fn read_group_import(
        &mut self,
        id: &str,
    ) -> Result<Option<GroupImportReadback>> {
        let (reply, response) = oneshot::channel();
        if let Err(error) = self
            .settings
            .send(SettingsUpdate::ReadGroupImport {
                operation_id: id.into(),
                reply,
            })
            .await
        {
            return Err(self.uncertain_group(error.to_string()).await);
        }
        match response.await {
            Ok(Ok(readback)) => Ok(readback),
            Ok(Err(GroupImportError::Rejected(error))) => Err(invalid(error)),
            uncertain => {
                let error = match uncertain {
                    Ok(Err(error)) => error.to_string(),
                    Err(error) => error.to_string(),
                    _ => unreachable!(),
                };
                Err(self.uncertain_group(error).await)
            }
        }
    }
    async fn uncertain_group(&mut self, error: String) -> IpcError {
        self.reset_recovery_required = true;
        self.preparation = self.preparation.wrapping_add(1);
        self.state.generation += 1;
        self.fail(&busy(error.clone())).await;
        busy(error)
    }
    pub(super) fn group_state(&self) -> super::super::screen_group::ScreenGroupState {
        super::super::screen_group::ScreenGroupState {
            revision: self.intent.clone(),
            lcd: self.state.clone(),
            recovery_required: self.reset_recovery_required,
        }
    }
    pub(super) async fn begin_group_reapply(
        &mut self,
        id: String,
        expected: super::super::group_barrier::RuntimeRevision,
    ) -> Result<super::super::screen_group::ScreenReapplyAdmission> {
        crate::profile_group::validate_operation_id(&id).map_err(invalid)?;
        if self.intent != expected {
            return Err(busy("screen reapply superseded; read current runtime"));
        }
        let readback = self
            .read_group_import(&id)
            .await?
            .ok_or_else(|| busy("screen import receipt unavailable"))?;
        if readback.receipt.kind != crate::profile_group::GroupKind::Screen
            || !readback.selected_content_current
        {
            return Err(busy("screen imported content is no longer current"));
        }
        self.intent.advance();
        self.reset_retry = None;
        self.preparation = self.preparation.wrapping_add(1);
        Ok(super::super::screen_group::ScreenReapplyAdmission {
            readback,
            revision: self.intent.clone(),
            generation: self.state.generation,
            preparation: self.preparation,
        })
    }
    pub(super) async fn reapply_screen_group(
        &mut self,
        id: String,
        expected_generation: u64,
        preparation: u64,
        prepared: PreparedScreen,
    ) -> Result<LcdState> {
        self.reapply_screen_group_result(id, expected_generation, preparation, prepared)
            .await
            .map(|r| r.runtime.lcd)
    }
    pub(super) async fn reapply_screen_group_result(
        &mut self,
        id: String,
        expected_generation: u64,
        preparation: u64,
        mut prepared: PreparedScreen,
    ) -> Result<super::super::screen_group::ScreenReapplyResult> {
        let admission = async {
            let readback = self
                .read_group_import(&id)
                .await?
                .ok_or_else(|| busy("screen import receipt unavailable"))?;
            if readback.receipt.kind != crate::profile_group::GroupKind::Screen
                || !readback.selected_content_current
                || self.state.generation != expected_generation
                || self.preparation != preparation
                || !prepared.matches(&readback.settings)
            {
                return Err(busy("screen group reapply superseded; read current state"));
            }
            if prepared.streaming() {
                self.check_playback_firmware().await?;
            }
            Ok(readback)
        }
        .await;
        let readback = match admission {
            Ok(readback) => readback,
            Err(error) => {
                prepared.stop().await?;
                return Err(error);
            }
        };
        self.apply_screen_group(prepared).await;
        Ok(super::super::screen_group::ScreenReapplyResult {
            readback,
            runtime: self.group_state(),
        })
    }
    pub(super) async fn commit_screen_group(
        &mut self,
        id: String,
        expected: GroupRevision,
        preparation: Option<u64>,
        mut prepared: PreparedScreenGroup,
    ) -> Result<ScreenGroupResult> {
        let admission = async {
            let intent = GroupImportIntent::new(id.clone(), expected.clone(), &prepared.archive)
                .map_err(invalid)?;
            if let Some(readback) = self.read_group_import(&id).await? {
                if !readback.receipt.matches(&intent) {
                    return Err(invalid("group import id already has different parameters"));
                }
                return Ok(Some(GroupImportCommit {
                    readback,
                    repeated: true,
                }));
            }
            if preparation.is_some_and(|ticket| ticket != self.preparation) {
                return Err(busy("screen group preparation superseded"));
            }
            if prepared.screen.streaming() {
                self.check_playback_firmware().await?;
            }
            Ok(None)
        }
        .await;
        match admission {
            Ok(Some(commit)) => {
                prepared.stop().await?;
                return Ok(ScreenGroupResult {
                    runtime: self.group_state(),
                    commit,
                    lcd: self.state.clone(),
                });
            }
            Err(error) => {
                prepared.stop().await?;
                return Err(error);
            }
            Ok(None) => {}
        }
        let (reply, response) = oneshot::channel();
        if let Err(error) = self
            .settings
            .send(SettingsUpdate::CommitProfileGroup {
                operation_id: id,
                expected,
                prepared: prepared.archive.clone(),
                reply,
            })
            .await
        {
            let cleanup = prepared.stop().await;
            let error = match cleanup {
                Ok(()) => error.to_string(),
                Err(cleanup) => format!("{error}; prepared source cleanup: {cleanup}"),
            };
            return Err(self.uncertain_group(error).await);
        }
        let commit = match response.await {
            Ok(Ok(commit)) => commit,
            Ok(Err(GroupImportError::Rejected(error))) => {
                prepared.stop().await?;
                return Err(invalid(error));
            }
            uncertain => {
                let cleanup = prepared.stop().await;
                let error = match uncertain {
                    Ok(Err(error)) => error.to_string(),
                    Err(error) => error.to_string(),
                    _ => unreachable!(),
                };
                let error = match cleanup {
                    Ok(()) => error,
                    Err(cleanup) => format!("{error}; prepared source cleanup: {cleanup}"),
                };
                return Err(self.uncertain_group(error).await);
            }
        };
        if commit.repeated {
            prepared.stop().await?;
        } else {
            self.apply_screen_group(prepared.screen).await;
        }
        Ok(ScreenGroupResult {
            runtime: self.group_state(),
            commit,
            lcd: self.state.clone(),
        })
    }
    pub(super) async fn apply_screen_group(&mut self, mut prepared: PreparedScreen) {
        let streaming = prepared.streaming();
        self.state.requested = prepared.config.clone();
        self.source_saved = !matches!(prepared.source, PreparedSource::Idle);
        self.media_key = match &prepared.source {
            PreparedSource::Media { key, .. }
            | PreparedSource::Composition {
                media: Some(key), ..
            } => Some(key.clone()),
            _ => None,
        };
        self.media_pin = None;
        let setup = async {
            self.replace().await?;
            if *self.shutdown.borrow() || self.shutdown.has_changed().is_err() {
                return Err(busy("group committed during teardown; replay required"));
            }
            self.state.applied = Some(
                self.config(
                    &prepared.config,
                    if streaming {
                        ConfigTarget::HostRotated
                    } else {
                        ConfigTarget::Requested
                    },
                )
                .await?,
            );
            if *self.shutdown.borrow() || self.shutdown.has_changed().is_err() {
                return Err(busy("group committed during teardown; replay required"));
            }
            Ok(())
        }
        .await;
        if let Err(error) = setup {
            let error = match prepared.stop().await {
                Ok(()) => error,
                Err(cleanup) => busy(format!("{error}; prepared source cleanup: {cleanup}")),
            };
            self.fail(&error).await;
            return;
        }
        if streaming {
            self.stats = LcdPlaybackStats {
                generation: self.state.generation,
                ..Default::default()
            };
            self.started = Some(Instant::now());
        }
        let applied = match prepared.source {
            PreparedSource::Idle => Ok(()),
            PreparedSource::Media { source, .. } => {
                self.media_pin = Some(source.pin());
                self.source = Source::Media(MediaReader::spawn(source));
                self.start_playback().await
            }
            PreparedSource::Composition {
                prepared: composition,
                media,
            } => {
                self.source = Source::Composition {
                    worker: composition.worker,
                    runtime: composition.runtime,
                    first: Some(composition.first),
                };
                self.state.source = if media.is_some() {
                    LcdSourceKind::DiagnosticClip
                } else {
                    LcdSourceKind::Theme
                };
                let result = self.frame().await;
                if result.is_ok() {
                    if let Source::Composition { worker, .. } = &self.source {
                        worker.release_preparation_permit();
                    }
                }
                result
            }
        };
        if let Err(error) = applied {
            self.fail(&error).await;
        }
    }
}
