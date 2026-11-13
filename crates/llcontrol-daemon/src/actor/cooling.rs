//! Sole per-device PWM producer for runtime policies and legacy manual setters.
use super::{
    command_dispatch::SettingsUpdate,
    group_barrier::{GroupBarrier, RuntimeRevision},
    HidCommand, Shutdown,
};
use crate::sensors::SensorCache;
use llcontrol_ipc::{
    CoolingChannelState, CoolingConfig, CoolingPolicy, CoolingState, CoolingTarget, IpcError,
    Result, SensorReading, SensorStatus, SensorUnit,
};
use llcontrol_protocol::galahad2_lcdv2::{
    fan_pwm,
    pump_pwm::{self, PumpPwmSource},
    pwm::Pwm,
};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

pub enum CoolingCommand {
    GroupState(oneshot::Sender<(RuntimeRevision, CoolingState)>),
    PrepareGroup {
        barrier: GroupBarrier,
        group: std::sync::Arc<crate::profile_group::FanAndLightingGroup>,
        reply: oneshot::Sender<Result<()>>,
    },
    InstallGroup {
        barrier: GroupBarrier,
        reply: oneshot::Sender<Result<()>>,
    },
    ApplyGroup {
        barrier: GroupBarrier,
        reply: oneshot::Sender<Result<CoolingState>>,
    },
    FreezeGroup {
        barrier: GroupBarrier,
        reply: oneshot::Sender<Result<()>>,
    },
    Stored {
        target: CoolingTarget,
        expected_revision: String,
        id: String,
        reply: oneshot::Sender<Result<CoolingState>>,
    },
    Set {
        target: CoolingTarget,
        policy: CoolingPolicy,
        reply: oneshot::Sender<Result<CoolingState>>,
    },
    Manual {
        target: CoolingTarget,
        percent: u8,
        motherboard: bool,
        reply: oneshot::Sender<Result<()>>,
    },
    State(oneshot::Sender<CoolingState>),
}

/// Rising temperature takes effect immediately. Falling temperature must move
/// by hysteresis from the last accepted temperature. Invalid data bypasses it.
pub fn evaluate(
    policy: &CoolingPolicy,
    rows: &[SensorReading],
    anchor: &mut Option<f64>,
) -> (u8, bool, bool) {
    match policy {
        CoolingPolicy::FixedRpm { rpm } => (pump_rpm_to_percent(*rpm), false, false),
        CoolingPolicy::CurveRpm {
            sensor_id,
            points,
            hysteresis_c,
            fallback_percent,
        } => {
            let Some(temperature) = accepted_temperature(sensor_id, *hysteresis_c, rows, anchor)
            else {
                return (*fallback_percent, false, true);
            };
            // 2.1.28 service truncates CPU Celsius before RPMConfig, then
            // truncates calculated RPM before passing it through the duty map.
            let temperature = temperature.trunc();
            let Some(first) = points.first() else {
                return (*fallback_percent, false, true);
            };
            let mut rpm = first.rpm;
            for pair in points.windows(2) {
                if temperature <= f64::from(pair[0].celsius) {
                    break;
                }
                rpm = pair[1].rpm;
                if temperature < f64::from(pair[1].celsius) {
                    let slope = (f64::from(pair[1].rpm) - f64::from(pair[0].rpm))
                        / (f64::from(pair[1].celsius) - f64::from(pair[0].celsius));
                    rpm = (slope * (temperature - f64::from(pair[0].celsius))
                        + f64::from(pair[0].rpm)) as u16;
                    break;
                }
            }
            (pump_rpm_to_percent(rpm), false, false)
        }
        CoolingPolicy::Fixed { percent } => (*percent, false, false),
        CoolingPolicy::Motherboard { percent } => (*percent, true, false),
        CoolingPolicy::Curve {
            sensor_id,
            points,
            hysteresis_c,
            fallback_percent,
        }
        | CoolingPolicy::FanCurve {
            sensor_id,
            points,
            hysteresis_c,
            fallback_percent,
        } => {
            let Some(accepted) = accepted_temperature(sensor_id, *hysteresis_c, rows, anchor)
            else {
                return (*fallback_percent, false, true);
            };
            let reference = matches!(policy, CoolingPolicy::FanCurve { .. });
            // 2.1.28 service controller:310-319 casts Celsius before RPMConfig;
            // RPMConfig.cs:16-54 computes slope first and truncates final duty.
            let accepted = if reference {
                accepted.trunc()
            } else {
                accepted
            };
            let Some(first) = points.first() else {
                return (*fallback_percent, false, true);
            };
            if accepted <= first.celsius {
                return (first.percent, false, false);
            }
            for pair in points.windows(2) {
                if accepted <= pair[1].celsius {
                    if reference && accepted == pair[1].celsius {
                        return (pair[1].percent, false, false);
                    }
                    let fraction =
                        (accepted - pair[0].celsius) / (pair[1].celsius - pair[0].celsius);
                    let percent = f64::from(pair[0].percent)
                        + fraction * (f64::from(pair[1].percent) - f64::from(pair[0].percent));
                    let duty = if reference {
                        let slope = (f64::from(pair[1].percent) - f64::from(pair[0].percent))
                            / (pair[1].celsius - pair[0].celsius);
                        (slope * (accepted - pair[0].celsius) + f64::from(pair[0].percent)) as u8
                    } else {
                        percent.ceil() as u8
                    };
                    return (duty, false, false);
                }
            }
            (
                points.last().map_or(*fallback_percent, |p| p.percent),
                false,
                false,
            )
        }
    }
}

fn accepted_temperature(
    sensor_id: &str,
    hysteresis_c: f64,
    rows: &[SensorReading],
    anchor: &mut Option<f64>,
) -> Option<f64> {
    let value = rows
        .iter()
        .find(|r| {
            r.id == sensor_id && r.unit == SensorUnit::Celsius && r.status == SensorStatus::Valid
        })
        .and_then(|r| r.value)
        .filter(|v| v.is_finite());
    let Some(value) = value else {
        *anchor = None;
        return None;
    };
    match *anchor {
        Some(old) if value < old && old - value < hysteresis_c => Some(old),
        _ => {
            *anchor = Some(value);
            Some(value)
        }
    }
}

/// LCDV2 2.1.28 service Galahad2VisionController.cs:139-175;
/// RPMConfig.cs:16-54 truncates interpolation toward zero. This is a target
/// conversion, not measured-RPM calibration. Valid policies constrain input.
pub fn pump_rpm_to_percent(rpm: u16) -> u8 {
    const DUTIES: [u8; 29] = [
        20, 23, 25, 27, 30, 31, 34, 36, 39, 41, 44, 47, 49, 52, 55, 58, 60, 64, 67, 70, 73, 77, 80,
        83, 87, 90, 94, 98, 100,
    ];
    let offset = rpm.clamp(800, 3600) - 800;
    let index = usize::from(offset / 100);
    let start = DUTIES[index];
    let Some(end) = DUTIES.get(index + 1) else {
        return start;
    };
    (f64::from(*end - start) / 100.0 * f64::from(offset % 100) + f64::from(start)) as u8
}

struct Channel {
    state: CoolingChannelState,
    anchor: Option<f64>,
    last_write: Option<Instant>,
    enabled: bool,
}
impl Channel {
    fn new(policy: Option<CoolingPolicy>, target: CoolingTarget, mode_id: Option<String>) -> Self {
        let error = policy
            .as_ref()
            .and_then(|p| p.validate(target).err())
            .map(|e| e.to_string());
        Self {
            enabled: error.is_none(),
            state: CoolingChannelState {
                requested: policy,
                mode_id,
                last_error: error,
                ..Default::default()
            },
            anchor: None,
            last_write: None,
        }
    }
}
struct Controller {
    revision: RuntimeRevision,
    group_barrier: Option<GroupBarrier>,
    prepared_group: Option<(
        GroupBarrier,
        std::sync::Arc<crate::profile_group::FanAndLightingGroup>,
    )>,
    installed_group: Option<(
        GroupBarrier,
        std::sync::Arc<crate::profile_group::FanAndLightingGroup>,
    )>,
    shutdown: Shutdown,
    fan: Channel,
    pump: Channel,
    hid: mpsc::Sender<HidCommand>,
    settings: mpsc::Sender<SettingsUpdate>,
    host: SensorCache,
    device: SensorCache,
}
impl Controller {
    fn state(&self) -> CoolingState {
        CoolingState {
            fan: self.fan.state.clone(),
            pump: self.pump.state.clone(),
        }
    }
    fn channel(&mut self, target: CoolingTarget) -> &mut Channel {
        match target {
            CoolingTarget::Fan => &mut self.fan,
            CoolingTarget::Pump => &mut self.pump,
        }
    }
    fn rows(&self) -> Vec<SensorReading> {
        let mut rows = self.host.readings();
        rows.extend(self.device.readings());
        rows
    }
    async fn tick(&mut self, target: CoolingTarget) {
        if GroupBarrier::held(&mut self.group_barrier) {
            return;
        }
        self.tick_unfrozen(target).await;
    }
    async fn tick_unfrozen(&mut self, target: CoolingTarget) {
        let rows = self.rows();
        let channel = self.channel(target);
        if !channel.enabled {
            return;
        }
        let Some(policy) = channel.state.requested.as_ref() else {
            return;
        };
        if let Some(sensor_id) = policy.sensor_id() {
            if rows
                .iter()
                .any(|r| r.id == sensor_id && r.unit != SensorUnit::Celsius)
            {
                channel.enabled = false;
                channel.state.last_error =
                    Some("persisted binding is not a temperature sensor".into());
                return;
            }
        }
        let (percent, motherboard, fallback) = evaluate(policy, &rows, &mut channel.anchor);
        channel.state.fallback_active = fallback;
        if channel.state.applied_percent == Some(percent)
            && channel.state.motherboard == motherboard
            && channel
                .last_write
                .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
        {
            return;
        }
        let result = write(&self.hid, target, percent, motherboard).await;
        let channel = self.channel(target);
        match result {
            Ok(()) => {
                channel.state.applied_percent = Some(percent);
                channel.state.motherboard = motherboard;
                channel.state.last_error = None;
                channel.last_write = Some(Instant::now());
            }
            Err(e) => {
                channel.state.applied_percent = None;
                channel.state.last_error = Some(e.to_string());
            }
        }
    }
    fn prepare_group(&self, group: &crate::profile_group::FanAndLightingGroup) -> Result<()> {
        group.validate_runtime()?;
        let rows = self.rows();
        for policy in [&group.cooling.fan, &group.cooling.pump]
            .into_iter()
            .flatten()
        {
            if policy.sensor_id().is_some_and(|id| {
                rows.iter()
                    .any(|row| row.id == id && row.unit != SensorUnit::Celsius)
            }) {
                return Err(IpcError::InvalidArgument(
                    "imported cooling binding is not a temperature sensor".into(),
                ));
            }
        }
        Ok(())
    }
    async fn apply_group(&mut self, barrier: GroupBarrier) -> Result<CoolingState> {
        if !GroupBarrier::owns(&mut self.group_barrier, &barrier) {
            return Err(GroupBarrier::busy());
        }
        let group = self
            .installed_group
            .as_ref()
            .filter(|(ticket, _)| ticket.matches(&barrier))
            .map(|(_, group)| group.clone())
            .ok_or_else(GroupBarrier::busy)?;
        for target in [CoolingTarget::Fan, CoolingTarget::Pump] {
            if *self.shutdown.borrow() || self.shutdown.has_changed().is_err() {
                self.channel(target).state.last_error =
                    Some("cooling shutdown interrupted group apply".into());
                continue;
            }
            if self.channel(target).state.requested.is_some() {
                self.tick_unfrozen(target).await;
            } else {
                let (percent, motherboard) = match target {
                    CoolingTarget::Fan => (group.fan.pwm_percent, false),
                    CoolingTarget::Pump => {
                        (group.pump.pwm_percent, group.pump.source == "application")
                    }
                };
                // Preserve legacy unset0, while applying an explicitly saved0.
                let configured = match target {
                    CoolingTarget::Fan => group.fan.has_manual_setting(),
                    CoolingTarget::Pump => group.pump.has_manual_setting(),
                };
                if !configured {
                    continue;
                }
                let result = write(&self.hid, target, percent, motherboard).await;
                let channel = self.channel(target);
                match result {
                    Ok(()) => {
                        channel.state.applied_percent = Some(percent);
                        channel.state.motherboard = motherboard;
                        channel.state.last_error = None;
                        channel.last_write = Some(Instant::now());
                    }
                    Err(error) => {
                        channel.state.applied_percent = None;
                        channel.state.last_error = Some(error.to_string());
                    }
                }
            }
        }
        Ok(self.state())
    }
    async fn set_policy(
        &mut self,
        target: CoolingTarget,
        policy: CoolingPolicy,
        mode_id: Option<String>,
    ) -> Result<CoolingState> {
        let validation = policy.validate(target).and_then(|()| {
            if let Some(sensor_id) = policy.sensor_id() {
                if !self
                    .rows()
                    .iter()
                    .any(|r| r.id == sensor_id && r.unit == SensorUnit::Celsius)
                {
                    return Err(IpcError::InvalidArgument(
                        "binding must identify a known host or this device temperature sensor"
                            .into(),
                    ));
                }
            }
            Ok(())
        });
        validation?;
        *self.channel(target) = Channel::new(Some(policy), target, mode_id);
        self.tick(target).await;
        let config = CoolingConfig {
            fan: self.fan.state.requested.clone(),
            pump: self.pump.state.requested.clone(),
            fan_mode: self.fan.state.mode_id.clone(),
            pump_mode: self.pump.state.mode_id.clone(),
        };
        self.settings
            .send(SettingsUpdate::Cooling(config))
            .await
            .map_err(|_| IpcError::DeviceBusy("settings actor closed".into()))
            .map(|()| self.state())
    }
    async fn command(&mut self, cmd: CoolingCommand) {
        if GroupBarrier::held(&mut self.group_barrier) {
            match cmd {
                CoolingCommand::Set { reply, .. } | CoolingCommand::Stored { reply, .. } => {
                    let _ = reply.send(Err(GroupBarrier::busy()));
                    return;
                }
                CoolingCommand::Manual { reply, .. } => {
                    let _ = reply.send(Err(GroupBarrier::busy()));
                    return;
                }
                _ => {}
            }
        }
        if matches!(
            &cmd,
            CoolingCommand::Set { .. }
                | CoolingCommand::Manual { .. }
                | CoolingCommand::Stored { .. }
        ) {
            self.revision.advance();
        }
        match cmd {
            CoolingCommand::GroupState(reply) => {
                let _ = reply.send((self.revision.clone(), self.state()));
            }
            CoolingCommand::PrepareGroup {
                barrier,
                group,
                reply,
            } => {
                let result = if GroupBarrier::owns(&mut self.group_barrier, &barrier)
                    && !self
                        .installed_group
                        .as_ref()
                        .is_some_and(|(ticket, _)| ticket.matches(&barrier))
                {
                    self.prepared_group = None;
                    self.prepare_group(&group).map(|()| {
                        self.prepared_group = Some((barrier, group));
                    })
                } else {
                    Err(GroupBarrier::busy())
                };
                let _ = reply.send(result);
            }
            CoolingCommand::InstallGroup { barrier, reply } => {
                let result = if GroupBarrier::owns(&mut self.group_barrier, &barrier)
                    && self
                        .prepared_group
                        .as_ref()
                        .is_some_and(|(ticket, _)| ticket.matches(&barrier))
                {
                    let prepared = self.prepared_group.take().expect("checked prepared group");
                    self.revision.advance();
                    self.fan = Channel::new(
                        prepared.1.cooling.fan.clone(),
                        CoolingTarget::Fan,
                        prepared.1.cooling.fan_mode.clone(),
                    );
                    self.pump = Channel::new(
                        prepared.1.cooling.pump.clone(),
                        CoolingTarget::Pump,
                        prepared.1.cooling.pump_mode.clone(),
                    );
                    self.installed_group = Some(prepared);
                    Ok(())
                } else {
                    Err(GroupBarrier::busy())
                };
                let _ = reply.send(result);
            }
            CoolingCommand::ApplyGroup { barrier, reply } => {
                let result = self.apply_group(barrier).await;
                let _ = reply.send(result);
            }
            CoolingCommand::FreezeGroup { barrier, reply } => {
                let _ = reply.send(GroupBarrier::acquire(&mut self.group_barrier, barrier));
            }
            CoolingCommand::State(reply) => {
                let _ = reply.send(self.state());
            }
            CoolingCommand::Set {
                target,
                policy,
                reply,
            } => {
                let result = self.set_policy(target, policy, None).await;
                let _ = reply.send(result);
            }
            CoolingCommand::Stored {
                target,
                expected_revision,
                id,
                reply,
            } => {
                let (resolved, rx) = oneshot::channel();
                let result = async {
                    self.settings
                        .send(SettingsUpdate::ResolveCoolingMode {
                            target,
                            expected_revision,
                            id,
                            reply: resolved,
                        })
                        .await
                        .map_err(|_| IpcError::DeviceBusy("settings actor closed".into()))?;
                    let mode = rx
                        .await
                        .map_err(|_| IpcError::DeviceBusy("mode resolution dropped".into()))?
                        .map_err(IpcError::DeviceBusy)?;
                    self.set_policy(target, mode.policy, Some(mode.id)).await
                }
                .await;
                let _ = reply.send(result);
            }
            CoolingCommand::Manual {
                target,
                percent,
                motherboard,
                reply,
            } => {
                let result = write(&self.hid, target, percent, motherboard).await;
                if let Err(e) = result {
                    let _ = reply.send(Err(e));
                    return;
                }
                let channel = self.channel(target);
                *channel = Channel::new(None, target, None);
                channel.state.applied_percent = Some(percent);
                channel.state.motherboard = motherboard;
                let result = self
                    .settings
                    .send(SettingsUpdate::CoolingManual {
                        target,
                        percent,
                        motherboard,
                    })
                    .await
                    .map_err(|_| IpcError::DeviceBusy("settings actor closed".into()));
                let _ = reply.send(result);
            }
        }
    }
}
async fn write(
    hid: &mpsc::Sender<HidCommand>,
    target: CoolingTarget,
    percent: u8,
    motherboard: bool,
) -> Result<()> {
    let pwm = Pwm::new(percent).map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
    if target == CoolingTarget::Fan && motherboard {
        return Err(IpcError::InvalidArgument(
            "fan motherboard mode unsupported".into(),
        ));
    }
    let bytes = match target {
        CoolingTarget::Fan => fan_pwm::build_request(pwm).to_vec(),
        CoolingTarget::Pump => pump_pwm::build_request(
            if motherboard {
                PumpPwmSource::Motherboard
            } else {
                PumpPwmSource::Mcu
            },
            pwm,
        )
        .to_vec(),
    };
    let (reply, rx) = oneshot::channel();
    hid.send(HidCommand::Write { bytes, reply })
        .await
        .map_err(|_| IpcError::DeviceBusy("HID actor closed".into()))?;
    rx.await
        .map_err(|_| IpcError::DeviceBusy("HID reply dropped".into()))?
        .map_err(|e| IpcError::DeviceBusy(e.to_string()))
}

pub struct CoolingInputs {
    pub hid: mpsc::Sender<HidCommand>,
    pub settings: mpsc::Sender<SettingsUpdate>,
    pub host: SensorCache,
    pub device: SensorCache,
    pub initial: CoolingConfig,
}
pub fn spawn(
    inputs: CoolingInputs,
    mut shutdown: Shutdown,
) -> (mpsc::Sender<CoolingCommand>, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::channel(16);
    let task = tokio::spawn(async move {
        let mut c = Controller {
            revision: RuntimeRevision::new(),
            group_barrier: None,
            prepared_group: None,
            installed_group: None,
            shutdown: shutdown.clone(),
            fan: Channel::new(
                inputs.initial.fan,
                CoolingTarget::Fan,
                inputs.initial.fan_mode,
            ),
            pump: Channel::new(
                inputs.initial.pump,
                CoolingTarget::Pump,
                inputs.initial.pump_mode,
            ),
            hid: inputs.hid,
            settings: inputs.settings,
            host: inputs.host,
            device: inputs.device,
        };
        let mut next_tick = tokio::time::Instant::now();
        let mut command_turn = true;
        loop {
            if *shutdown.borrow() || shutdown.has_changed().is_err() {
                break;
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                cmd = rx.recv(), if command_turn || tokio::time::Instant::now() < next_tick => {
                    match cmd { Some(cmd) => c.command(cmd).await, None => break }
                    command_turn = false;
                },
                _ = tokio::time::sleep_until(next_tick) => {
                    c.tick(CoolingTarget::Fan).await;
                    if *shutdown.borrow() || shutdown.has_changed().is_err() { break; }
                    c.tick(CoolingTarget::Pump).await;
                    next_tick = tokio::time::Instant::now() + Duration::from_secs(1);
                    command_turn = true;
                },
            }
        }
    });
    (tx, task)
}
