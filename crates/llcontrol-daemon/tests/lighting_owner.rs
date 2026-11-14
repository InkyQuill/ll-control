//! Ordering and failure tests for the sole RGB producer; no physical HID.
use llcontrol_daemon::{
    actor::{command_dispatch::SettingsUpdate, lighting_keeper, HidCommand, LightingStateMsg},
    persistence::LightingState,
};
use llcontrol_ipc::{LedTarget, LightingKind, Rgb};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};

fn state(color: &str) -> LightingState {
    LightingState {
        mode: "static".into(),
        brightness: 100,
        speed: 50,
        direction: "forward".into(),
        color: color.into(),
        additional_colors: Vec::new(),
        fan_topology: None,
        disabled: false,
    }
}
struct Harness {
    tx: mpsc::Sender<LightingStateMsg>,
    writes: mpsc::Receiver<HidCommand>,
    settings: mpsc::Receiver<SettingsUpdate>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
impl Harness {
    fn new(initial: Option<LightingState>) -> Self {
        let (hid, writes) = mpsc::channel(16);
        let (tx, rx) = mpsc::channel(16);
        let (settings_tx, settings) = mpsc::channel(16);
        let (stop, shutdown) = watch::channel(false);
        let task = lighting_keeper::spawn(hid, 100, initial, None, rx, settings_tx, shutdown);
        Self {
            tx,
            writes,
            settings,
            stop,
            task,
        }
    }
    async fn write(&mut self) -> (Vec<u8>, oneshot::Sender<llcontrol_hid::Result<()>>) {
        let Some(HidCommand::Write { bytes, reply }) = self.writes.recv().await else {
            panic!("expected RGB write")
        };
        assert!(matches!(bytes[1], 20 | 131 | 133), "must not touch PWM/LCD");
        (bytes, reply)
    }
    async fn set(&self, color: &str) -> oneshot::Receiver<llcontrol_ipc::Result<()>> {
        let (reply, done) = oneshot::channel();
        self.tx
            .send(LightingStateMsg::SetFan {
                state: state(color),
                reply,
            })
            .await
            .unwrap();
        done
    }
    async fn base(&self) -> Option<llcontrol_ipc::LightingConfig> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LightingStateMsg::Base {
                kind: LightingKind::Fan,
                reply,
            })
            .await
            .unwrap();
        rx.await.unwrap().current
    }
    async fn stop(self) {
        self.stop.send(true).unwrap();
        self.task.await.unwrap();
    }
}
#[tokio::test(start_paused = true)]
async fn delayed_refresh_cannot_overtake_manual_and_ack_includes_settings() {
    let mut h = Harness::new(Some(state("#ff0000")));
    let (old, refresh) = h.write().await;
    let mut done = h.set("#00ff00").await;
    assert!(matches!(
        done.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    tokio::time::advance(Duration::from_secs(10)).await;
    refresh.send(Ok(())).unwrap();
    let (new, write) = h.write().await;
    assert_ne!(new, old, "queued manual must precede overdue refresh");
    assert!(h.settings.try_recv().is_err());
    write.send(Ok(())).unwrap();
    done.await.unwrap().unwrap();
    let SettingsUpdate::LightingFan(saved) = h.settings.try_recv().unwrap() else {
        panic!("persist manual")
    };
    assert_eq!(saved.color, "#00ff00");
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert!(h.writes.try_recv().is_err(), "manual mode was restarted");
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn failed_manual_keeps_previous_refresh_and_does_not_persist_failed_intent() {
    let mut h = Harness::new(Some(state("#ff0000")));
    let (old, reply) = h.write().await;
    reply.send(Ok(())).unwrap();
    let done = h.set("#00ff00").await;
    let (_, reply) = h.write().await;
    reply
        .send(Err(llcontrol_hid::Error::BlockingTaskFailed(
            "injected".into(),
        )))
        .unwrap();
    assert!(done.await.unwrap().is_err());
    assert!(h.settings.try_recv().is_err());
    let (next, reply) = h.write().await;
    assert_eq!(next, old);
    reply.send(Ok(())).unwrap();
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn direct_colors_replace_refresh_until_next_mode_and_clear_is_a_barrier() {
    let mut h = Harness::new(Some(state("#ff0000")));
    let (_, reply) = h.write().await;
    reply.send(Ok(())).unwrap();
    let (reply, done) = oneshot::channel();
    h.tx.send(LightingStateMsg::Direct {
        target: LedTarget::Fan,
        colors: vec![Rgb { r: 1, g: 2, b: 3 }],
        reply,
    })
    .await
    .unwrap();
    let (prepare, reply) = h.write().await;
    assert_eq!((prepare[1], prepare[23]), (133, 2));
    reply.send(Ok(())).unwrap();
    let (direct, reply) = h.write().await;
    assert_eq!(direct[1], 20);
    reply.send(Ok(())).unwrap();
    done.await.unwrap().unwrap();
    let (repeated, reply) = h.write().await;
    assert_eq!(direct, repeated);
    reply.send(Ok(())).unwrap();
    assert!(
        h.base().await.is_none(),
        "direct data is not a manual mode base"
    );
    assert!(
        h.settings.try_recv().is_err(),
        "direct data stays transient"
    );
    let done = h.set("#0000ff").await;
    let (mode, reply) = h.write().await;
    assert_eq!(mode[1], 133);
    reply.send(Ok(())).unwrap();
    done.await.unwrap().unwrap();
    let (reply, done) = oneshot::channel();
    h.tx.send(LightingStateMsg::Clear {
        kind: LightingKind::Fan,
        reply,
    })
    .await
    .unwrap();
    done.await.unwrap().unwrap();
    assert!(h.base().await.is_none(), "Clear removes manual base");
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(h.writes.try_recv().is_err());
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn invalid_direct_data_and_closed_settings_are_reported() {
    let mut h = Harness::new(None);
    let (reply, done) = oneshot::channel();
    h.tx.send(LightingStateMsg::Direct {
        target: LedTarget::Screen,
        colors: vec![],
        reply,
    })
    .await
    .unwrap();
    assert!(done.await.unwrap().is_err());
    assert!(h.writes.try_recv().is_err());
    h.settings.close();
    let done = h.set("#123456").await;
    let (_, reply) = h.write().await;
    reply.send(Ok(())).unwrap();
    assert!(done
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("settings actor closed"));
    assert_eq!(
        h.base().await.unwrap().color,
        Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56
        }
    );
    assert!(h.writes.try_recv().is_err());
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn shutdown_finishes_inflight_transaction_and_rejects_queued_commands() {
    let mut h = Harness::new(None);
    let first = h.set("#123456").await;
    let (_, reply) = h.write().await;
    let second = h.set("#abcdef").await;
    h.stop.send(true).unwrap();
    reply.send(Ok(())).unwrap();
    first.await.unwrap().unwrap();
    h.task.await.unwrap();
    assert!(second.await.is_err());
    assert!(h.writes.try_recv().is_err());
    assert!(matches!(
        h.settings.try_recv(),
        Ok(SettingsUpdate::LightingFan(_))
    ));
}

#[tokio::test(start_paused = true)]
async fn direct_data_failure_after_preparation_does_not_commit_and_retry_prepares_again() {
    let mut h = Harness::new(None);
    for fail in [true, false] {
        let (reply, done) = oneshot::channel();
        h.tx.send(LightingStateMsg::Direct {
            target: LedTarget::Screen,
            colors: vec![Rgb { r: 1, g: 2, b: 3 }],
            reply,
        })
        .await
        .unwrap();
        let (prepare, reply) = h.write().await;
        assert_eq!(
            (prepare[1], prepare[24]),
            (131, 2),
            "first attempt and unknown-state retry prepare Application source"
        );
        reply.send(Ok(())).unwrap();
        let (data, reply) = h.write().await;
        assert_eq!(data[1], 20);
        reply
            .send(if fail {
                Err(llcontrol_hid::Error::BlockingTaskFailed(
                    "data failed after preparation".into(),
                ))
            } else {
                Ok(())
            })
            .unwrap();
        assert_eq!(done.await.unwrap().is_err(), fail);
        assert!(
            h.settings.try_recv().is_err(),
            "transient/failed data must not persist"
        );
        if fail {
            tokio::time::advance(Duration::from_secs(1)).await;
            tokio::task::yield_now().await;
            assert!(
                h.writes.try_recv().is_err(),
                "failed first output must not become a refresh base"
            );
        }
    }
    let (repeated, reply) = h.write().await;
    assert_eq!(
        repeated[1], 20,
        "successful direct refresh needs no repeated preparation"
    );
    reply.send(Ok(())).unwrap();
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn manual_base_read_is_full_read_only_and_failed_replacement_keeps_it() {
    let mut h = Harness::new(None);
    assert!(h.base().await.is_none());
    let mut manual = state("#010203");
    manual.mode = "meteor".into();
    manual.brightness = 77;
    manual.speed = 25;
    manual.direction = "reverse".into();
    manual.disabled = true;
    manual.additional_colors = vec!["#040506".into(), "#070809".into(), "#0a0b0c".into()];
    let (reply, done) = oneshot::channel();
    h.tx.send(LightingStateMsg::SetFan {
        state: manual,
        reply,
    })
    .await
    .unwrap();
    let (_, reply) = h.write().await;
    reply.send(Ok(())).unwrap();
    done.await.unwrap().unwrap();
    let base = serde_json::to_value(h.base().await.unwrap()).unwrap();
    assert_eq!(
        base,
        serde_json::json!({"fan_topology":{"sync_to_pump":false,"size":"360","manual_led_count":24},"mode":"meteor","color":{"r":1,"g":2,"b":3},"additional_colors":[{"r":4,"g":5,"b":6},{"r":7,"g":8,"b":9},{"r":10,"g":11,"b":12}],"brightness_percent":77,"speed_percent":25,"direction":"reverse","scope":null,"disabled":true})
    );
    assert!(h.writes.try_recv().is_err(), "base read must not write HID");
    let done = h.set("#ffffff").await;
    let (_, reply) = h.write().await;
    reply
        .send(Err(llcontrol_hid::Error::BlockingTaskFailed(
            "injected".into(),
        )))
        .unwrap();
    assert!(done.await.unwrap().is_err());
    assert_eq!(serde_json::to_value(h.base().await.unwrap()).unwrap(), base);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn failed_topology_write_keeps_previous_global_topology_and_mode_base() {
    let mut h = Harness::new(Some(state("#010203")));
    let (_, reply) = h.write().await;
    reply.send(Ok(())).unwrap();
    let mut candidate = state("#ffffff");
    candidate.fan_topology = Some(llcontrol_ipc::FanLightingTopology {
        sync_to_pump: true,
        size: llcontrol_ipc::FanLedSize::Mm280,
        manual_led_count: 31,
    });
    let (reply, done) = oneshot::channel();
    h.tx.send(LightingStateMsg::SetFan {
        state: candidate,
        reply,
    })
    .await
    .unwrap();
    let (packet, reply) = h.write().await;
    assert_eq!((packet[24], packet[25]), (1, 16));
    reply
        .send(Err(llcontrol_hid::Error::BlockingTaskFailed(
            "injected".into(),
        )))
        .unwrap();
    assert!(done.await.unwrap().is_err());
    let base = h.base().await.unwrap();
    assert_eq!(base.color, Rgb { r: 1, g: 2, b: 3 });
    assert_eq!(base.fan_topology, Some(Default::default()));
    assert!(h.settings.try_recv().is_err());
    h.stop().await;
}
