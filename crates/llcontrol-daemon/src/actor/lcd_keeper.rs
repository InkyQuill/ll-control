//! Serialized display state, source lifetime, and transaction-boundary control.
use super::{
    command_dispatch::SettingsUpdate, lcd_state::merge_lcd_config, FrameOutcome, FrameReadPolicy,
    HidCommand, LcdCommand, Shutdown,
};
use crate::{codec_str::parse_lcd_mode, media_reader::MediaReader, persistence::LcdSection};
use llcontrol_ipc::{
    validate_h264_clip, H264Clip, IpcError, LcdConfig, LcdConfigPatch, LcdPlaybackStats,
    LcdSourceKind, LcdState, Result,
};
use llcontrol_protocol::galahad2_lcdv2::{
    firmware::FirmwareVersion,
    h264::build_send_h264_frame_512_request,
    lcd::{build_lcd_control_request, Brightness, LCDControlMode, LCDControlSetting},
    packets::ACommandPacket,
};
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::Instant,
};

enum ConfigTarget {
    Requested,
    HostRotated,
}

enum Source {
    Idle,
    Media(MediaReader),
    Composition {
        worker: crate::media_composition::MediaComposition,
        runtime: std::sync::Arc<crate::theme_runtime::ThemeRuntime>,
        first: Option<llcontrol_ipc::H264AccessUnit>,
    },
    Clip {
        clip: H264Clip,
        index: usize,
        loops: Option<u16>,
    },
}
struct Display {
    cleanup_errors: Vec<String>,
    intent: super::group_barrier::RuntimeRevision,
    shutdown: Shutdown,
    reset_recovery_required: bool,
    reset_retry: Option<(String, std::sync::Arc<crate::theme_runtime::ThemeRuntime>)>,
    source_saved: bool,
    media_key: Option<String>,
    media_pin: Option<crate::media_assets::MediaPin>,
    preparation: u64,
    hid: mpsc::Sender<HidCommand>,
    settings: mpsc::Sender<SettingsUpdate>,
    state: LcdState,
    source: Source,
    stats: LcdPlaybackStats,
    started: Option<Instant>,
    deadline: Instant,
}

pub fn spawn(
    hid: mpsc::Sender<HidCommand>,
    refresh_ms: u64,
    initial: LcdSection,
    settings: mpsc::Sender<SettingsUpdate>,
    commands: mpsc::Receiver<LcdCommand>,
    shutdown: Shutdown,
) -> JoinHandle<()> {
    spawn_inner(hid, refresh_ms, initial, settings, commands, shutdown, None)
}

/// Actor-tree variant: report nested worker cleanup failures after all joins.
pub(super) fn spawn_observed(
    hid: mpsc::Sender<HidCommand>,
    refresh_ms: u64,
    initial: LcdSection,
    settings: mpsc::Sender<SettingsUpdate>,
    commands: mpsc::Receiver<LcdCommand>,
    shutdown: Shutdown,
) -> (JoinHandle<()>, oneshot::Receiver<Vec<String>>) {
    let (report, received) = oneshot::channel();
    (
        spawn_inner(
            hid,
            refresh_ms,
            initial,
            settings,
            commands,
            shutdown,
            Some(report),
        ),
        received,
    )
}

fn spawn_inner(
    hid: mpsc::Sender<HidCommand>,
    _refresh_ms: u64,
    initial: LcdSection,
    settings: mpsc::Sender<SettingsUpdate>,
    mut commands: mpsc::Receiver<LcdCommand>,
    mut shutdown: Shutdown,
    cleanup_report: Option<oneshot::Sender<Vec<String>>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let requested = LcdConfig::new(
            if initial.control_mode.is_empty() {
                "LocalUI"
            } else {
                &initial.control_mode
            },
            if initial.control_mode.is_empty()
                && initial.rotation.is_empty()
                && initial.brightness == 0
            {
                100
            } else {
                initial.brightness
            },
            if initial.rotation.is_empty() {
                0
            } else {
                initial.rotation.parse().unwrap_or(u16::MAX)
            },
        );
        let validated = merge_lcd_config(
            &requested,
            &LcdConfigPatch {
                mode: Some(requested.mode.clone()),
                brightness_percent: Some(requested.brightness_percent),
                rotation_degrees: Some(requested.rotation_degrees),
            },
        );
        // Invalid persisted values remain visible as a restore error, but must not
        // poison later controls such as Clear, which merges its omitted siblings.
        let (requested, last_error) = match validated {
            Ok(config) => (config, None),
            Err(error) => (
                LcdConfig::new("LocalUI", 100, 0),
                Some(format!("LCD settings restore: {error}")),
            ),
        };
        let mut display = Display {
            cleanup_errors: Vec::new(),
            intent: super::group_barrier::RuntimeRevision::new(),
            shutdown: shutdown.clone(),
            reset_recovery_required: false,
            reset_retry: None,
            source_saved: false,
            media_key: None,
            media_pin: None,
            preparation: 0,
            hid,
            settings,
            state: LcdState {
                persistence: None,
                requested,
                applied: None,
                source: LcdSourceKind::Idle,
                generation: 0,
                last_error,
            },
            source: Source::Idle,
            stats: LcdPlaybackStats::default(),
            started: None,
            deadline: Instant::now(),
        };
        loop {
            if *shutdown.borrow() {
                break;
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                command = commands.recv() => match command { Some(command) => display.command(command).await, None => break },
                _ = tokio::time::sleep_until(display.deadline), if !matches!(display.source, Source::Idle) => {
                    if matches!(display.source, Source::Media(_) | Source::Composition { .. }) {
                        if !display.refresh_media(&mut commands, &mut shutdown).await {
                            break;
                        }
                    } else if let Err(error) = display.frame().await {
                        display.fail(&error).await;
                    }
                }
            }
        }
        // Queued Apply commands also own live codecs. Close admission first;
        // all accepted workers must be reaped before the teardown barrier.
        commands.close();
        while let Some(command) = commands.recv().await {
            match command {
                LcdCommand::SetImagePrepared {
                    mut prepared,
                    reply,
                    ..
                }
                | LcdCommand::SetComposition {
                    mut prepared,
                    reply,
                    ..
                }
                | LcdCommand::SetMediaComposition {
                    mut prepared,
                    reply,
                    ..
                } => {
                    if let Err(error) = prepared.worker.stop().await {
                        tracing::warn!(%error,"queued LCD composition shutdown failed");
                        display.record_cleanup_error(&error);
                    }
                    let _ = reply.send(Err(busy("LCD actor shutting down")));
                }
                LcdCommand::ResetScreen {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.worker.stop().await.map_err(busy);
                    let _ = reply.send(cleanup.and(Err(busy("LCD actor shutting down"))));
                }
                LcdCommand::ReapplyScreenReset {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.worker.stop().await.map_err(busy);
                    let _ = reply.send(cleanup.and(Err(busy("LCD actor shutting down"))));
                }
                LcdCommand::CommitScreenGroup {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.stop().await;
                    if let Err(error) = &cleanup {
                        display.record_cleanup_error(error);
                    }
                    let _ = reply.send(cleanup.and(Err(busy("LCD actor shutting down"))));
                }
                LcdCommand::ReapplyScreenGroupChecked {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.stop().await;
                    if let Err(error) = &cleanup {
                        display.record_cleanup_error(error);
                    }
                    let _ = reply.send(cleanup.and(Err(busy("LCD actor shutting down"))));
                }
                LcdCommand::ReapplyScreenGroup {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.stop().await;
                    if let Err(error) = &cleanup {
                        display.record_cleanup_error(error);
                    }
                    let _ = reply.send(cleanup.and(Err(busy("LCD actor shutting down"))));
                }
                _ => {}
            }
        }
        // Shutdown/sleep must join file reads before this actor reports teardown.
        if let Err(error) = display.stop().await {
            tracing::warn!(%error, "LCD media reader shutdown failed");
        }
        if let Some(report) = cleanup_report {
            let _ = report.send(display.cleanup_errors);
        }
    })
}
impl Display {
    fn record_cleanup_error(&mut self, error: &impl std::fmt::Display) {
        // Retain failure across later Idle/successful stop calls, bounded even
        // if callers repeatedly replace broken sources. First failure vetoes
        // firmware preflight; dropping excess diagnostics cannot turn it ready.
        if self.cleanup_errors.len() < 16 {
            self.cleanup_errors.push(error.to_string());
        }
    }
    async fn stop(&mut self) -> Result<()> {
        let joined = match &mut self.source {
            Source::Media(reader) => reader.stop().await.map_err(busy),
            Source::Composition { worker, .. } => worker.stop().await.map_err(busy),
            _ => Ok(()),
        };
        if let Err(error) = &joined {
            self.record_cleanup_error(error);
        }
        if let Some(started) = self.started.take() {
            self.stats.elapsed_ms = millis(started.elapsed());
        }
        self.source = Source::Idle;
        self.state.source = LcdSourceKind::Idle;
        self.stats.pending_frames = 0;
        joined
    }
    async fn fail(&mut self, error: &IpcError) {
        let stopped = self.stop().await;
        self.state.last_error = Some(match stopped {
            Ok(()) => error.to_string(),
            Err(stop_error) => format!("{error}; reader stop: {stop_error}"),
        });
    }
    async fn replace(&mut self) -> Result<()> {
        self.reset_retry = None;
        self.preparation = self.preparation.wrapping_add(1);
        self.stop().await?;
        self.state.generation += 1;
        self.state.last_error = None;
        Ok(())
    }
    async fn persist_theme(&mut self, theme: Option<llcontrol_ipc::LcdTheme>) -> Result<()> {
        // Ordinary LCD operations must not emit unrelated deltas. During startup
        // config replay, saved source intent is restored separately by the manager.
        if theme.is_none() && !self.source_saved {
            return Ok(());
        }
        let saved = theme.is_some();
        self.settings
            .send(SettingsUpdate::Theme(theme))
            .await
            .map_err(|_| busy("settings actor closed"))?;
        self.source_saved = saved;
        self.media_key = None;
        self.media_pin = None;
        Ok(())
    }
    async fn persist_media(
        &mut self,
        key: String,
        pin: Option<crate::media_assets::MediaPin>,
    ) -> Result<()> {
        self.settings
            .send(SettingsUpdate::MediaClip {
                key: key.clone(),
                pin: pin.clone(),
            })
            .await
            .map_err(|_| busy("settings actor closed"))?;
        self.source_saved = true;
        self.media_key = Some(key);
        self.media_pin = pin;
        Ok(())
    }
    async fn command(&mut self, command: LcdCommand) {
        // Invalidate retry tokens for every ordinary mutation admission, even
        // validation/HID failure; reads and autonomous frame refresh are inert.
        if !matches!(
            &command,
            LcdCommand::State { .. }
                | LcdCommand::GroupState { .. }
                | LcdCommand::ReadScreenReset { .. }
                | LcdCommand::Stats { .. }
                | LcdCommand::Theme { .. }
                | LcdCommand::MediaClip { .. }
                | LcdCommand::BeginScreenGroupReapply { .. }
                | LcdCommand::ReapplyScreenGroupChecked { .. }
        ) {
            self.intent.advance();
        }
        let command = if self.reset_recovery_required {
            match command {
                LcdCommand::State { .. }
                | LcdCommand::GroupState { .. }
                | LcdCommand::ReadScreenReset { .. }
                | LcdCommand::Stats { .. }
                | LcdCommand::Theme { .. }
                | LcdCommand::MediaClip { .. } => command,
                LcdCommand::BeginScreenGroupReapply { reply, .. } => {
                    let _ = reply.send(Err(busy("screen group recovery requires daemon restart")));
                    return;
                }
                LcdCommand::ReapplyScreenGroupChecked {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.stop().await;
                    let _ = reply.send(
                        cleanup.and(Err(busy("screen group recovery requires daemon restart"))),
                    );
                    return;
                }
                LcdCommand::BeginScreenReset { reply, .. } => {
                    let _ = reply.send(Err(busy("screen reset recovery requires daemon restart")));
                    return;
                }
                LcdCommand::ResetScreen {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.worker.stop().await.map_err(busy);
                    let _ = reply.send(
                        cleanup.and(Err(busy("screen reset recovery requires daemon restart"))),
                    );
                    return;
                }
                LcdCommand::ReapplyScreenReset {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.worker.stop().await.map_err(busy);
                    let _ = reply.send(
                        cleanup.and(Err(busy("screen reset recovery requires daemon restart"))),
                    );
                    return;
                }
                LcdCommand::SetTheme { reply, .. }
                | LcdCommand::SetMediaClip { reply, .. }
                | LcdCommand::SetImage { reply, .. }
                | LcdCommand::Patch { reply, .. }
                | LcdCommand::Clear { reply }
                | LcdCommand::PlayClip { reply, .. } => {
                    let _ = reply.send(Err(busy("screen reset recovery requires daemon restart")));
                    return;
                }
                LcdCommand::CommitScreenGroup {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.stop().await;
                    let _ = reply.send(
                        cleanup.and(Err(busy("screen group recovery requires daemon restart"))),
                    );
                    return;
                }
                LcdCommand::ReapplyScreenGroup {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let cleanup = prepared.stop().await;
                    let _ = reply.send(
                        cleanup.and(Err(busy("screen group recovery requires daemon restart"))),
                    );
                    return;
                }
                LcdCommand::SetImagePrepared {
                    mut prepared,
                    reply,
                    ..
                }
                | LcdCommand::SetComposition {
                    mut prepared,
                    reply,
                    ..
                }
                | LcdCommand::SetMediaComposition {
                    mut prepared,
                    reply,
                    ..
                } => {
                    let _ = prepared.worker.stop().await;
                    let _ = reply.send(Err(busy("screen reset recovery requires daemon restart")));
                    return;
                }
                // Dropped startup/preparation replies report closure to their caller.
                _ => return,
            }
        } else {
            command
        };
        match command {
            LcdCommand::GroupState { reply } => {
                let _ = reply.send(self.group_state());
            }
            LcdCommand::BeginScreenGroupReapply {
                operation_id,
                expected,
                reply,
            } => {
                let result = self.begin_group_reapply(operation_id, expected).await;
                let _ = reply.send(result);
            }
            LcdCommand::ReapplyScreenGroupChecked {
                operation_id,
                expected,
                expected_generation,
                preparation,
                mut prepared,
                reply,
            } => {
                let result = if self.intent != expected {
                    prepared
                        .stop()
                        .await
                        .and(Err(busy("screen reapply superseded; read current runtime")))
                } else {
                    self.intent.advance();
                    self.reapply_screen_group_result(
                        operation_id,
                        expected_generation,
                        preparation,
                        prepared,
                    )
                    .await
                };
                let _ = reply.send(result);
            }
            LcdCommand::SetMediaComposition {
                key,
                mut prepared,
                preparation,
                reply,
            } => {
                let result = if preparation.is_some_and(|ticket| ticket != self.preparation) {
                    prepared
                        .worker
                        .stop()
                        .await
                        .map_err(busy)
                        .and(Err(busy("media preparation superseded")))
                } else {
                    self.play_composition(prepared, Some(key))
                        .await
                        .map(|()| self.state.clone())
                };
                let _ = reply.send(result);
            }
            LcdCommand::CommitScreenGroup {
                operation_id,
                expected,
                preparation,
                prepared,
                reply,
            } => {
                let _ = reply.send(
                    self.commit_screen_group(operation_id, expected, preparation, prepared)
                        .await,
                );
            }
            LcdCommand::ReapplyScreenGroup {
                operation_id,
                expected_generation,
                preparation,
                prepared,
                reply,
            } => {
                let _ = reply.send(
                    self.reapply_screen_group(
                        operation_id,
                        expected_generation,
                        preparation,
                        prepared,
                    )
                    .await,
                );
            }
            LcdCommand::PrepareScreenResetReapply {
                operation_id,
                expected_generation,
                reply,
            } => {
                let result = self
                    .reset_retry_runtime(&operation_id, expected_generation)
                    .map(|runtime| {
                        self.preparation = self.preparation.wrapping_add(1);
                        (runtime, self.preparation)
                    });
                let _ = reply.send(result);
            }
            LcdCommand::ReapplyScreenReset {
                operation_id,
                expected_generation,
                preparation,
                mut prepared,
                reply,
            } => {
                let valid = self
                    .reset_retry_runtime(&operation_id, expected_generation)
                    .is_ok_and(|runtime| {
                        preparation == self.preparation
                            && std::sync::Arc::ptr_eq(&runtime, &prepared.runtime)
                    });
                let result = if !valid {
                    prepared
                        .worker
                        .stop()
                        .await
                        .map_err(busy)
                        .and(Err(busy("screen reset reapply superseded")))
                } else if let Err(error) = self.check_playback_firmware().await {
                    prepared.worker.stop().await.map_err(busy).and(Err(error))
                } else {
                    self.apply_screen_reset(operation_id, prepared).await;
                    Ok(self.state.clone())
                };
                let _ = reply.send(result);
            }
            LcdCommand::BeginScreenReset {
                operation_id,
                reply,
            } => {
                let (sender, response) = oneshot::channel();
                let result = async {
                    self.settings
                        .send(SettingsUpdate::ReadScreenReset { reply: sender })
                        .await
                        .map_err(|_| busy("settings actor closed"))?;
                    let (receipt, _) = response
                        .await
                        .map_err(|_| busy("settings readback closed"))?;
                    if let Some(receipt) = receipt.filter(|r| r.operation_id == operation_id) {
                        return Ok(super::ScreenResetAdmission::Repeated(
                            super::ScreenResetState {
                                receipt,
                                repeated: true,
                                lcd: self.state.clone(),
                            },
                        ));
                    }
                    self.reset_retry = None;
                    self.preparation = self.preparation.wrapping_add(1);
                    Ok(super::ScreenResetAdmission::Prepare(self.preparation))
                }
                .await;
                let _ = reply.send(result);
            }
            LcdCommand::ReadScreenReset { reply } => {
                let (sender, response) = oneshot::channel();
                let result = async {
                    self.settings
                        .send(SettingsUpdate::ReadScreenReset { reply: sender })
                        .await
                        .map_err(|_| busy("settings actor closed"))?;
                    let (receipt, persistence) = response
                        .await
                        .map_err(|_| busy("settings readback closed"))?;
                    Ok(llcontrol_ipc::ScreenResetReadback {
                        receipt,
                        persistence,
                        reapply_available: self.reset_retry.is_some(),
                        lcd: self.state.clone(),
                    })
                }
                .await;
                let _ = reply.send(result);
            }
            LcdCommand::ResetScreen {
                preparation,
                operation_id,
                expected_profile_revision,
                expected_collection_revision,
                mut prepared,
                reply,
            } => {
                if preparation.is_some_and(|ticket| ticket != self.preparation) {
                    let cleanup = prepared.worker.stop().await.map_err(busy);
                    let _ = reply.send(cleanup.and(Err(busy(
                        "screen reset preparation superseded by a newer display request",
                    ))));
                    return;
                }
                let result = self
                    .reset_screen(
                        operation_id,
                        expected_profile_revision,
                        expected_collection_revision,
                        prepared,
                    )
                    .await;
                let _ = reply.send(result);
            }
            LcdCommand::SetComposition {
                mut prepared,
                preparation,
                reply,
            } => {
                if preparation.is_some_and(|ticket| ticket != self.preparation) {
                    let cleanup = prepared.worker.stop().await.map_err(busy);
                    drop(prepared);
                    let _ = reply.send(cleanup.and(Err(busy(
                        "composition preparation superseded by a newer display request",
                    ))));
                    return;
                }
                let result = self.play_composition(prepared, None).await;
                let _ = reply.send(result.map(|()| self.state.clone()));
            }
            LcdCommand::MediaClip { reply } => {
                let _ = reply.send(self.media_key.clone());
            }
            LcdCommand::RestoreMediaError { key, error, reply } => {
                self.fail(&busy(format!("media restore: {error}"))).await;
                let _ = self.persist_media(key, None).await;
                let _ = reply.send(());
            }
            LcdCommand::SetMediaClip {
                key,
                source,
                preparation,
                reply,
            } => {
                if preparation.is_some_and(|ticket| ticket != self.preparation) {
                    let _ = reply.send(Err(busy(
                        "media preparation superseded by a newer display request",
                    )));
                    return;
                }
                let result = self.play_media(key, source).await;
                let _ = reply.send(result.map(|()| self.state.clone()));
            }
            LcdCommand::RestoreThemeError {
                theme,
                error,
                reply,
            } => {
                self.fail(&busy(format!("theme restore: {error}"))).await;
                let _ = self.persist_theme(Some(theme)).await;
                let _ = reply.send(());
            }
            LcdCommand::Theme { reply } => {
                let theme = match &self.source {
                    Source::Composition { runtime, .. }
                        if self.state.source == LcdSourceKind::Theme =>
                    {
                        Some(runtime.theme().clone())
                    }
                    _ => None,
                };
                let _ = reply.send(theme);
            }
            LcdCommand::BeginThemePreparation { reply } => {
                self.reset_retry = None;
                self.preparation = self.preparation.wrapping_add(1);
                let _ = reply.send(self.preparation);
            }
            LcdCommand::SetTheme {
                prepared,
                preparation,
                reply,
            } => {
                if preparation.is_some_and(|ticket| ticket != self.preparation) {
                    let _ = reply.send(Err(busy(
                        "theme preparation superseded by a newer display request",
                    )));
                    return;
                }
                let runtime = prepared.runtime;
                runtime.set_output_rotation(
                    llcontrol_image::FrameRotation::try_from(self.state.requested.rotation_degrees)
                        .map_err(invalid)
                        .unwrap_or(llcontrol_image::FrameRotation::None),
                );
                let result =
                    match crate::media_composition::PreparedComposition::prepare_canvas(runtime)
                        .await
                    {
                        Ok(prepared) => self
                            .play_composition(prepared, None)
                            .await
                            .map(|()| self.state.clone()),
                        Err(error) => Err(invalid(error)),
                    };
                let _ = reply.send(result);
            }
            LcdCommand::State { reply } => {
                let _ = reply.send(self.state.clone());
            }
            LcdCommand::Stats { reply } => {
                let mut stats = self.stats.clone();
                if let Some(started) = self.started {
                    stats.elapsed_ms = millis(started.elapsed());
                }
                let _ = reply.send(stats);
            }
            LcdCommand::Patch { patch, reply } => {
                let result = self.patch(patch).await;
                let _ = reply.send(result.map(|()| self.state.clone()));
            }
            LcdCommand::Clear { reply } => {
                let result = self
                    .patch(LcdConfigPatch {
                        mode: Some("LocalUI".into()),
                        brightness_percent: None,
                        rotation_degrees: None,
                    })
                    .await;
                let _ = reply.send(result.map(|()| self.state.clone()));
            }
            LcdCommand::SetImagePrepared {
                preparation,
                mut prepared,
                reply,
            } => {
                let result = if preparation != self.preparation {
                    prepared
                        .worker
                        .stop()
                        .await
                        .map_err(busy)
                        .and(Err(busy("image preparation superseded")))
                } else {
                    self.play_image_composition(prepared).await
                };
                let _ = reply.send(result.map(|()| self.state.clone()));
            }
            LcdCommand::SetImage { jpeg, reply } => {
                let result = async {
                    let runtime = std::sync::Arc::new(
                        crate::theme_runtime::ThemeRuntime::from_static_jpeg(&jpeg)?,
                    );
                    runtime.set_output_rotation(
                        llcontrol_image::FrameRotation::try_from(
                            self.state.requested.rotation_degrees,
                        )
                        .map_err(invalid)?,
                    );
                    let prepared =
                        crate::media_composition::PreparedComposition::prepare_canvas(runtime)
                            .await
                            .map_err(invalid)?;
                    self.play_image_composition(prepared).await
                }
                .await;
                let _ = reply.send(result.map(|()| self.state.clone()));
            }
            LcdCommand::PlayClip { clip, loops, reply } => {
                let result = self.play(clip, loops).await;
                let _ = reply.send(result.map(|()| self.state.clone()));
            }
        }
    }
    async fn reset_screen(
        &mut self,
        operation_id: String,
        expected_profile_revision: u64,
        expected_collection_revision: u64,
        mut composition: crate::media_composition::PreparedComposition,
    ) -> Result<super::ScreenResetState> {
        use super::command_dispatch::ScreenResetError;
        let default =
            llcontrol_ipc::NamedProfile::defaults(llcontrol_ipc::ProfileGroup::Screen, 0, None)?;
        if llcontrol_ipc::ProfileContent::Screen(composition.runtime.theme().clone())
            != default.content
        {
            composition.worker.stop().await.map_err(busy)?;
            return Err(invalid("screen reset requires the prepared default theme"));
        }
        if let Err(error) = self.check_playback_firmware().await {
            composition.worker.stop().await.map_err(busy)?;
            return Err(error);
        }
        let (reply, response) = oneshot::channel();
        if self
            .settings
            .send(SettingsUpdate::ResetScreen {
                operation_id,
                expected_profile_revision,
                expected_collection_revision,
                reply,
            })
            .await
            .is_err()
        {
            composition.worker.stop().await.map_err(busy)?;
            return Err(busy("settings actor closed"));
        }
        let committed = match response.await {
            Ok(Ok(committed)) => committed,
            Ok(Err(ScreenResetError::Rejected(error))) => {
                composition.worker.stop().await.map_err(busy)?;
                return Err(invalid(error));
            }
            uncertain => {
                let error = match uncertain {
                    Ok(Err(error)) => error.to_string(),
                    Err(error) => format!("screen reset reply lost: {error}"),
                    Ok(Ok(_)) => unreachable!(),
                };
                let _ = composition.worker.stop().await;
                self.reset_recovery_required = true;
                self.reset_retry = None;
                self.preparation = self.preparation.wrapping_add(1);
                self.state.generation += 1;
                self.fail(&busy(error.clone())).await;
                return Err(busy(error));
            }
        };
        let Some(receipt) = committed.settings.screen_reset_receipt else {
            composition.worker.stop().await.map_err(busy)?;
            return Err(busy("screen reset committed without receipt"));
        };
        if !committed.repeated {
            self.apply_screen_reset(receipt.operation_id.clone(), composition)
                .await;
        } else {
            composition.worker.stop().await.map_err(busy)?;
        }
        Ok(super::ScreenResetState {
            receipt,
            repeated: committed.repeated,
            lcd: self.state.clone(),
        })
    }

    fn reset_retry_runtime(
        &self,
        operation_id: &str,
        expected_generation: u64,
    ) -> Result<std::sync::Arc<crate::theme_runtime::ThemeRuntime>> {
        self.reset_retry
            .as_ref()
            .filter(|(id, _)| id == operation_id && self.state.generation == expected_generation)
            .map(|(_, runtime)| runtime.clone())
            .ok_or_else(|| busy("screen reset reapply is no longer available; read current state"))
    }

    async fn apply_screen_reset(
        &mut self,
        operation_id: String,
        mut composition: crate::media_composition::PreparedComposition,
    ) {
        let retry_runtime = composition.runtime.clone();
        self.source_saved = true;
        self.media_key = None;
        self.media_pin = None;
        self.state.requested = LcdConfig::new("Application", 50, 0);
        let apply = async {
            if let Err(error) = self.replace().await {
                composition.worker.stop().await.map_err(busy)?;
                return Err(error);
            }
            let applied = match self
                .config(&self.state.requested, ConfigTarget::HostRotated)
                .await
            {
                Ok(applied) => applied,
                Err(error) => {
                    composition.worker.stop().await.map_err(busy)?;
                    return Err(error);
                }
            };
            self.state.applied = Some(applied);
            self.source = Source::Composition {
                worker: composition.worker,
                runtime: composition.runtime,
                first: Some(composition.first),
            };
            self.state.source = LcdSourceKind::Theme;
            self.frame().await?;
            if let Source::Composition { worker, .. } = &self.source {
                worker.release_preparation_permit();
            }
            Ok::<(), IpcError>(())
        }
        .await;
        if let Err(error) = apply {
            self.fail(&error).await;
            self.reset_retry = Some((operation_id, retry_runtime));
        }
    }

    async fn patch(&mut self, patch: LcdConfigPatch) -> Result<()> {
        let next = merge_lcd_config(&self.state.requested, &patch)?;
        if matches!(self.source, Source::Clip { .. })
            && patch.mode.is_none()
            && next.rotation_degrees != 0
        {
            return Err(invalid("diagnostic playback requires rotation 0"));
        }
        if matches!(self.source, Source::Media(_))
            && patch.mode.is_none()
            && next == self.state.requested
            && self.state.last_error.is_none()
        {
            return Ok(());
        }
        if matches!(self.source, Source::Media(_))
            && patch.mode.is_none()
            && next.rotation_degrees != 0
        {
            return self.rotate_direct_media(next).await;
        }
        let live_rotation = if patch.mode.is_none() && patch.rotation_degrees.is_some() {
            match &self.source {
                Source::Composition { runtime, .. } => Some((
                    runtime.clone(),
                    llcontrol_image::FrameRotation::try_from(next.rotation_degrees)
                        .map_err(invalid)?,
                )),
                _ => None,
            }
        } else {
            None
        };
        if let Some((runtime, rotation)) = &live_rotation {
            if next.brightness_percent == self.state.requested.brightness_percent {
                runtime.set_output_rotation(*rotation);
                self.state.requested = next.clone();
                if let Some(applied) = &mut self.state.applied {
                    applied.rotation_degrees = next.rotation_degrees;
                }
                self.state.last_error = None;
                self.reset_retry = None;
                return self.persist_lcd_config(&next).await;
            }
        }
        self.reset_retry = None;
        if patch.mode.is_some() {
            self.persist_theme(None).await?;
            self.replace().await?;
        }
        self.state.requested = next.clone();
        let result = self
            .config(
                &next,
                match self.source {
                    Source::Clip { .. } | Source::Media(_) | Source::Composition { .. } => {
                        ConfigTarget::HostRotated
                    }
                    Source::Idle => ConfigTarget::Requested,
                },
            )
            .await;
        let applied = match result {
            Ok(applied) => applied,
            Err(error) => {
                self.fail(&error).await;
                return Err(error);
            }
        };
        if let Some((runtime, rotation)) = live_rotation {
            runtime.set_output_rotation(rotation);
        }
        self.state.applied = Some(applied);
        self.state.last_error = None;
        self.persist_lcd_config(&next).await
    }
    async fn rotate_direct_media(&mut self, next: LcdConfig) -> Result<()> {
        use crate::{media_composition::MediaComposition, theme_runtime::ThemeRuntime};
        let key = self
            .media_key
            .clone()
            .ok_or_else(|| busy("direct media identity missing"))?;
        let runtime = std::sync::Arc::new(
            ThemeRuntime::new(
                super::screen_group::media_theme(key),
                Default::default(),
                vec![],
            )?
            .with_output_rotation(
                llcontrol_image::FrameRotation::try_from(next.rotation_degrees).map_err(invalid)?,
            ),
        );
        let Source::Media(reader) = &mut self.source else {
            return Err(busy("direct media source changed"));
        };
        let index = reader.next_index();
        let source = reader.take_source().await.map_err(busy)?;
        // Duplicate only the fd/index (legacy payload is Arc-owned). Indexed
        // reads use positional I/O, so rollback and decoder cannot race a seek.
        let backup = match source.try_clone() {
            Ok(backup) => backup,
            Err(error) => {
                self.source = Source::Media(MediaReader::spawn_at(source, index));
                return Err(busy(error));
            }
        };
        let mut worker = match MediaComposition::spawn_at(source, runtime.clone(), index) {
            Ok(worker) => worker,
            Err(error) => {
                self.source = Source::Media(MediaReader::spawn_at(backup, index));
                return Err(busy(error));
            }
        };
        let mut shutdown = self.shutdown.clone();
        let first = tokio::select! {
            biased;
            _ = async { if !*shutdown.borrow() { let _ = shutdown.changed().await; } } => Err(busy("LCD actor shutting down")),
            first = worker.next_unit() => first.map_err(busy),
        };
        let first = match first {
            Ok(first) => first,
            Err(error) => {
                worker.stop().await.map_err(busy)?;
                self.source = Source::Media(MediaReader::spawn_at(backup, index));
                return Err(error);
            }
        };
        // Prepare before any visible config mutation. A brightness failure keeps
        // the original stream angle/position; never send a new-angle AU on error.
        let applied = if next.brightness_percent == self.state.requested.brightness_percent {
            self.state.applied.clone()
        } else {
            match self.config(&next, ConfigTarget::HostRotated).await {
                Ok(applied) => Some(applied),
                Err(error) => {
                    worker.stop().await.map_err(busy)?;
                    self.source = Source::Media(MediaReader::spawn_at(backup, index));
                    return Err(error);
                }
            }
        };
        if *self.shutdown.borrow() {
            worker.stop().await.map_err(busy)?;
            self.source = Source::Media(MediaReader::spawn_at(backup, index));
            return Err(busy("LCD actor shutting down"));
        }
        self.source = Source::Composition {
            worker,
            runtime,
            first: Some(first),
        };
        self.state.requested = next.clone();
        self.state.applied = applied.map(|mut config| {
            config.rotation_degrees = next.rotation_degrees;
            config
        });
        self.state.last_error = None;
        self.reset_retry = None;
        // Keep generation, stats, started/deadline, source identity and pins.
        self.persist_lcd_config(&next).await
    }
    async fn persist_lcd_config(&self, next: &LcdConfig) -> Result<()> {
        self.settings
            .send(SettingsUpdate::Lcd(LcdSection {
                brightness: next.brightness_percent,
                rotation: next.rotation_degrees.to_string(),
                control_mode: next.mode.clone(),
            }))
            .await
            .map_err(|_| busy("settings actor closed"))?;
        Ok(())
    }
    async fn config(&self, config: &LcdConfig, target: ConfigTarget) -> Result<LcdConfig> {
        let settings_only = !matches!(target, ConfigTarget::Requested);
        let mut setting = LCDControlSetting::default();
        setting.mode = if settings_only {
            LCDControlMode::LCDSetting
        } else {
            parse_lcd_mode(&config.mode)
        };
        setting.brightness = Brightness::new(config.brightness_percent).map_err(invalid)?;
        // Mount rotation belongs to the frame producer, even across handoff to LocalUI.
        // LCDControlSetting::default leaves the hardware rotation byte at zero.
        setting.video_fps = 24;
        let (reply, rx) = oneshot::channel();
        self.hid
            .send(HidCommand::Write {
                bytes: build_lcd_control_request(setting).to_vec(),
                reply,
            })
            .await
            .map_err(|_| busy("HID actor closed"))?;
        rx.await
            .map_err(|_| busy("HID reply dropped"))?
            .map_err(busy)?;
        Ok(LcdConfig::new(
            if settings_only {
                "LCDSetting"
            } else {
                &config.mode
            },
            config.brightness_percent,
            config.rotation_degrees,
        ))
    }
    async fn play(&mut self, clip: H264Clip, loops: u16) -> Result<()> {
        validate_h264_clip(&clip, loops)?;
        self.prepare_playback(None, None, 0).await?;
        self.source = Source::Clip {
            clip,
            index: 0,
            loops: Some(loops),
        };
        self.start_playback().await
    }
    async fn play_media(
        &mut self,
        key: String,
        source: crate::media_assets::PinnedMedia,
    ) -> Result<()> {
        self.prepare_playback(Some((key, source.pin())), None, 0)
            .await?;
        self.source = Source::Media(MediaReader::spawn(source));
        self.start_playback().await
    }
    async fn play_composition(
        &mut self,
        prepared: crate::media_composition::PreparedComposition,
        logical_media: Option<String>,
    ) -> Result<()> {
        let crate::media_composition::PreparedComposition {
            mut worker,
            first,
            runtime,
            pin,
        } = prepared;
        let theme = runtime.theme().clone();
        let preparation = async {
            let media = match (theme.background_video.clone(), pin) {
                (Some(key), Some(pin)) => Some((key, pin)),
                (None, None) => None,
                _ => return Err(invalid("composition source pin does not match theme")),
            };
            let saved_theme = if let Some(key) = &logical_media {
                if theme != super::screen_group::media_theme(key.clone()) {
                    return Err(invalid("media composition identity mismatch"));
                }
                None
            } else {
                Some((theme, runtime.image_leases(), runtime.media_pin()))
            };
            self.prepare_playback(media, saved_theme, runtime.output_rotation().degrees())
                .await
        }
        .await;
        if let Err(error) = preparation {
            worker.stop().await.map_err(busy)?;
            return Err(error);
        }
        self.source = Source::Composition {
            worker,
            runtime,
            first: Some(first),
        };
        self.state.source = if logical_media.is_some() {
            LcdSourceKind::DiagnosticClip
        } else {
            LcdSourceKind::Theme
        };
        let result = self.frame().await;
        if let Err(error) = &result {
            self.fail(error).await;
        } else if let Source::Composition { worker, .. } = &self.source {
            worker.release_preparation_permit();
        }
        result
    }
    async fn play_image_composition(
        &mut self,
        prepared: crate::media_composition::PreparedComposition,
    ) -> Result<()> {
        let crate::media_composition::PreparedComposition {
            mut worker,
            first,
            runtime,
            ..
        } = prepared;
        if let Err(error) = self
            .prepare_playback(None, None, runtime.output_rotation().degrees())
            .await
        {
            worker.stop().await.map_err(busy)?;
            return Err(error);
        }
        self.source = Source::Composition {
            worker,
            runtime,
            first: Some(first),
        };
        self.state.source = LcdSourceKind::Jpeg;
        let result = self.frame().await;
        if let Err(error) = &result {
            self.fail(error).await;
        } else if let Source::Composition { worker, .. } = &self.source {
            worker.release_preparation_permit();
        }
        result
    }
    async fn prepare_playback(
        &mut self,
        media: Option<(String, crate::media_assets::MediaPin)>,
        theme: Option<(
            llcontrol_ipc::LcdTheme,
            llcontrol_image::theme::ThemeAssets,
            Option<crate::media_assets::MediaPin>,
        )>,
        software_rotation: u16,
    ) -> Result<()> {
        if self.state.requested.rotation_degrees != software_rotation {
            return Err(invalid("diagnostic playback requires rotation 0"));
        }
        self.check_playback_firmware().await?;
        if let Some((key, pin)) = media {
            if let Some((theme, images, extra_pin)) = theme {
                let mut commit_pin = pin.clone();
                if let Some(extra) = extra_pin {
                    commit_pin.retain(extra);
                }
                self.settings
                    .send(SettingsUpdate::ComposedTheme {
                        theme,
                        pin: commit_pin,
                        images,
                    })
                    .await
                    .map_err(|_| busy("settings actor closed"))?;
                self.source_saved = true;
                self.media_key = None;
                self.media_pin = Some(pin);
            } else {
                self.persist_media(key, Some(pin)).await?;
            }
        } else if let Some((theme, images, pin)) = theme {
            self.settings
                .send(SettingsUpdate::PinnedTheme { theme, images, pin })
                .await
                .map_err(|_| busy("settings actor closed"))?;
            self.source_saved = true;
            self.media_key = None;
            self.media_pin = None;
        } else {
            self.persist_theme(None).await?;
        }
        self.replace().await?;
        self.stats = LcdPlaybackStats {
            generation: self.state.generation,
            ..LcdPlaybackStats::default()
        };
        self.started = Some(Instant::now());
        match self
            .config(&self.state.requested, ConfigTarget::HostRotated)
            .await
        {
            Ok(applied) => self.state.applied = Some(applied),
            Err(error) => {
                self.fail(&error).await;
                return Err(error);
            }
        }
        Ok(())
    }
    async fn check_playback_firmware(&self) -> Result<()> {
        // Share PN classification with identity readback; a delayed PN1 is not a version.
        let (reply, rx) = oneshot::channel();
        self.hid
            .send(HidCommand::Firmware { reply })
            .await
            .map_err(|_| busy("HID actor closed"))?;
        let responses = rx
            .await
            .map_err(|_| busy("firmware reply dropped"))?
            .map_err(busy)?;
        let packet = ACommandPacket::parse_response(&responses.version, 134).map_err(invalid)?;
        // Preserve the stricter playback gate: damaged identity bytes cannot enable streaming.
        std::str::from_utf8(packet.data()).map_err(invalid)?;
        let firmware = FirmwareVersion::from_optional_responses(
            &responses.version,
            responses.datetime.as_ref().map(|bytes| bytes.as_slice()),
        )
        .map_err(invalid)?;
        if !eligible(&firmware.user_version) {
            return Err(invalid(
                "diagnostic playback requires known firmware >= 1.5",
            ));
        }
        Ok(())
    }
    async fn start_playback(&mut self) -> Result<()> {
        self.state.source = LcdSourceKind::DiagnosticClip;
        let result = self.frame().await;
        if let Err(error) = &result {
            self.fail(error).await;
        }
        result
    }
    async fn refresh_media(
        &mut self,
        commands: &mut mpsc::Receiver<LcdCommand>,
        shutdown: &mut Shutdown,
    ) -> bool {
        let source = &mut self.source;
        tokio::select! {
            biased;
            _ = shutdown.changed() => false,
            command = commands.recv() => match command {
                Some(command) => { self.command(command).await; true }
                None => false,
            },
            unit = next_video_unit(source) => {
                let result = match unit {
                    Ok(unit) => self.send_frame(Some(unit)).await,
                    Err(error) => Err(busy(error)),
                };
                if let Err(error) = result { self.fail(&error).await; }
                true
            }
        }
    }
    async fn frame(&mut self) -> Result<()> {
        let media_frame = if matches!(self.source, Source::Media(_) | Source::Composition { .. }) {
            Some(next_video_unit(&mut self.source).await.map_err(busy)?)
        } else {
            None
        };
        self.send_frame(media_frame).await
    }
    async fn send_frame(
        &mut self,
        media_frame: Option<llcontrol_ipc::H264AccessUnit>,
    ) -> Result<()> {
        let (packets, read_policy, length, clip_frame) = match &self.source {
            Source::Idle => return Ok(()),
            Source::Media(_) | Source::Composition { .. } => {
                let unit = media_frame
                    .as_ref()
                    .ok_or_else(|| busy("missing media frame"))?;
                (
                    build_send_h264_frame_512_request(&unit.bytes)
                        .map_err(invalid)?
                        .iter()
                        .map(|p| p.to_vec())
                        .collect::<Vec<_>>(),
                    FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 },
                    unit.bytes.len(),
                    true,
                )
            }
            Source::Clip { clip, index, .. } => (
                build_send_h264_frame_512_request(&clip.units[*index].bytes)
                    .map_err(invalid)?
                    .iter()
                    .map(|p| p.to_vec())
                    .collect::<Vec<_>>(),
                FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 },
                clip.units[*index].bytes.len(),
                true,
            ),
        };
        let start = Instant::now();
        let report_count = packets.len();
        let result = batch(&self.hid, packets, read_policy).await;
        if clip_frame {
            match &result {
                Ok(outcome) => {
                    self.stats.sent_frames += 1;
                    self.stats.sent_bytes += length as u64;
                    if outcome.readback.is_empty() {
                        self.stats.read_timeouts += 1;
                    }
                    tracing::trace!(generation=self.state.generation, elapsed_us=outcome.elapsed.as_micros(), readback=?outcome.readback, "LCD frame transaction");
                }
                Err(error) => {
                    self.stats.failed_transactions += 1;
                    // A failed read does not undo the completed AU write. Preserve
                    // that distinction while still stopping the failed transaction.
                    if matches!(error, llcontrol_hid::Error::FrameBatchFailed { written_reports, .. } if *written_reports == report_count)
                    {
                        self.stats.sent_frames += 1;
                        self.stats.sent_bytes += length as u64;
                    }
                }
            }
        }
        result.map_err(busy)?;
        match &mut self.source {
            Source::Clip { clip, index, loops } => {
                *index += 1;
                if *index == clip.units.len() {
                    *index = 0;
                    if let Some(remaining) = loops {
                        *remaining -= 1;
                    }
                }
                if *loops == Some(0) {
                    self.stop().await?;
                } else {
                    self.stats.pending_frames = 1;
                    self.deadline =
                        (start + Duration::from_nanos(1_000_000_000 / 24)).max(Instant::now());
                }
            }
            Source::Media(reader) => {
                self.stats.pending_frames = 1;
                self.deadline = (start
                    + Duration::from_nanos(1_000_000_000 / u64::from(reader.fps())))
                .max(Instant::now());
            }
            Source::Composition { .. } => {
                self.stats.pending_frames = 1;
                self.deadline =
                    (start + Duration::from_nanos(1_000_000_000 / 24)).max(Instant::now());
            }
            Source::Idle => {}
        }
        Ok(())
    }
}
async fn next_video_unit(source: &mut Source) -> anyhow::Result<llcontrol_ipc::H264AccessUnit> {
    match source {
        Source::Media(reader) => reader.next_unit().await,
        Source::Composition { first, worker, .. } => match first.take() {
            Some(unit) => Ok(unit),
            None => worker.next_unit().await,
        },
        _ => anyhow::bail!("source is not video"),
    }
}
fn eligible(version: &str) -> bool {
    let parts: Option<Vec<u32>> = version
        .trim()
        .split('.')
        .map(|part| part.parse().ok())
        .collect();
    matches!(parts.as_deref(), Some([major, minor]) if (*major, *minor) >= (1, 5))
}
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
fn invalid(error: impl std::fmt::Display) -> IpcError {
    IpcError::InvalidArgument(error.to_string())
}
fn busy(error: impl std::fmt::Display) -> IpcError {
    IpcError::DeviceBusy(error.to_string())
}
async fn batch(
    hid: &mpsc::Sender<HidCommand>,
    packets: Vec<Vec<u8>>,
    read_policy: FrameReadPolicy,
) -> llcontrol_hid::Result<FrameOutcome> {
    let (reply, rx) = oneshot::channel();
    hid.send(HidCommand::FrameBatch {
        packets,
        read_policy,
        reply,
    })
    .await
    .map_err(|_| llcontrol_hid::Error::BlockingTaskFailed("HID actor closed".into()))?;
    rx.await
        .map_err(|_| llcontrol_hid::Error::BlockingTaskFailed("HID reply dropped".into()))?
}

#[cfg(test)]
#[path = "lcd_render_tests.rs"]
mod render_tests;

mod group;
