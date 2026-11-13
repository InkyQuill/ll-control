use super::*;
use crate::actor::{hid_io, FrameReadPolicy, HidCommand};
use crate::stats::Stats;
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use tokio::sync::oneshot;

#[derive(Clone, Copy, Debug)]
enum Fault {
    None,
    WrongVersion,
    Short,
    Stale,
    Reject,
    Timeout,
    PartialWrite,
    ReadFailure,
    RejectSecond,
    ActivationLost,
    Gate,
    PumpHoldFailure,
    FanHoldFailure,
}
#[derive(Clone)]
struct Script {
    mock: MockBackend,
    fault: Fault,
    info: DeviceInfo,
    gated: Arc<AtomicBool>,
    entered: Arc<AtomicBool>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    cancel_on_pump: Option<watch::Sender<bool>>,
    journal_root: Arc<tempfile::TempDir>,
    observed_journal: Arc<Mutex<Option<PathBuf>>>,
    tamper_after_query: bool,
    tamper_on_activation: bool,
}
struct Handle {
    inner: Box<dyn OpenHandle>,
    script: Script,
}
impl Script {
    fn new(fault: Fault) -> Self {
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            None,
            PathBuf::from("/mock/update"),
        );
        let mock = MockBackend::new();
        mock.add_device(info.clone());
        Self {
            mock,
            fault,
            info,
            gated: Arc::default(),
            entered: Arc::default(),
            gate: Arc::new((Mutex::new(false), Condvar::new())),
            cancel_on_pump: None,
            journal_root: Arc::new(tempfile::tempdir().unwrap()),
            observed_journal: Arc::default(),
            tamper_after_query: false,
            tamper_on_activation: false,
        }
    }
    fn operation(&self, hold: CoolingHold) -> crate::firmware_operation::ArmedOperation {
        let path = if self
            .journal_root
            .path()
            .join("firmware-operation.json")
            .exists()
        {
            tempfile::tempdir_in(self.journal_root.path())
                .unwrap()
                .keep()
        } else {
            self.journal_root.path().to_path_buf()
        };
        let operation = crate::firmware_operation::Journal::new(path.clone())
            .arm(
                &self.info,
                hold,
                crate::persistence::DeviceSettings::default(),
            )
            .unwrap();
        *self.observed_journal.lock().unwrap() = Some(path);
        operation
    }
    fn options(&self, eligible: bool, hold: CoolingHold) -> TransferOptions {
        let mut target = self.info.clone();
        if !eligible {
            target.pid = 0;
        }
        TransferOptions {
            target,
            cooling_hold: hold,
            operation: self.operation(hold),
            journal: crate::firmware_operation::Journal::new(self.journal_root.path().into()),
        }
    }
    fn open_handle(&self) -> Box<dyn OpenHandle> {
        self.open(&self.info).unwrap()
    }
    fn history(&self) -> Vec<Vec<u8>> {
        self.mock.write_history(&self.info)
    }
    fn release(&self) {
        *self.gate.0.lock().unwrap() = true;
        self.gate.1.notify_all();
    }
}
impl HidBackend for Script {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
        Ok(Box::new(Handle {
            inner: self.mock.open(info)?,
            script: self.clone(),
        }))
    }
}
impl OpenHandle for Handle {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        self.inner.write(bytes)?;
        if bytes.starts_with(&[1, 134])
            || bytes.starts_with(&[1, 138])
            || bytes.starts_with(&[1, 139])
            || bytes.starts_with(&[2, 11])
            || bytes.starts_with(&[2, 10])
        {
            let path = self
                .script
                .observed_journal
                .lock()
                .unwrap()
                .clone()
                .unwrap();
            let record = crate::firmware_operation::Journal::new(path.clone())
                .load()
                .unwrap()
                .unwrap();
            let expected = if bytes.starts_with(&[1, 134]) {
                crate::firmware_operation::Stage::Armed
            } else {
                crate::firmware_operation::Stage::EffectsPossible
            };
            assert_eq!(
                record.stage, expected,
                "durable marker must precede wire call"
            );
            if expected == crate::firmware_operation::Stage::EffectsPossible {
                let settings = crate::persistence::PersistenceStore::new_with_root(path.clone())
                    .read(&record.target.settings_key)
                    .unwrap();
                let guard = settings
                    .firmware_update_guard
                    .as_ref()
                    .expect("settings anchor before every side effect");
                assert_eq!(guard.operation_id, record.operation_id);
                assert_eq!(guard.image_sha256, record.image_sha256);
            }
            if (bytes.starts_with(&[1, 134]) && self.script.tamper_after_query)
                || (bytes.starts_with(&[2, 10]) && self.script.tamper_on_activation)
            {
                std::fs::write(path.join("firmware-operation.json"), b"external edit").unwrap();
            }
        }

        if bytes.starts_with(&[1, 138]) {
            if matches!(self.script.fault, Fault::PumpHoldFailure) {
                return Err(llcontrol_hid::Error::ShortWrite {
                    written: 17,
                    expected: 64,
                });
            }
            if let Some(cancel) = &self.script.cancel_on_pump {
                let _ = cancel.send(true);
            }
            return Ok(());
        }
        if bytes.starts_with(&[1, 139]) && matches!(self.script.fault, Fault::FanHoldFailure) {
            return Err(llcontrol_hid::Error::DeviceNotFound(
                "synthetic fan hold detach".into(),
            ));
        }
        let mut response = if bytes.starts_with(&[1, 134]) {
            let identity = if matches!(self.script.fault, Fault::WrongVersion) {
                "N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.5"
            } else {
                IDENTITY
            };
            let version = ACommandPacket::new_request(134, identity.as_bytes())
                .as_bytes()
                .to_vec();
            let mut date = ACommandPacket::new_request(134, b"synthetic date")
                .as_bytes()
                .to_vec();
            date[4] = 1;
            self.script.mock.queue_response(&self.script.info, version);
            date
        } else if bytes.starts_with(&[2, 11]) {
            let pn = u32::from_be_bytes([0, bytes[6], bytes[7], bytes[8]]);
            let mut echo = bytes[..512].to_vec();
            if pn == 1 && matches!(self.script.fault, Fault::RejectSecond) {
                echo[1] = 2;
                echo[6] = 0x87;
            }
            if pn == 0 {
                match self.script.fault {
                    Fault::Short => {
                        echo.truncate(12);
                    }
                    Fault::Stale => {
                        echo[8] = 1;
                    }
                    Fault::Reject => {
                        echo[1] = 2;
                        echo[6] = 0x84;
                    }
                    Fault::Timeout => return Ok(()),
                    Fault::PartialWrite => {
                        return Err(llcontrol_hid::Error::ShortWrite {
                            written: 17,
                            expected: 1024,
                        })
                    }
                    Fault::ReadFailure => {
                        self.script.gated.store(true, Ordering::Release);
                    }
                    Fault::Gate => {
                        self.script.gated.store(true, Ordering::Release);
                    }
                    _ => (),
                }
            }
            echo
        } else if bytes.starts_with(&[2, 10]) {
            if matches!(self.script.fault, Fault::ActivationLost) {
                return Ok(());
            }
            let mut echo = bytes[..512].to_vec();
            echo[12] = 1;
            echo
        } else {
            return Ok(());
        };
        // A valid late PN1 during upload must not be accepted as its ACK.
        if bytes.starts_with(&[2, 11]) {
            let mut late = ACommandPacket::new_request(134, b"late synthetic date")
                .as_bytes()
                .to_vec();
            late[4] = 1;
            self.script.mock.queue_response(&self.script.info, late);
        }
        self.script
            .mock
            .queue_response(&self.script.info, std::mem::take(&mut response));
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], timeout: i32) -> llcontrol_hid::Result<usize> {
        if self.script.gated.swap(false, Ordering::AcqRel) {
            if matches!(self.script.fault, Fault::ReadFailure) {
                return Err(llcontrol_hid::Error::DeviceNotFound(
                    "synthetic detach".into(),
                ));
            }
            self.script.entered.store(true, Ordering::Release);
            let guard = self.script.gate.0.lock().unwrap();
            let (guard, timed) = self
                .script
                .gate
                .1
                .wait_timeout_while(guard, Duration::from_secs(5), |open| !*open)
                .unwrap();
            assert!(*guard && !timed.timed_out(), "test gate must be released");
        }
        self.inner.read_timeout(bytes, timeout)
    }
}
fn fast() -> Timing {
    Timing {
        settle: Duration::ZERO,
        inter_chunk: Duration::ZERO,
        before_activate: Duration::ZERO,
        reply: Duration::from_millis(30),
    }
}
fn synthetic() -> ValidatedImage {
    ValidatedImage(vec![0x5a; IMAGE_SIZE])
}
fn run_script(script: &Script, image: ValidatedImage, eligible: bool, cancelled: bool) -> Outcome {
    run_held(script, image, eligible, cancelled, CoolingHold::PumpOnly)
}
fn run_held(
    script: &Script,
    image: ValidatedImage,
    eligible: bool,
    cancelled: bool,
    cooling_hold: CoolingHold,
) -> Outcome {
    let (_cancel, cancel) = watch::channel(cancelled);
    let (_stop, stop) = watch::channel(false);
    let (progress, observed) = watch::channel(Progress::default());
    let result = run_timed(
        &*script.open_handle(),
        image,
        script.options(eligible, cooling_hold),
        cancel,
        stop,
        progress,
        fast(),
    );
    assert_eq!(*observed.borrow(), result.progress);
    result
}
#[test]
fn image_recognition_rejects_unknown_bytes_without_io_or_public_unchecked_constructor() {
    for bytes in [
        vec![],
        vec![0; IMAGE_SIZE - 1],
        vec![0; IMAGE_SIZE],
        vec![0; IMAGE_SIZE + 1],
    ] {
        assert!(ValidatedImage::recognize(bytes).is_err());
    }
}
#[test]
fn complete_transaction_owns_order_and_stops_at_awaiting_reconnect() {
    let script = Script::new(Fault::None);
    let result = run_script(&script, synthetic(), true, false);
    assert_eq!(result.progress.phase, Phase::AwaitingReconnect);
    assert_eq!(result.progress.chunk_writes, 619);
    assert_eq!(result.progress.chunk_echoes, 619);
    assert!(result.progress.activation_written);
    assert!(result.diagnostic.is_none());
    assert_eq!(
        script.history().iter().map(|p| p[1]).collect::<Vec<_>>(),
        std::iter::once(134)
            .chain(std::iter::once(138))
            .chain(std::iter::repeat_n(11, 619))
            .chain(std::iter::once(10))
            .collect::<Vec<_>>()
    );
    assert_eq!(result.writes(), 622);
}
#[test]
fn chunk_faults_never_retry_or_activate_and_write_errors_stay_ambiguous() {
    for fault in [
        Fault::Short,
        Fault::Stale,
        Fault::Reject,
        Fault::Timeout,
        Fault::PartialWrite,
        Fault::ReadFailure,
    ] {
        let script = Script::new(fault);
        let result = run_script(&script, synthetic(), true, false);
        assert_eq!(result.progress.phase, Phase::Unknown, "{fault:?}");
        assert_eq!(result.progress.attempted, Some(Step::Chunk(0)));
        assert_eq!(result.progress.chunk_echoes, 0);
        assert_eq!(
            result.progress.chunk_writes,
            u32::from(!matches!(fault, Fault::PartialWrite))
        );
        assert!(result.diagnostic.is_some());
        assert_eq!(
            script.history().iter().map(|p| p[1]).collect::<Vec<_>>(),
            [134, 138, 11]
        );
    }
}
#[test]
fn lost_activation_reply_is_not_success_and_does_not_repeat_activation() {
    let script = Script::new(Fault::ActivationLost);
    let result = run_script(&script, synthetic(), true, false);
    assert_eq!(result.progress.phase, Phase::Unknown);
    assert_eq!(result.progress.attempted, Some(Step::Activate));
    assert!(result.progress.activation_written);
    assert_eq!(script.history().iter().filter(|p| p[1] == 10).count(), 1);
}
#[test]
fn wrong_device_cancelled_busy_input_or_wrong_current_firmware_never_write_flash() {
    for (eligible, cancelled) in [(false, false), (true, true)] {
        let script = Script::new(Fault::None);
        let result = run_script(&script, synthetic(), eligible, cancelled);
        assert_eq!(result.progress.phase, Phase::NotStarted);
        assert!(script.history().is_empty());
    }
    let script = Script::new(Fault::WrongVersion);
    let result = run_script(&script, synthetic(), true, false);
    assert_eq!(result.progress.phase, Phase::NotStarted);
    assert_eq!(script.history().len(), 1);
    let script = Script::new(Fault::None);
    for _ in 0..10 {
        script.mock.queue_response(&script.info, vec![1; 64]);
    }
    let result = run_script(&script, synthetic(), true, false);
    assert_eq!(result.progress.phase, Phase::NotStarted);
    assert!(script.history().is_empty());
}
async fn entered(script: &Script) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !script.entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn cancel_after_last_echo_during_activation_delay_prevents_activation() {
    let script = Script::new(Fault::None);
    let worker = script.clone();
    let (cancel_tx, cancel) = watch::channel(false);
    let (_stop, stop) = watch::channel(false);
    let (progress, mut observed) = watch::channel(Progress::default());
    let job = tokio::task::spawn_blocking(move || {
        run_timed(
            &*worker.open_handle(),
            synthetic(),
            worker.options(true, CoolingHold::PumpOnly),
            cancel,
            stop,
            progress,
            Timing {
                before_activate: Duration::from_secs(1),
                ..fast()
            },
        )
    });
    while observed.borrow().chunk_echoes != 619 {
        observed.changed().await.unwrap();
    }
    cancel_tx.send(true).unwrap();
    let result = job.await.unwrap();
    assert_eq!(result.progress.phase, Phase::Unknown);
    assert!(!result.progress.activation_written);
    assert_eq!(script.history().len(), 621);
}
#[tokio::test]
async fn actor_serializes_update_and_rejects_every_later_command_even_after_cancel() {
    let script = Script::new(Fault::Gate);
    let stats = Arc::new(Stats::new());
    let (stop, shutdown) = watch::channel(false);
    let (cancel_tx, cancel) = watch::channel(false);
    let (progress, terminal) = watch::channel(Progress::default());
    let repeated_progress = progress.clone();
    let (reply, result) = oneshot::channel();
    let (tx, task) = hid_io::spawn_tracked_with_journal(
        Arc::new(script.clone()),
        script.info.clone(),
        stats.clone(),
        shutdown,
        crate::firmware_operation::Journal::new(script.journal_root.path().into()),
    )
    .unwrap();
    tx.send(HidCommand::FirmwareUpdate {
        operation: Box::new(script.operation(CoolingHold::PumpOnly)),
        cooling_hold: CoolingHold::PumpOnly,
        image: synthetic(),
        cancel,
        progress,
        reply,
    })
    .await
    .unwrap();
    entered(&script).await;
    let (write, wr) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![1, 222],
        reply: write,
    })
    .await
    .unwrap();
    let (query, qr) = oneshot::channel();
    tx.send(HidCommand::Firmware { reply: query })
        .await
        .unwrap();
    let (sensor, sr) = oneshot::channel();
    tx.send(HidCommand::PollSensors { reply: sensor })
        .await
        .unwrap();
    let (read, rr) = oneshot::channel();
    tx.send(HidCommand::WriteRead64 {
        bytes: vec![1, 222],
        expected_cmd: 222,
        reply: read,
    })
    .await
    .unwrap();
    let (read, br) = oneshot::channel();
    tx.send(HidCommand::WriteRead512 {
        bytes: vec![2, 222],
        expected_report_id: 2,
        expected_cmd: 222,
        reply: read,
    })
    .await
    .unwrap();
    let (frame, fr) = oneshot::channel();
    tx.send(HidCommand::FrameBatch {
        packets: vec![],
        read_policy: FrameReadPolicy::None,
        reply: frame,
    })
    .await
    .unwrap();
    let (_cancel2, cancel) = watch::channel(false);
    let progress = repeated_progress;
    let (reply, second) = oneshot::channel();
    tx.send(HidCommand::FirmwareUpdate {
        operation: Box::new(script.operation(CoolingHold::PumpOnly)),
        cooling_hold: CoolingHold::PumpOnly,
        image: synthetic(),
        cancel,
        progress,
        reply,
    })
    .await
    .unwrap();
    assert_eq!(script.history().len(), 3);
    cancel_tx.send(true).unwrap();
    script.release();
    assert_eq!(result.await.unwrap().progress.phase, Phase::Unknown);
    assert!(wr.await.unwrap().is_err());
    assert!(qr.await.unwrap().is_err());
    assert!(sr.await.unwrap().is_err());
    assert!(rr.await.unwrap().is_err());
    assert!(br.await.unwrap().is_err());
    assert!(fr.await.unwrap().is_err());
    assert_eq!(second.await.unwrap().progress.phase, Phase::NotStarted);
    assert_eq!(script.history().len(), 3);
    assert_eq!(terminal.borrow().phase, Phase::Unknown);
    assert_eq!(stats.hid_write_count.load(Ordering::Relaxed), 3);
    stop.send(true).unwrap();
    task.await.unwrap();
}
#[tokio::test]
async fn shutdown_joins_the_started_read_before_dropping_queued_writes() {
    let script = Script::new(Fault::Gate);
    let (stop, shutdown) = watch::channel(false);
    let (_cancel, cancel) = watch::channel(false);
    let (progress, _) = watch::channel(Progress::default());
    let (reply, result) = oneshot::channel();
    let (tx, task) = hid_io::spawn_tracked_with_journal(
        Arc::new(script.clone()),
        script.info.clone(),
        Arc::new(Stats::new()),
        shutdown,
        crate::firmware_operation::Journal::new(script.journal_root.path().into()),
    )
    .unwrap();
    tx.send(HidCommand::FirmwareUpdate {
        operation: Box::new(script.operation(CoolingHold::PumpOnly)),
        cooling_hold: CoolingHold::PumpOnly,
        image: synthetic(),
        cancel,
        progress,
        reply,
    })
    .await
    .unwrap();
    entered(&script).await;
    let (reply, mut queued) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![1, 222],
        reply,
    })
    .await
    .unwrap();
    stop.send(true).unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!task.is_finished());
    assert!(matches!(
        queued.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    script.release();
    assert_eq!(result.await.unwrap().progress.phase, Phase::Unknown);
    task.await.unwrap();
    assert!(queued.await.is_err());
    assert_eq!(script.history().len(), 3);
}
#[tokio::test]
async fn dropping_the_requester_does_not_detach_the_update_or_allow_queued_writes() {
    let script = Script::new(Fault::Gate);
    let (stop, shutdown) = watch::channel(false);
    let (_cancel, cancel) = watch::channel(false);
    let (progress, mut observed) = watch::channel(Progress::default());
    let (reply, result) = oneshot::channel();
    let (tx, task) = hid_io::spawn_tracked_with_journal(
        Arc::new(script.clone()),
        script.info.clone(),
        Arc::new(Stats::new()),
        shutdown,
        crate::firmware_operation::Journal::new(script.journal_root.path().into()),
    )
    .unwrap();
    tx.send(HidCommand::FirmwareUpdate {
        operation: Box::new(script.operation(CoolingHold::PumpOnly)),
        cooling_hold: CoolingHold::PumpOnly,
        image: synthetic(),
        cancel,
        progress,
        reply,
    })
    .await
    .unwrap();
    entered(&script).await;
    drop(result);
    let (reply, queued) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![1, 222],
        reply,
    })
    .await
    .unwrap();
    script.release();
    tokio::time::timeout(Duration::from_secs(20), async {
        while observed.borrow().phase != Phase::AwaitingReconnect {
            observed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(queued.await.unwrap().is_err());
    assert_eq!(script.history().len(), 622);
    stop.send(true).unwrap();
    task.await.unwrap();
}
#[test]
#[ignore = "requires separately acquired vendor image; never opens hardware"]
fn pinned_vendor_image_is_recognized_and_transfers_all_619_chunks_on_mock_only() {
    let path = std::env::var("LLCONTROL_TEST_FIRMWARE_IMAGE").unwrap();
    let image = ValidatedImage::recognize(std::fs::read(path).unwrap()).unwrap();
    let script = Script::new(Fault::None);
    let result = run_script(&script, image, true, false);
    assert_eq!(result.progress.phase, Phase::AwaitingReconnect);
    assert_eq!(result.progress.chunk_echoes, 619);
    assert_eq!(result.writes(), 622);
}

#[tokio::test]
async fn pending_sensor_finishes_before_update_and_later_fifo_writer_is_fenced() {
    let script = Script::new(Fault::None);
    let (stop, shutdown) = watch::channel(false);
    let (tx, task) = hid_io::spawn_tracked_with_journal(
        Arc::new(script.clone()),
        script.info.clone(),
        Arc::new(Stats::new()),
        shutdown,
        crate::firmware_operation::Journal::new(script.journal_root.path().into()),
    )
    .unwrap();
    let (reply, sensor) = oneshot::channel();
    tx.send(HidCommand::PollSensors { reply }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while script.history().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (_cancel, cancel) = watch::channel(true);
    let (progress, _) = watch::channel(Progress::default());
    let (reply, update) = oneshot::channel();
    tx.send(HidCommand::FirmwareUpdate {
        operation: Box::new(script.operation(CoolingHold::PumpOnly)),
        cooling_hold: CoolingHold::PumpOnly,
        image: synthetic(),
        cancel,
        progress,
        reply,
    })
    .await
    .unwrap();
    let (reply, writer) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![1, 222],
        reply,
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        script.history().iter().map(|p| p[1]).collect::<Vec<_>>(),
        [129]
    );
    script.mock.queue_response(
        &script.info,
        ACommandPacket::new_request(129, &[0, 1, 0, 2, 1, 35, 2])
            .as_bytes()
            .to_vec(),
    );
    assert_eq!(sensor.await.unwrap().unwrap()[1], 129);
    assert_eq!(update.await.unwrap().progress.phase, Phase::NotStarted);
    assert!(writer.await.unwrap().is_err());
    assert_eq!(script.history().len(), 1);
    stop.send(true).unwrap();
    task.await.unwrap();
}

#[test]
fn rejection_after_one_ack_retains_exact_partial_progress_and_never_activates() {
    let script = Script::new(Fault::RejectSecond);
    let result = run_script(&script, synthetic(), true, false);
    assert_eq!(result.progress.phase, Phase::Unknown);
    assert_eq!(result.progress.chunk_writes, 2);
    assert_eq!(result.progress.chunk_echoes, 1);
    assert_eq!(result.progress.attempted, Some(Step::Chunk(1)));
    assert!(!result.progress.activation_written);
    assert_eq!(
        script.history().iter().map(|p| p[1]).collect::<Vec<_>>(),
        [134, 138, 11, 11]
    );
}

#[test]
fn explicit_controller_fan_hold_is_written_after_pump_and_before_any_firmware_chunk() {
    let script = Script::new(Fault::None);
    let result = run_held(
        &script,
        synthetic(),
        true,
        false,
        CoolingHold::PumpAndControllerFan,
    );
    assert_eq!(result.progress.phase, Phase::AwaitingReconnect);
    assert_eq!(
        result.progress.cooling_hold,
        Some(CoolingHold::PumpAndControllerFan)
    );
    assert!(result.progress.pump_hold_written && result.progress.fan_hold_written);
    let writes = script.history();
    assert_eq!(
        writes.iter().map(|p| p[1]).collect::<Vec<_>>(),
        [134, 138, 139]
            .into_iter()
            .chain(std::iter::repeat_n(11, 619))
            .chain(std::iter::once(10))
            .collect::<Vec<_>>()
    );
    assert_eq!(&writes[1][6..8], &[0, 100]);
    assert_eq!(&writes[2][6..8], &[0, 100]);
    assert_eq!(result.writes(), 623);
}
#[test]
fn failed_or_partial_hold_writes_keep_cooling_unknown_and_never_start_flash() {
    for fault in [Fault::PumpHoldFailure, Fault::FanHoldFailure] {
        let script = Script::new(fault);
        let result = run_held(
            &script,
            synthetic(),
            true,
            false,
            CoolingHold::PumpAndControllerFan,
        );
        assert_eq!(result.progress.phase, Phase::Unknown);
        assert_eq!(result.progress.chunk_writes, 0);
        assert_eq!(result.progress.chunk_echoes, 0);
        assert!(!result.progress.activation_written && !result.progress.fan_hold_written);
        if matches!(fault, Fault::PumpHoldFailure) {
            assert_eq!(result.progress.attempted, Some(Step::PumpHold));
            assert!(!result.progress.pump_hold_written);
            assert_eq!(result.writes(), 1);
            assert_eq!(
                script.history().iter().map(|p| p[1]).collect::<Vec<_>>(),
                [134, 138]
            );
        } else {
            assert_eq!(result.progress.attempted, Some(Step::FanHold));
            assert!(result.progress.pump_hold_written);
            assert_eq!(result.writes(), 2);
            assert_eq!(
                script.history().iter().map(|p| p[1]).collect::<Vec<_>>(),
                [134, 138, 139]
            );
        }
    }
}
#[test]
fn cancellation_after_pump_hold_does_not_start_fan_or_flash_or_restore_old_pwm() {
    for cooling_hold in [CoolingHold::PumpOnly, CoolingHold::PumpAndControllerFan] {
        let mut script = Script::new(Fault::None);
        let (cancel_tx, cancel) = watch::channel(false);
        script.cancel_on_pump = Some(cancel_tx);
        let (_stop, stop) = watch::channel(false);
        let (progress, _) = watch::channel(Progress::default());
        let result = run_timed(
            &*script.open_handle(),
            synthetic(),
            script.options(true, cooling_hold),
            cancel,
            stop,
            progress,
            fast(),
        );
        assert_eq!(result.progress.phase, Phase::Unknown);
        assert_eq!(result.progress.attempted, Some(Step::PumpHold));
        assert!(result.progress.pump_hold_written);
        assert!(!result.progress.fan_hold_written);
        assert_eq!(result.progress.chunk_writes, 0);
        assert_eq!(
            script.history().iter().map(|p| p[1]).collect::<Vec<_>>(),
            [134, 138]
        );
    }
}

#[test]
fn target_hold_or_file_mismatch_prevents_even_readonly_preflight_and_preserves_quarantine() {
    for mismatch in ["serial", "path", "hold", "scope", "missing", "changed"] {
        let script = Script::new(Fault::None);
        let mut options = script.options(true, CoolingHold::PumpOnly);
        let path = script.observed_journal.lock().unwrap().clone().unwrap();
        match mismatch {
            "scope" => {
                options.journal = crate::firmware_operation::Journal::new(
                    script.journal_root.path().join("foreign"),
                )
            }
            "serial" => options.target.serial = Some("different".into()),
            "path" => options.target.path = "/mock/replaced".into(),
            "hold" => options.cooling_hold = CoolingHold::PumpAndControllerFan,
            "missing" => std::fs::remove_file(path.join("firmware-operation.json")).unwrap(),
            "changed" => {
                std::fs::write(path.join("firmware-operation.json"), b"external edit").unwrap()
            }
            _ => unreachable!(),
        }
        let (_cancel, cancel) = watch::channel(false);
        let (_stop, stop) = watch::channel(false);
        let (progress, _) = watch::channel(Progress::default());
        let result = run_timed(
            &*script.open_handle(),
            synthetic(),
            options,
            cancel,
            stop,
            progress,
            fast(),
        );
        assert!(matches!(
            result.progress.phase,
            Phase::NotStarted | Phase::Unknown
        ));
        assert!(script.history().is_empty());
        assert!(!result.progress.pump_hold_written);
        if mismatch == "changed" {
            assert_eq!(
                std::fs::read(path.join("firmware-operation.json")).unwrap(),
                b"external edit"
            );
        }
        if mismatch == "hold" {
            assert_eq!(
                crate::firmware_operation::Journal::new(path)
                    .load()
                    .unwrap()
                    .unwrap()
                    .stage,
                crate::firmware_operation::Stage::Armed
            );
        }
    }
}
#[test]
fn changed_journal_after_query_refuses_first_hold_and_any_flash() {
    let mut script = Script::new(Fault::None);
    script.tamper_after_query = true;
    let result = run_script(&script, synthetic(), true, false);
    assert_eq!(result.progress.phase, Phase::Unknown);
    assert_eq!(
        script.history().iter().map(|p| p[1]).collect::<Vec<_>>(),
        [134]
    );
    assert!(result.progress.attempted.is_none());
    assert!(!result.progress.pump_hold_written);
    assert!(result.diagnostic.unwrap().contains("outcome not durable"));
}
#[test]
fn failed_terminal_commit_cannot_report_success_despite_all_wire_acks() {
    let mut script = Script::new(Fault::None);
    script.tamper_on_activation = true;
    let result = run_script(&script, synthetic(), true, false);
    assert_eq!(result.progress.phase, Phase::Unknown);
    assert_eq!(result.progress.chunk_echoes, 619);
    assert!(result.progress.activation_written);
    assert_eq!(result.writes(), 622);
    let path = script.observed_journal.lock().unwrap().clone().unwrap();
    assert_eq!(
        std::fs::read(path.join("firmware-operation.json")).unwrap(),
        b"external edit"
    );
    assert!(crate::firmware_operation::Journal::new(path)
        .ensure_replay_allowed(&script.info)
        .is_err());
}
#[test]
fn completed_outcomes_are_reopened_from_disk_with_the_exact_transport_progress() {
    for fault in [Fault::None, Fault::PartialWrite, Fault::WrongVersion] {
        let script = Script::new(fault);
        let result = run_script(&script, synthetic(), true, false);
        let path = script.observed_journal.lock().unwrap().clone().unwrap();
        let record = crate::firmware_operation::Journal::new(path)
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(record.progress.unwrap(), result.progress);
        assert_eq!(
            record.stage,
            match fault {
                Fault::None => crate::firmware_operation::Stage::AwaitingReconnect,
                Fault::WrongVersion => crate::firmware_operation::Stage::NotStarted,
                _ => crate::firmware_operation::Stage::Unknown,
            }
        );
    }
}

#[tokio::test]
async fn actor_without_bound_store_scope_cannot_start_an_update() {
    let script = Script::new(Fault::None);
    let (stop, shutdown) = watch::channel(false);
    let (tx, task) = hid_io::spawn_tracked(
        Arc::new(script.clone()),
        script.info.clone(),
        Arc::new(Stats::new()),
        shutdown,
    )
    .unwrap();
    let (_cancel, cancel) = watch::channel(false);
    let (progress, _) = watch::channel(Progress::default());
    let (reply, result) = oneshot::channel();
    tx.send(HidCommand::FirmwareUpdate {
        operation: Box::new(script.operation(CoolingHold::PumpOnly)),
        image: synthetic(),
        cooling_hold: CoolingHold::PumpOnly,
        cancel,
        progress,
        reply,
    })
    .await
    .unwrap();
    let result = result.await.unwrap();
    assert_eq!(result.progress.phase, Phase::NotStarted);
    assert!(result
        .diagnostic
        .unwrap()
        .contains("no firmware journal scope"));
    assert!(script.history().is_empty());
    let path = script.observed_journal.lock().unwrap().clone().unwrap();
    assert_eq!(
        crate::firmware_operation::Journal::new(path)
            .load()
            .unwrap()
            .unwrap()
            .stage,
        crate::firmware_operation::Stage::Armed
    );
    stop.send(true).unwrap();
    task.await.unwrap();
}
