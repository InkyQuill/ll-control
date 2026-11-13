//! Integration test for the `lighting_keeper` actor.
//!
//! Wires a real `hid_io::spawn` over a `MockBackend`, then asserts that
//! the keeper emits cmd-133 / cmd-131 writes at the configured cadence
//! and that state replacement / clearing behave as documented.

use llcontrol_daemon::actor::{hid_io, lighting_keeper, LightingStateMsg};
use llcontrol_daemon::persistence::{LightingState, PumpLightingState};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use llcontrol_ipc::LightingKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};

/// Galahad II LCDV2 ACommandPacket header offsets.
const REPORT_ID_OFFSET: usize = 0;
const COMMAND_OFFSET: usize = 1;
const REPORT_ID: u8 = 0x01;
const CMD_SET_PUMP_LIGHTING: u8 = 131;
const CMD_SET_FAN_LIGHTING: u8 = 133;

fn lcdv2_info() -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        None,
        PathBuf::from("/mock/hidraw0"),
    )
}

fn fan_state() -> LightingState {
    LightingState {
        mode: "static".into(),
        brightness: 4,
        speed: 2,
        direction: "forward".into(),
        color: "#c4a8ff".into(),
        additional_colors: Vec::new(),
        fan_topology: None,
        disabled: false,
    }
}

fn pump_state() -> PumpLightingState {
    PumpLightingState {
        scope: "inner".into(),
        mode: "rainbow".into(),
        brightness: 3,
        speed: 1,
        direction: "forward".into(),
        color: "#00ff00".into(),
        additional_colors: Vec::new(),
        disabled: false,
    }
}

fn count_writes_for_cmd(history: &[Vec<u8>], cmd: u8) -> usize {
    history
        .iter()
        .filter(|w| w.len() >= 2 && w[REPORT_ID_OFFSET] == REPORT_ID && w[COMMAND_OFFSET] == cmd)
        .count()
}

#[tokio::test]
async fn keeper_reissues_fan_then_pump_and_stops_after_clear() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let backend: Arc<dyn HidBackend> = Arc::new(mock.clone());

    // hid_io actor (P1D-5).
    let (_sd_tx, sd_rx) = watch::channel(false);
    let hid_io_tx = hid_io::spawn(
        backend,
        lcdv2_info(),
        std::sync::Arc::new(llcontrol_daemon::stats::Stats::new()),
        sd_rx.clone(),
    )
    .expect("spawn hid_io");

    // lighting_keeper actor with a tight 50ms re-issue cadence so the
    // test wraps in well under a second.
    let (state_tx, state_rx) = mpsc::channel::<LightingStateMsg>(8);
    let (settings, _settings_rx) = mpsc::channel(16);
    let _keeper = lighting_keeper::spawn(
        hid_io_tx,
        50,
        Some(fan_state()),
        None,
        state_rx,
        settings,
        sd_rx.clone(),
    );

    // Wait long enough for the periodic interval to fire several times.
    // tokio::time::interval's first tick fires immediately, but with
    // MissedTickBehavior::Delay still ticks at the configured cadence.
    tokio::time::sleep(Duration::from_millis(180)).await;

    // Fan re-issues should already be visible.
    let history_after_fan = mock.write_history(&lcdv2_info());
    let fan_writes = count_writes_for_cmd(&history_after_fan, CMD_SET_FAN_LIGHTING);
    assert!(
        fan_writes >= 1,
        "expected >=1 SetFanLighting (cmd 133) write, history={:?}",
        history_after_fan
            .iter()
            .map(|w| (w.first().copied(), w.get(1).copied()))
            .collect::<Vec<_>>()
    );
    let pump_writes_before = count_writes_for_cmd(&history_after_fan, CMD_SET_PUMP_LIGHTING);
    assert_eq!(
        pump_writes_before, 0,
        "no pump writes expected before SetPump message, got history={:?}",
        history_after_fan
    );

    // Replace pump state -> next tick should also emit a cmd-131 write.
    let (reply, done) = oneshot::channel();
    state_tx
        .send(LightingStateMsg::SetPump {
            state: pump_state(),
            reply,
        })
        .await
        .expect("send SetPump");
    done.await.unwrap().unwrap();

    tokio::time::sleep(Duration::from_millis(180)).await;

    let history_after_pump = mock.write_history(&lcdv2_info());
    let pump_writes = count_writes_for_cmd(&history_after_pump, CMD_SET_PUMP_LIGHTING);
    assert!(
        pump_writes >= 1,
        "expected >=1 SetPumpLighting (cmd 131) write after SetPump, history={:?}",
        history_after_pump
    );

    // Clear fan -> after a settling period, fan-write count must stop growing.
    let (reply, done) = oneshot::channel();
    state_tx
        .send(LightingStateMsg::Clear {
            kind: LightingKind::Fan,
            reply,
        })
        .await
        .expect("send Clear(Fan)");
    done.await.unwrap().unwrap();

    // Give the keeper one tick to absorb the message before snapshotting.
    tokio::time::sleep(Duration::from_millis(80)).await;
    let baseline = count_writes_for_cmd(&mock.write_history(&lcdv2_info()), CMD_SET_FAN_LIGHTING);

    // Now wait several ticks; fan count must be unchanged.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = count_writes_for_cmd(&mock.write_history(&lcdv2_info()), CMD_SET_FAN_LIGHTING);
    assert_eq!(
        after, baseline,
        "fan writes increased after Clear(Fan): baseline={}, after={}",
        baseline, after
    );

    // Pump must still be re-issuing.
    let pump_after =
        count_writes_for_cmd(&mock.write_history(&lcdv2_info()), CMD_SET_PUMP_LIGHTING);
    assert!(
        pump_after > pump_writes,
        "pump writes did not advance after Clear(Fan): before={}, after={}",
        pump_writes,
        pump_after
    );
}
