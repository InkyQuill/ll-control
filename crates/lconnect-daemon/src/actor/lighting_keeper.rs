//! Sole RGB state owner: serializes manual modes, direct LED data and refresh.
//!
//! Reissue preserves the LCDV2 FW1.6 cmd131 buffering workaround. Failed
//! refreshes retry on the next cadence; manual commands return their error.
//! All device access remains in the HID actor.

use super::{command_dispatch::SettingsUpdate, HidCommand, LightingStateMsg, Shutdown};
use super::{
    group_barrier::{GroupBarrier, RuntimeRevision},
    thermal::Evaluator,
};
use crate::codec_str::{
    parse_direction, parse_fan_mode, parse_hex_color, parse_pump_mode, parse_pump_scope,
    percent_to_wire,
};
use crate::persistence::{LightingState, PumpLightingState};
use crate::sensors::SensorCache;
use lconnect_ipc::{
    IpcError, LedTarget, LightingKind, Result, SensorUnit, ThermalConfig, ThermalLayer,
    ThermalState, ThermalZoneState,
};
use lconnect_protocol::galahad2_lcdv2::lighting::{FanLightingSetting, PumpLightingSetting};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::ArgbSignalSource;
use lconnect_protocol::galahad2_lcdv2::set_led_colors::{
    build_set_fan_led_colors_request, build_set_screen_led_colors_request,
};
use lconnect_protocol::galahad2_lcdv2::set_lighting::{
    build_set_fan_lighting_request, build_set_pump_lighting_request,
};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::warn;

/// Spawn the sole per-device RGB producer. A command acknowledges only after
/// its HID write and, for persistent modes, its settings delta are accepted.
/// Direct colors are retained for refresh during this process, but are not
/// persisted. Clear stops refresh without changing the physical LEDs or disk.
#[derive(Default)]
pub struct ThermalInputs {
    pub dynamic_profiles: lconnect_ipc::DynamicRgbProfiles,
    pub dynamic: Option<lconnect_ipc::DynamicRgbConfig>,
    pub config: ThermalConfig,
    pub host: SensorCache,
    pub device: SensorCache,
}

pub fn spawn(
    hid_io_tx: mpsc::Sender<HidCommand>,
    reissue_ms: u64,
    initial_fan: Option<LightingState>,
    initial_pump: Option<PumpLightingState>,
    state_rx: mpsc::Receiver<LightingStateMsg>,
    settings: mpsc::Sender<SettingsUpdate>,
    shutdown: Shutdown,
) -> JoinHandle<()> {
    spawn_with_thermal(
        hid_io_tx,
        reissue_ms,
        initial_fan,
        initial_pump,
        ThermalInputs::default(),
        state_rx,
        settings,
        shutdown,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_with_thermal(
    hid_io_tx: mpsc::Sender<HidCommand>,
    reissue_ms: u64,
    initial_fan: Option<LightingState>,
    initial_pump: Option<PumpLightingState>,
    thermal: ThermalInputs,
    state_rx: mpsc::Receiver<LightingStateMsg>,
    settings: mpsc::Sender<SettingsUpdate>,
    shutdown: Shutdown,
) -> JoinHandle<()> {
    spawn_with_profiles(
        hid_io_tx,
        reissue_ms,
        crate::persistence::LightingSection {
            fan_topology: initial_fan
                .as_ref()
                .and_then(|s| s.fan_topology.clone())
                .unwrap_or_default(),
            fan: initial_fan,
            pump: initial_pump,
            ..Default::default()
        },
        thermal,
        state_rx,
        settings,
        shutdown,
    )
}

pub fn spawn_with_profiles(
    hid_io_tx: mpsc::Sender<HidCommand>,
    reissue_ms: u64,
    initial: crate::persistence::LightingSection,
    mut thermal: ThermalInputs,
    mut state_rx: mpsc::Receiver<LightingStateMsg>,
    settings: mpsc::Sender<SettingsUpdate>,
    mut shutdown: Shutdown,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut revision = RuntimeRevision::new();
        let mut group_barrier = None;
        let mut prepared_group: Option<(GroupBarrier, PreparedLightingGroup)> = None;
        let mut installed_group: Option<GroupBarrier> = None;
        let mut fan_topology = initial.fan_topology;
        let mut fan = initial.fan.map(|mut s| {
            s.fan_topology = Some(fan_topology.clone());
            Output::Fan(s)
        });
        let mut pump = initial.pump.map(Output::Pump);
        let mut profiles = std::collections::BTreeMap::new();
        for output in initial
            .fan_modes
            .into_values()
            .map(Output::Fan)
            .chain(initial.pump_modes.into_values().map(Output::Pump))
            .chain(fan.iter().chain(pump.iter()).filter_map(|o| match o {
                Output::Fan(s) => Some(Output::Fan(s.clone())),
                Output::Pump(s) => Some(Output::Pump(s.clone())),
                _ => None,
            }))
        {
            remember_mode(&mut profiles, &output);
        }
        if let (Some(config), Some(Output::Pump(base))) = (&thermal.dynamic, &pump) {
            let _ = thermal
                .dynamic_profiles
                .remember(&base.mode, config.clone());
        }
        let cadence = Duration::from_millis(reissue_ms.max(1));
        let mut thermal = ThermalRuntime::new(thermal, cadence);
        let tick_period = cadence.min(Duration::from_secs(1));
        let mut next_tick = tokio::time::Instant::now();
        let mut command_turn = true;
        loop {
            if *shutdown.borrow() || shutdown.has_changed().is_err() {
                break;
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                msg = state_rx.recv(), if command_turn || tokio::time::Instant::now() < next_tick => {
                    let Some(msg) = msg else { break; };
                    if GroupBarrier::held(&mut group_barrier) {
                        command_turn = false;
                        match msg {
                            LightingStateMsg::SetDynamicRgb {reply,..} => { let _=reply.send(Err(GroupBarrier::busy())); continue; }
                            LightingStateMsg::SetThermal {reply,..} => { let _=reply.send(Err(GroupBarrier::busy())); continue; }
                            LightingStateMsg::SetFan {reply,..} | LightingStateMsg::SetPump {reply,..} |
                            LightingStateMsg::SetPumpWithDynamic {reply,..} | LightingStateMsg::Direct {reply,..} |
                            LightingStateMsg::Clear {reply,..} => { let _=reply.send(Err(GroupBarrier::busy())); continue; }
                            _=>{}
                        }
                    }
                    // Admission of a newer user mutation invalidates old reapply
                    // tokens even if that request later fails validation or HID.
                    if matches!(&msg,LightingStateMsg::SetDynamicRgb {..} | LightingStateMsg::SetThermal {..} |
                        LightingStateMsg::SetFan {..} | LightingStateMsg::SetPump {..} | LightingStateMsg::SetPumpWithDynamic {..} |
                        LightingStateMsg::Direct {..} | LightingStateMsg::Clear {..}) {revision.advance();}
                    match msg {
                        LightingStateMsg::GroupState(reply)=>{let _=reply.send((revision.clone(),super::LightingRuntimeState {thermal:thermal.snapshot(),dynamic:thermal.dynamic_snapshot(&pump)}));}
                        LightingStateMsg::PrepareGroup {barrier,group,reply} => {
                            let result=if GroupBarrier::owns(&mut group_barrier,&barrier) && !installed_group.as_ref().is_some_and(|ticket|ticket.matches(&barrier)) {
                                prepared_group=None;
                                PreparedLightingGroup::new(&group,&thermal).map(|prepared|{prepared_group=Some((barrier,prepared));})
                            } else {Err(GroupBarrier::busy())};
                            let _=reply.send(result);
                        }
                        LightingStateMsg::InstallGroup {barrier,reply} => {
                            let result=if GroupBarrier::owns(&mut group_barrier,&barrier) && prepared_group.as_ref().is_some_and(|(ticket,_)|ticket.matches(&barrier)) {
                                let (ticket,prepared)=prepared_group.take().expect("checked lighting preparation");
                                revision.advance();
                                fan_topology=prepared.topology;fan=prepared.fan;pump=prepared.pump;profiles=prepared.profiles;thermal=prepared.thermal;
                                installed_group=Some(ticket);Ok(())
                            } else {Err(GroupBarrier::busy())};
                            let _=reply.send(result);
                        }
                        LightingStateMsg::ApplyGroup {barrier,reply} => {
                            let result=if GroupBarrier::owns(&mut group_barrier,&barrier) && installed_group.as_ref().is_some_and(|ticket|ticket.matches(&barrier)) {
                                thermal.refresh(&hid_io_tx,&fan,&pump,&shutdown,true).await;
                                Ok(super::LightingRuntimeState {thermal:thermal.snapshot(),dynamic:thermal.dynamic_snapshot(&pump)})
                            } else {Err(GroupBarrier::busy())};
                            let _=reply.send(result);
                        }
                        LightingStateMsg::FreezeGroup {barrier,reply} => {
                            let _=reply.send(GroupBarrier::acquire(&mut group_barrier,barrier));
                        }
                        LightingStateMsg::Base {kind, reply} => {
                            let output=match kind {LightingKind::Fan=>fan.as_ref(),LightingKind::Pump=>pump.as_ref(),_=>None};
                            let annotate = |mut cfg:lconnect_ipc::LightingConfig| {
                                if kind == LightingKind::Fan { cfg.fan_topology=Some(fan_topology.clone()); }
                                else { cfg.dynamic_mode=Some(thermal.dynamic.profiles.selected(&cfg.mode).is_some_and(|p|p.enabled)); }
                                cfg
                            };
                            let _=reply.send(super::LightingBase {fan_topology:if kind==LightingKind::Fan {Some(fan_topology.clone())} else {None},current:output.and_then(Output::base_config).map(annotate),profiles:profiles.iter().filter(|((k,_),_)|*k==(kind==LightingKind::Fan)).map(|(_,cfg)|annotate(cfg.clone())).collect()});
                        }
                        LightingStateMsg::DynamicRgbState(reply) => { let _ = reply.send(thermal.dynamic_snapshot(&pump)); }
                        LightingStateMsg::SetDynamicRgb { config, reply } => {
                            let result = match thermal.validate_dynamic(config.as_ref(), &pump, true) {
                                Err(e) => Err(e),
                                Ok(()) => {
                                    let mut profiles=thermal.dynamic.profiles.clone();
                                    let retained=match (&config,&pump) {
                                        (Some(config),Some(Output::Pump(base)))=>profiles.remember(&base.mode,config.clone()),
                                        (None,Some(Output::Pump(base)))=>{profiles.disable(&base.mode);Ok(())},
                                        _=>Ok(())
                                    };
                                    if let Err(e)=retained {let _=reply.send(Err(e));continue;}
                                    thermal.dynamic = lconnect_ipc::DynamicRgbState { requested: config.clone(),profiles, ..Default::default() };
                                    thermal.refresh(&hid_io_tx, &fan, &pump, &shutdown, true).await;
                                    settings.send(SettingsUpdate::DynamicRgbSnapshot {config,profiles:thermal.dynamic.profiles.clone()}).await
                                        .map_err(|_| IpcError::DeviceBusy("settings actor closed after dynamic RGB update".into()))
                                        .map(|()| thermal.dynamic_snapshot(&pump))
                                }
                            };
                            let _ = reply.send(result);
                        }
                        LightingStateMsg::ThermalState(reply) => { let _ = reply.send(thermal.snapshot()); }
                        LightingStateMsg::SetThermal { config, reply } => {
                            let validation = thermal.validate(&config, &fan, &pump, true);
                            let result = match validation {
                                Err(e) => Err(e),
                                Ok(()) => {
                                    thermal.evaluator.reconfigure(&thermal.state.requested, &config);
                                    thermal.state.requested = config.clone();
                                    thermal.grace = None;
                                    thermal.refresh(&hid_io_tx, &fan, &pump, &shutdown, true).await;
                                    settings.send(SettingsUpdate::Thermal(config)).await
                                        .map_err(|_| IpcError::DeviceBusy("settings actor closed after thermal update".into()))
                                        .map(|()| thermal.snapshot())
                                }
                            };
                            let _ = reply.send(result);
                        }
                        LightingStateMsg::Clear { kind, reply } => {
                            let result = if thermal.locked() || (kind == LightingKind::Pump && thermal.dynamic.requested.is_some()) {
                                Err(IpcError::InvalidArgument("disable thermal/dynamic policies and restore base before clearing lighting".into()))
                            } else { match kind {
                                LightingKind::Fan => { fan = None; thermal.state.fan = Default::default(); Ok(()) },
                                LightingKind::Pump => { pump = None; thermal.state.pump = Default::default(); Ok(()) },
                                _ => Err(IpcError::InvalidArgument("unsupported lighting kind".into())),
                            }};
                            let _ = reply.send(result);
                        }
                        other => {
                            let mut restore_dynamic = false;
                            let (output, reply) = match other {
                                LightingStateMsg::SetFan { mut state, reply } => {
                                    let topology=state.fan_topology.clone().unwrap_or_else(||fan_topology.clone());
                                    let output=topology.validate().map(|()|{state.fan_topology=Some(topology);Output::Fan(state)});
                                    (output,reply)
                                },
                                LightingStateMsg::SetPump { state, reply } => (Ok(Output::Pump(state)), reply),
                                LightingStateMsg::SetPumpWithDynamic { state, restore_dynamic: restore, reply } => {
                                    restore_dynamic = restore;
                                    (Ok(Output::Pump(state)), reply)
                                },
                                LightingStateMsg::Direct { target, colors, reply } => (
                                    direct_output(target, colors, match target { LedTarget::Fan => fan.as_ref(), _ => pump.as_ref() }), reply),
                                LightingStateMsg::GroupState(_) | LightingStateMsg::PrepareGroup { .. } | LightingStateMsg::InstallGroup { .. } | LightingStateMsg::ApplyGroup { .. } | LightingStateMsg::FreezeGroup { .. } | LightingStateMsg::Base { .. } | LightingStateMsg::DynamicRgbState(_) | LightingStateMsg::SetDynamicRgb { .. } | LightingStateMsg::Clear { .. } | LightingStateMsg::SetThermal { .. } | LightingStateMsg::ThermalState(_) => unreachable!("handled above"),
                            };
                            let result = match output {
                                Err(e) => Err(e),
                                Ok(output) => {
                                    let selected_policy = if restore_dynamic {
                                        let Output::Pump(base) = &output else { unreachable!("pump restore message") };
                                        thermal.dynamic.profiles.selected(&base.mode).and_then(|p|p.bindings.last()).cloned()
                                            .ok_or_else(||IpcError::InvalidArgument("this pump mode has no saved dynamic source".into()))
                                            .and_then(|cfg| thermal.validate_dynamic(Some(&cfg), &Some(Output::Pump(base.clone())), false).map(|()|Some(cfg)))
                                    } else { Ok(None) };
                                    let selected_policy = match selected_policy {Ok(cfg)=>cfg,Err(e)=>{let _=reply.send(Err(e));continue;}};
                                    let mut retained_profiles=thermal.dynamic.profiles.clone();
                                    if let Output::Pump(base)=&output {
                                        let retained=if let Some(cfg)=&selected_policy {retained_profiles.remember(&base.mode,cfg.clone())}
                                            else {retained_profiles.disable(&base.mode);Ok(())};
                                        if let Err(e)=retained {let _=reply.send(Err(e));continue;}
                                    } else if output.kind()==LightingKind::Pump {
                                        if let Some(Output::Pump(base))=&pump {retained_profiles.disable(&base.mode);}
                                    }
                                    // Restore both zones first, including an old pump scope,
                                    // before allowing a manual mode to replace its base.
                                    let restore = thermal.restore_overlay(&hid_io_tx, &fan, &pump, &shutdown).await;
                                    let write_result = match restore {
                                        Ok(()) => {
                                            let old = if output.kind() == LightingKind::Fan { &fan } else { &pump };
                                            let initialize = matches!(output, Output::Direct { .. }) &&
                                                (!matches!(old, Some(Output::Direct { .. })) || thermal.zone(output.kind()).applied.is_none());
                                            match output.prepare_direct(&hid_io_tx, initialize).await {
                                                Ok(()) => output.write(&hid_io_tx).await,
                                                Err(e) => Err(e),
                                            }
                                        }, Err(e) => Err(e)
                                    };
                                    match write_result {
                                        Err(e) => {
                                            if output.kind() == LightingKind::Pump && thermal.dynamic.requested.is_some() {
                                                thermal.dynamic.applied=None;
                                                thermal.dynamic.last_error=Some(e.to_string());
                                            }
                                            thermal.zone(output.kind()).applied = None;
                                            thermal.zone(output.kind()).last_error = Some(e.to_string());
                                            Err(e)
                                        },
                                        Ok(()) => {
                                            *thermal.zone(output.kind()) = ThermalZoneState { applied: Some(ThermalLayer::Base), last_error: None, direct_led_count: output.direct_led_count() };
                                            thermal.grace = Some(tokio::time::Instant::now() + Duration::from_secs(10));
                                            if let Output::Fan(s)=&output {fan_topology=s.fan_topology.clone().unwrap_or_default();}
                                            remember_mode(&mut profiles,&output);
                                            let pump_changed = output.kind() == LightingKind::Pump;
                                            let mut update = output.settings_update();
                                            if pump_changed {
                                                let policy_changed=thermal.dynamic.requested.is_some() || thermal.dynamic.profiles!=retained_profiles;
                                                thermal.dynamic = lconnect_ipc::DynamicRgbState {requested:selected_policy.clone(), profiles:retained_profiles,..Default::default()};
                                                update=match &output {
                                                    Output::Pump(base)=>Some(SettingsUpdate::LightingPumpDynamic {state:base.clone(),config:selected_policy,profiles:thermal.dynamic.profiles.clone()}),
                                                    _ if policy_changed=>Some(SettingsUpdate::DynamicRgbSnapshot {config:None,profiles:thermal.dynamic.profiles.clone()}),
                                                    _=>None
                                                };
                                            }

                                            // The write is now real even if the settings actor has
                                            // closed. Keep refresh consistent with that write.
                                            match output.kind() {
                                                LightingKind::Fan => fan = Some(output),
                                                _ => pump = Some(output),
                                            }
                                            let persisted = match update {
                                                Some(update) => settings.send(update).await.map_err(|_| IpcError::DeviceBusy("settings actor closed after lighting write".into())),
                                                None => Ok(()),
                                            };
                                            if restore_dynamic {
                                                thermal.refresh(&hid_io_tx, &fan, &pump, &shutdown, true).await;
                                            }
                                            persisted.and_then(|()| if restore_dynamic {
                                                match &thermal.dynamic.last_error {Some(e)=>Err(IpcError::DeviceBusy(e.clone())),None=>Ok(())}
                                            } else {Ok(())})
                                        }
                                    }
                                }
                            };
                            let _ = reply.send(result);
                        }
                    }
                    command_turn = false;
                },
                _ = tokio::time::sleep_until(next_tick) => {
                    if !GroupBarrier::held(&mut group_barrier) {
                        thermal.refresh(&hid_io_tx, &fan, &pump, &shutdown, false).await;
                    }
                    next_tick = tokio::time::Instant::now() + tick_period;
                    command_turn = true;
                },
            }
        }
    })
}

struct PreparedLightingGroup {
    topology: lconnect_ipc::FanLightingTopology,
    fan: Option<Output>,
    pump: Option<Output>,
    profiles: std::collections::BTreeMap<(bool, String), lconnect_ipc::LightingConfig>,
    thermal: ThermalRuntime,
}
impl PreparedLightingGroup {
    fn new(
        group: &crate::profile_group::FanAndLightingGroup,
        current: &ThermalRuntime,
    ) -> Result<Self> {
        group.validate_runtime()?;
        let topology = group.lighting.fan_topology.clone();
        let fan = group.lighting.fan.clone().map(|mut state| {
            state.fan_topology = Some(topology.clone());
            Output::Fan(state)
        });
        let pump = group.lighting.pump.clone().map(Output::Pump);
        let mut profiles = std::collections::BTreeMap::new();
        for output in group
            .lighting
            .fan_modes
            .values()
            .cloned()
            .map(Output::Fan)
            .chain(
                group
                    .lighting
                    .pump_modes
                    .values()
                    .cloned()
                    .map(Output::Pump),
            )
            .chain(
                fan.iter()
                    .chain(pump.iter())
                    .filter_map(|output| match output {
                        Output::Fan(state) => Some(Output::Fan(state.clone())),
                        Output::Pump(state) => Some(Output::Pump(state.clone())),
                        _ => None,
                    }),
            )
        {
            remember_mode(&mut profiles, &output);
        }
        let mut dynamic_profiles = group.dynamic_rgb_profiles.clone();
        if let (Some(config), Some(Output::Pump(base))) = (&group.dynamic_rgb, &pump) {
            dynamic_profiles.remember(&base.mode, config.clone())?;
        }
        let mut thermal = ThermalRuntime::new(
            ThermalInputs {
                config: current.state.requested.clone(), // Global alerts do not travel with this group.
                dynamic: group.dynamic_rgb.clone(),
                dynamic_profiles,
                host: current.host.clone(),
                device: current.device.clone(),
            },
            current.cadence,
        );
        // Global warnings retain history even when samples disappear during
        // transfer. Replacing bases must not reset an active thermal warning.
        thermal.evaluator = current.evaluator.clone();
        thermal.grace = current.grace;
        thermal.state.active_source = current.state.active_source;
        thermal.state.degraded_sources = current.state.degraded_sources.clone();
        if (current.overlay_pending[0] && fan.is_none())
            || (current.overlay_pending[1] && pump.is_none())
        {
            return Err(IpcError::InvalidArgument(
                "restore retained thermal overlay before removing its lighting base".into(),
            ));
        }
        thermal.validate(&thermal.state.requested, &fan, &pump, false)?;
        thermal.validate_dynamic(thermal.dynamic.requested.as_ref(), &pump, false)?;
        Ok(Self {
            topology,
            fan,
            pump,
            profiles,
            thermal,
        })
    }
}

struct ThermalRuntime {
    dynamic: lconnect_ipc::DynamicRgbState,
    state: ThermalState,
    evaluator: Evaluator,
    host: SensorCache,
    device: SensorCache,
    grace: Option<tokio::time::Instant>,
    cadence: Duration,
    last_refresh: Option<tokio::time::Instant>,
    overlay_pending: [bool; 2],
}
impl ThermalRuntime {
    fn new(inputs: ThermalInputs, cadence: Duration) -> Self {
        Self {
            dynamic: lconnect_ipc::DynamicRgbState {
                requested: inputs.dynamic,
                profiles: inputs.dynamic_profiles,
                ..Default::default()
            },
            state: ThermalState {
                requested: inputs.config,
                ..Default::default()
            },
            evaluator: Evaluator::default(),
            host: inputs.host,
            device: inputs.device,
            grace: None,
            cadence,
            last_refresh: None,
            overlay_pending: [false; 2],
        }
    }
    fn snapshot(&self) -> ThermalState {
        let mut state = self.state.clone();
        state.grace_remaining_ms = self.grace.map_or(0, |until| {
            until
                .saturating_duration_since(tokio::time::Instant::now())
                .as_millis() as u64
        });
        state
    }
    fn rows(&self) -> Vec<lconnect_ipc::SensorReading> {
        let mut rows = self.host.readings();
        rows.extend(self.device.readings());
        rows
    }
    fn validate(
        &self,
        config: &ThermalConfig,
        fan: &Option<Output>,
        pump: &Option<Output>,
        require_known: bool,
    ) -> Result<()> {
        config.validate()?;
        if !config.enabled() {
            return Ok(());
        }
        if fan.is_none() || pump.is_none() {
            return Err(IpcError::InvalidArgument(
                "thermal alerts require known fan and pump lighting bases".into(),
            ));
        }
        let rows = self.rows();
        for rule in [&config.cpu, &config.gpu].into_iter().flatten() {
            let row = rows.iter().find(|r| r.id == rule.sensor_id);
            if row.is_some_and(|r| r.unit != SensorUnit::Celsius)
                || (require_known && row.is_none())
            {
                return Err(IpcError::InvalidArgument(
                    "thermal binding must be a known host or this device Celsius sensor".into(),
                ));
            }
        }
        Ok(())
    }
    fn dynamic_snapshot(&self, pump: &Option<Output>) -> lconnect_ipc::DynamicRgbState {
        let mut state = self.dynamic.clone();
        if let Some(Output::Pump(base)) = pump {
            state.base_mode = Some(base.mode.clone());
            if let Some((count, speed)) = lconnect_ipc::dynamic_rgb_controls(&base.mode) {
                state.color_count = Some(count);
                state.speed_supported = speed;
                if let Some(mode) = lconnect_ipc::lighting_catalog(LightingKind::Pump)
                    .into_iter()
                    .find(|m| m.mode == lconnect_ipc::canonical_lighting_mode(&base.mode))
                {
                    state.direction_supported = mode.direction;
                    state.mode_defaults = Some(mode.defaults);
                }
            }
        }
        state
    }
    fn validate_dynamic(
        &self,
        config: Option<&lconnect_ipc::DynamicRgbConfig>,
        pump: &Option<Output>,
        require_known: bool,
    ) -> Result<()> {
        let Some(config) = config else { return Ok(()) };
        config.validate()?;
        let Some(Output::Pump(base)) = pump else {
            return Err(IpcError::InvalidArgument(
                "dynamic RGB requires a known pump mode base".into(),
            ));
        };
        config.validate_mode(&base.mode)?;
        let rows = self.rows();
        let row = rows.iter().find(|r| r.id == config.sensor_id);
        if row.is_some_and(|r| r.unit != config.unit) || (require_known && row.is_none()) {
            return Err(IpcError::InvalidArgument(
                "dynamic RGB requires a known matching sensor unit".into(),
            ));
        }
        Ok(())
    }
    async fn write_base(&mut self, hid: &mpsc::Sender<HidCommand>, base: &Output) -> Result<()> {
        if let (Output::Pump(state), Some(config)) = (base, self.dynamic.requested.as_ref()) {
            self.dynamic.applied = None;
            let valid = config.validate_mode(&state.mode).is_ok();
            let sample = if valid {
                super::dynamic_rgb::evaluate(config, &self.rows())
            } else {
                None
            };
            self.dynamic.degraded = sample.is_none();
            if let Some(sample) = sample {
                let mut setting = pump_setting_from_state(state);
                for (field, color) in [
                    &mut setting.color1,
                    &mut setting.color2,
                    &mut setting.color3,
                    &mut setting.color4,
                ]
                .into_iter()
                .zip(&sample.colors)
                {
                    *field = lconnect_protocol::galahad2_lcdv2::Rgb {
                        r: color.r,
                        g: color.g,
                        b: color.b,
                    };
                }
                if let Some(speed) = sample.speed_percent {
                    setting.speed = percent_to_wire(speed);
                }
                if let Some(brightness) = config.brightness_percent {
                    setting.brightness = percent_to_wire(brightness);
                }
                if let Some(direction) = &config.direction {
                    setting.direction = parse_direction(direction);
                }
                write(hid, build_set_pump_lighting_request(setting).to_vec()).await?;
                self.dynamic.applied = Some(sample);
                return Ok(());
            }
        }
        base.write(hid).await
    }
    fn locked(&self) -> bool {
        self.state.requested.enabled() || self.overlay_pending.iter().any(|pending| *pending)
    }
    fn zone_index(kind: LightingKind) -> usize {
        usize::from(kind != LightingKind::Fan)
    }
    fn zone(&mut self, kind: LightingKind) -> &mut ThermalZoneState {
        if kind == LightingKind::Fan {
            &mut self.state.fan
        } else {
            &mut self.state.pump
        }
    }
    async fn apply(
        &mut self,
        hid: &mpsc::Sender<HidCommand>,
        base: &Output,
        layer: ThermalLayer,
    ) -> Result<()> {
        let index = Self::zone_index(base.kind());
        if matches!(layer, ThermalLayer::Alert { .. }) {
            self.overlay_pending[index] = true;
        }
        let prepared = base
            .prepare_direct(hid, self.zone(base.kind()).applied.is_none())
            .await;
        let result = match prepared {
            Err(e) => Err(e),
            Ok(()) => match &layer {
                ThermalLayer::Base => self.write_base(hid, base).await,
                ThermalLayer::Alert { color } => match base.alert_packet(*color) {
                    Ok(packet) => write(hid, packet).await,
                    Err(e) => Err(e),
                },
            },
        };
        if base.kind() == LightingKind::Pump && self.dynamic.requested.is_some() {
            self.dynamic.suppressed_by_thermal = matches!(layer, ThermalLayer::Alert { .. });
            if self.dynamic.suppressed_by_thermal || result.is_err() {
                self.dynamic.applied = None;
            }
            self.dynamic.last_error = result.as_ref().err().map(ToString::to_string);
        }
        if result.is_ok() && layer == ThermalLayer::Base {
            self.overlay_pending[index] = false;
        }
        let zone = self.zone(base.kind());
        zone.direct_led_count = base.direct_led_count();
        match &result {
            Ok(()) => {
                zone.applied = Some(layer);
                zone.last_error = None;
            }
            Err(e) => {
                zone.applied = None;
                zone.last_error = Some(e.to_string());
            }
        }
        result
    }
    async fn restore_overlay(
        &mut self,
        hid: &mpsc::Sender<HidCommand>,
        fan: &Option<Output>,
        pump: &Option<Output>,
        shutdown: &Shutdown,
    ) -> Result<()> {
        for base in [fan, pump].into_iter().flatten() {
            if self.overlay_pending[Self::zone_index(base.kind())] {
                if *shutdown.borrow() || shutdown.has_changed().is_err() {
                    return Err(IpcError::DeviceBusy("lighting shutting down".into()));
                }
                self.apply(hid, base, ThermalLayer::Base).await?;
            }
        }
        Ok(())
    }
    async fn refresh(
        &mut self,
        hid: &mpsc::Sender<HidCommand>,
        fan: &Option<Output>,
        pump: &Option<Output>,
        shutdown: &Shutdown,
        force: bool,
    ) {
        let dynamic_error = self
            .validate_dynamic(self.dynamic.requested.as_ref(), pump, false)
            .err()
            .map(|e| e.to_string());
        self.dynamic.degraded =
            dynamic_error.is_some()
                || self.dynamic.requested.as_ref().is_some_and(|config| {
                    super::dynamic_rgb::evaluate(config, &self.rows()).is_none()
                });
        if dynamic_error.is_some() {
            self.dynamic.last_error = dynamic_error.clone();
            self.dynamic.degraded = true;
            self.dynamic.applied = None;
        }
        self.state.blocked_reason = self
            .validate(&self.state.requested, fan, pump, false)
            .err()
            .map(|e| e.to_string());
        let active = if self.state.blocked_reason.is_none() {
            self.evaluator.evaluate(&self.state.requested, &self.rows())
        } else {
            None
        };
        self.state.active_source = active.map(|(source, _)| source);
        self.state.degraded_sources = self.evaluator.degraded.clone();
        let layer = match active {
            Some((_, color)) if self.snapshot().grace_remaining_ms == 0 => {
                ThermalLayer::Alert { color }
            }
            _ => ThermalLayer::Base,
        };
        let due = force
            || self
                .last_refresh
                .is_none_or(|t| t.elapsed() >= self.cadence);
        for base in [fan, pump].into_iter().flatten() {
            if *shutdown.borrow() || shutdown.has_changed().is_err() {
                let zone = self.zone(base.kind());
                zone.last_error = Some("lighting shutdown interrupted application".into());
                continue;
            }
            let dynamic_due = base.kind() == LightingKind::Pump && self.dynamic.requested.is_some();
            let zone = self.zone(base.kind());
            if due
                || dynamic_due
                || zone.applied.as_ref() != Some(&layer)
                || zone.last_error.is_some()
            {
                if let Err(e) = self.apply(hid, base, layer.clone()).await {
                    warn!("thermal/lighting refresh failed: {e}");
                }
            }
        }
        if self.dynamic.last_error.is_none() {
            self.dynamic.last_error = dynamic_error;
        }
        if due {
            self.last_refresh = Some(tokio::time::Instant::now());
        }
    }
}

enum Output {
    Fan(LightingState),
    Pump(PumpLightingState),
    Direct {
        kind: LightingKind,
        packets: Vec<Vec<u8>>,
        colors: Vec<lconnect_ipc::Rgb>,
    },
}
impl Output {
    fn base_config(&self) -> Option<lconnect_ipc::LightingConfig> {
        let (mode, color, additional, brightness, speed, direction, scope, disabled) = match self {
            Self::Fan(s) => (
                &s.mode,
                &s.color,
                &s.additional_colors,
                s.brightness,
                s.speed,
                &s.direction,
                None,
                s.disabled,
            ),
            Self::Pump(s) => (
                &s.mode,
                &s.color,
                &s.additional_colors,
                s.brightness,
                s.speed,
                &s.direction,
                Some(s.scope.clone()),
                s.disabled,
            ),
            Self::Direct { .. } => return None,
        };
        let rgb = |value: &str| {
            let c = parse_hex_color(value);
            lconnect_ipc::Rgb {
                r: c.r,
                g: c.g,
                b: c.b,
            }
        };
        let mut config = lconnect_ipc::LightingConfig::new(mode.clone(), rgb(color));
        config.additional_colors = additional.iter().map(|c| rgb(c)).collect();
        config.brightness_percent = brightness;
        config.speed_percent = speed;
        config.direction = direction.clone();
        config.scope = scope;
        config.disabled = disabled;
        if let Self::Fan(s) = self {
            config.fan_topology = Some(s.fan_topology.clone().unwrap_or_default());
        }
        Some(config)
    }
    fn direct_led_count(&self) -> Option<usize> {
        match self {
            Self::Direct { colors, .. } => Some(colors.len()),
            _ => None,
        }
    }
    fn kind(&self) -> LightingKind {
        match self {
            Self::Fan(_) => LightingKind::Fan,
            Self::Pump(_) => LightingKind::Pump,
            Self::Direct { kind, .. } => *kind,
        }
    }
    // 2.1.28 Galahad2VisionController.cs:1680-1745; preserve base scope
    // and fan LED count through the existing verified mode builders.
    fn alert_packet(&self, color: lconnect_ipc::Rgb) -> Result<Vec<u8>> {
        use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
            FanLightingMode, LightingDirection, PumpLightingMode,
        };
        let c = lconnect_protocol::galahad2_lcdv2::Rgb {
            r: color.r,
            g: color.g,
            b: color.b,
        };
        match self {
            Self::Fan(s) => {
                let mut setting = fan_setting_from_state(s);
                setting.mode = FanLightingMode::StaticColor;
                setting.sync_to_pump = false;
                setting.brightness = 4;
                setting.speed = 2;
                setting.disabled = false;
                setting.direction = LightingDirection::RightOrClockwise;
                setting.color1 = c;
                setting.color2 = c;
                setting.color3 = c;
                setting.color4 = c;
                Ok(build_set_fan_lighting_request(setting).to_vec())
            }
            Self::Pump(s) => {
                let mut setting = pump_setting_from_state(s);
                setting.mode = PumpLightingMode::StaticColor;
                setting.brightness = 4;
                setting.speed = 2;
                setting.disabled = false;
                setting.direction = LightingDirection::RightOrClockwise;
                setting.color1 = c;
                Ok(build_set_pump_lighting_request(setting).to_vec())
            }
            Self::Direct { kind, colors, .. } => {
                let target = if *kind == LightingKind::Fan {
                    LedTarget::Fan
                } else {
                    LedTarget::Screen
                };
                let warning = vec![color; colors.len()];
                let Output::Direct { packets, .. } = direct_output(target, warning, None)? else {
                    unreachable!()
                };
                packets
                    .into_iter()
                    .next()
                    .ok_or_else(|| IpcError::InvalidArgument("empty direct alert packet".into()))
            }
        }
    }
    /// 2.1.28 Galahad2VisionController.cs:426-449,648-674. Enter the
    /// application source before cmd20, only on transition or unknown state.
    async fn prepare_direct(&self, hid: &mpsc::Sender<HidCommand>, required: bool) -> Result<()> {
        if !required {
            return Ok(());
        }
        use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
            FanLightingMode, PumpLightingMode,
        };
        match self {
            Self::Direct {
                kind: LightingKind::Fan,
                ..
            } => {
                write(
                    hid,
                    build_set_fan_lighting_request(FanLightingSetting {
                        mode: FanLightingMode::StaticColor,
                        source: ArgbSignalSource::Application,
                        brightness: 4,
                        ..Default::default()
                    })
                    .to_vec(),
                )
                .await
            }
            Self::Direct { .. } => {
                write(
                    hid,
                    build_set_pump_lighting_request(PumpLightingSetting {
                        mode: PumpLightingMode::StaticColor,
                        source: ArgbSignalSource::Application,
                        brightness: 4,
                        ..Default::default()
                    })
                    .to_vec(),
                )
                .await
            }
            _ => Ok(()),
        }
    }
    fn settings_update(&self) -> Option<SettingsUpdate> {
        match self {
            Self::Fan(s) => Some(SettingsUpdate::LightingFan(s.clone())),
            Self::Pump(s) => Some(SettingsUpdate::LightingPump(s.clone())),
            Self::Direct { .. } => None,
        }
    }
    async fn write(&self, hid: &mpsc::Sender<HidCommand>) -> Result<()> {
        match self {
            Self::Fan(s) => {
                write(
                    hid,
                    build_set_fan_lighting_request(fan_setting_from_state(s)).to_vec(),
                )
                .await
            }
            Self::Pump(s) => {
                write(
                    hid,
                    build_set_pump_lighting_request(pump_setting_from_state(s)).to_vec(),
                )
                .await
            }
            Self::Direct { packets, .. } => {
                for packet in packets {
                    write(hid, packet.clone()).await?;
                }
                Ok(())
            }
        }
    }
}
fn direct_output(
    target: LedTarget,
    mut colors: Vec<lconnect_ipc::Rgb>,
    previous: Option<&Output>,
) -> Result<Output> {
    // Short arrays replace a prefix. Retain the known tail so neither an
    // alert nor its restoration loses LEDs addressed by an earlier update.
    if colors.is_empty() {
        return Err(IpcError::InvalidArgument(
            "direct colors cannot be empty".into(),
        ));
    }
    if let Some(Output::Direct { colors: old, .. }) = previous {
        if colors.len() < old.len() {
            colors.extend_from_slice(&old[colors.len()..]);
        }
    }
    let retained = colors.clone();
    let colors: Vec<_> = colors
        .into_iter()
        .map(|c| lconnect_protocol::galahad2_lcdv2::Rgb {
            r: c.r,
            g: c.g,
            b: c.b,
        })
        .collect();
    let (kind, packets) = match target {
        LedTarget::Fan => (LightingKind::Fan, build_set_fan_led_colors_request(&colors)),
        LedTarget::Screen => (
            LightingKind::Pump,
            build_set_screen_led_colors_request(&colors),
        ),
        _ => return Err(IpcError::InvalidArgument("unsupported LED target".into())),
    };
    let packets = packets.map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
    Ok(Output::Direct {
        kind,
        colors: retained,
        packets: packets.into_iter().map(|p| p.to_vec()).collect(),
    })
}
async fn write(hid: &mpsc::Sender<HidCommand>, bytes: Vec<u8>) -> Result<()> {
    let (reply, rx) = oneshot::channel();
    hid.send(HidCommand::Write { bytes, reply })
        .await
        .map_err(|_| IpcError::DeviceBusy("HID actor closed".into()))?;
    rx.await
        .map_err(|_| IpcError::DeviceBusy("HID reply dropped".into()))?
        .map_err(|e| IpcError::DeviceBusy(e.to_string()))
}

fn fan_setting_from_state(s: &LightingState) -> FanLightingSetting {
    let mut setting = FanLightingSetting {
        mode: parse_fan_mode(&s.mode),
        brightness: percent_to_wire(s.brightness),
        speed: percent_to_wire(s.speed),
        direction: parse_direction(&s.direction),
        disabled: s.disabled,
        ..FanLightingSetting::default()
    };
    setting.color1 = parse_hex_color(&s.color);
    for (slot, color) in [
        &mut setting.color2,
        &mut setting.color3,
        &mut setting.color4,
    ]
    .into_iter()
    .zip(&s.additional_colors)
    {
        *slot = parse_hex_color(color);
    }
    let topology = s.fan_topology.clone().unwrap_or_default();
    setting.sync_to_pump = topology.sync_to_pump;
    setting.number_of_led = topology.effective_led_count();
    setting.source = ArgbSignalSource::Mcu;
    setting
}

fn pump_setting_from_state(s: &PumpLightingState) -> PumpLightingSetting {
    let mut setting = PumpLightingSetting {
        scope: parse_pump_scope(&s.scope),
        mode: parse_pump_mode(&s.mode),
        brightness: percent_to_wire(s.brightness),
        speed: percent_to_wire(s.speed),
        direction: parse_direction(&s.direction),
        disabled: s.disabled,
        ..PumpLightingSetting::default()
    };
    setting.color1 = parse_hex_color(&s.color);
    for (slot, color) in [
        &mut setting.color2,
        &mut setting.color3,
        &mut setting.color4,
    ]
    .into_iter()
    .zip(&s.additional_colors)
    {
        *slot = parse_hex_color(color);
    }
    setting.source = ArgbSignalSource::Mcu;
    setting
}

fn remember_mode(
    profiles: &mut std::collections::BTreeMap<(bool, String), lconnect_ipc::LightingConfig>,
    output: &Output,
) {
    if let Some(config) = output.base_config() {
        let mode = lconnect_ipc::canonical_lighting_mode(&config.mode);
        if lconnect_ipc::lighting_color_count(output.kind(), mode).is_some() {
            profiles.insert((output.kind() == LightingKind::Fan, mode.into()), config);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lconnect_protocol::galahad2_lcdv2::lighting_enums::{FanLightingMode, LightingDirection};
    use lconnect_protocol::galahad2_lcdv2::Rgb;

    #[test]
    fn parse_hex_color_with_hash() {
        assert_eq!(
            parse_hex_color("#c4a8ff"),
            Rgb {
                r: 0xc4,
                g: 0xa8,
                b: 0xff,
            }
        );
    }

    #[test]
    fn parse_hex_color_without_hash() {
        assert_eq!(
            parse_hex_color("ff0000"),
            Rgb {
                r: 0xff,
                g: 0x00,
                b: 0x00,
            }
        );
    }

    #[test]
    fn parse_hex_color_invalid_falls_back_to_black() {
        assert_eq!(parse_hex_color("nope"), Rgb::BLACK);
        assert_eq!(parse_hex_color(""), Rgb::BLACK);
        assert_eq!(parse_hex_color("#zzzzzz"), Rgb::BLACK);
    }

    #[test]
    fn parse_fan_mode_known_and_unknown() {
        assert_eq!(parse_fan_mode("static"), FanLightingMode::StaticColor);
        assert_eq!(parse_fan_mode("Rainbow"), FanLightingMode::Rainbow);
        assert_eq!(parse_fan_mode("nope"), FanLightingMode::Rainbow);
    }

    #[test]
    fn parse_direction_forward_and_reverse() {
        assert_eq!(
            parse_direction("forward"),
            LightingDirection::RightOrClockwise
        );
        assert_eq!(
            parse_direction("reverse"),
            LightingDirection::LeftOrCounterClockwise
        );
    }
}
