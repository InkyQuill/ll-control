//! Integration test for the hid_io actor against the in-process
//! `MockBackend`. Verifies that a `WriteRead64` command queued on the
//! actor's mpsc returns the canned A-class response from the mock.

use llcontrol_daemon::actor::{hid_io, FrameReadPolicy, HidCommand};
use llcontrol_daemon::stats::Stats;
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, watch};

fn lcdv2_info() -> DeviceInfo {
    // `DeviceInfo` is `#[non_exhaustive]`; use the public constructor.
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        None,
        PathBuf::from("/mock/hidraw0"),
    )
}

#[tokio::test]
async fn write_read_64_returns_canned_response() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    // Queue an A-class echo response: [report_id=0x01, cmd_echo=0x86, ...].
    let mut canned = vec![0u8; 64];
    canned[0] = 0x01;
    canned[1] = 0x86;
    mock.queue_response(&lcdv2_info(), canned);

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");

    let (reply_tx, reply_rx) = oneshot::channel();
    let mut bytes = vec![0u8; 64];
    bytes[0] = 0x01;
    bytes[1] = 0x86;
    tx.send(HidCommand::WriteRead64 {
        bytes,
        expected_cmd: 0x86,
        reply: reply_tx,
    })
    .await
    .expect("send hid command");

    let resp = reply_rx
        .await
        .expect("hid_io dropped reply channel")
        .expect("hid_io returned an error");
    assert_eq!(resp[0], 0x01);
    assert_eq!(resp[1], 0x86);
    assert_eq!(
        stats.hid_write_count.load(Ordering::Relaxed),
        1,
        "hid_io must bump hid_write_count once on a successful WriteRead64"
    );
}

#[tokio::test]
async fn write_read_512_c_class_returns_canned_response() {
    // C-class framing uses report_id 0x03 (e.g., SendH264Frame512 cmd 0x0d).
    // Pre-fix, drain_for_echo_512 hardcoded report_id 0x02 and would
    // discard this response as stale, exhausting the retry budget and
    // poisoning the device's USB read buffer for the next call.
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let mut canned = vec![0u8; 512];
    canned[0] = 0x03;
    canned[1] = 0x0d;
    mock.queue_response(&lcdv2_info(), canned);

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");

    let (reply_tx, reply_rx) = oneshot::channel();
    let mut bytes = vec![0u8; 512];
    bytes[0] = 0x03;
    bytes[1] = 0x0d;
    tx.send(HidCommand::WriteRead512 {
        bytes,
        expected_report_id: 0x03,
        expected_cmd: 0x0d,
        reply: reply_tx,
    })
    .await
    .expect("send hid command");

    let resp = reply_rx
        .await
        .expect("hid_io dropped reply channel")
        .expect("hid_io returned an error");
    assert_eq!(resp[0], 0x03);
    assert_eq!(resp[1], 0x0d);
}

/// Pinning the regression that motivated moving the counter into hid_io:
/// a plain `HidCommand::Write` (the variant that keeper-driven writes
/// use) must bump `Stats::hid_write_count`. Pre-fix, only the IPC
/// helpers counted, so keeper-driven writes (lighting_keeper at 1.5s,
/// lcd_keeper at 15s) silently undercounted `daemon stats hid_write_count`.
#[tokio::test]
async fn write_command_increments_hid_write_count() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![0x01, 0x86, 0x00, 0x00, 0x00, 0x00],
        reply: reply_tx,
    })
    .await
    .expect("send hid command");

    reply_rx
        .await
        .expect("hid_io dropped reply channel")
        .expect("hid_io returned an error");

    assert_eq!(
        stats.hid_write_count.load(Ordering::Relaxed),
        1,
        "hid_io must bump hid_write_count on a successful Write"
    );
}

#[tokio::test]
async fn frame_batch_keeps_fragments_contiguous_before_following_write() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let mock_for_assert = mock.clone();
    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");

    let packets =
        llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(&vec![
            0x55;
            1200
        ])
        .expect("build H.264 packets");
    assert_eq!(packets.len(), 3);

    let (batch_reply_tx, batch_reply_rx) = oneshot::channel();
    tx.send(HidCommand::FrameBatch {
        packets: packets.iter().map(|packet| packet.to_vec()).collect(),
        read_policy: FrameReadPolicy::None,
        reply: batch_reply_tx,
    })
    .await
    .expect("send frame batch");
    let (sensor_reply_tx, sensor_reply_rx) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![0x01, 129],
        reply: sensor_reply_tx,
    })
    .await
    .expect("send sensor write");

    let outcome = batch_reply_rx
        .await
        .expect("batch reply")
        .expect("batch succeeds");
    sensor_reply_rx
        .await
        .expect("sensor reply")
        .expect("sensor succeeds");
    assert_eq!(outcome.written_reports, 3);
    assert!(outcome.readback.is_empty());
    let history = mock_for_assert.write_history(&lcdv2_info());
    assert_eq!(
        history.iter().map(|packet| packet[1]).collect::<Vec<_>>(),
        [13, 13, 13, 129]
    );
    assert_eq!(
        history[..3]
            .iter()
            .map(|packet| packet[8])
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(stats.hid_write_count.load(Ordering::Relaxed), 4);
}

#[tokio::test]
async fn frame_batch_observation_preserves_raw_bytes_and_timeout() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    mock.queue_response(&lcdv2_info(), vec![0x7f, 0xee, 0x42]);
    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let tx =
        hid_io::spawn(backend, lcdv2_info(), Arc::new(Stats::new()), sd_rx).expect("spawn hid_io");

    let packet = llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(&[1])
        .expect("build observation packet")[0]
        .to_vec();
    let (raw_tx, raw_rx) = oneshot::channel();
    tx.send(HidCommand::FrameBatch {
        packets: vec![packet.clone()],
        read_policy: FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 },
        reply: raw_tx,
    })
    .await
    .expect("send raw observation batch");
    assert_eq!(
        raw_rx
            .await
            .expect("raw reply")
            .expect("raw batch succeeds")
            .readback,
        [0x7f, 0xee, 0x42]
    );

    let (timeout_tx, timeout_rx) = oneshot::channel();
    tx.send(HidCommand::FrameBatch {
        packets: vec![packet],
        read_policy: FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 },
        reply: timeout_tx,
    })
    .await
    .expect("send timeout batch");
    assert!(timeout_rx
        .await
        .expect("timeout reply")
        .expect("timeout is a successful observation")
        .readback
        .is_empty());
}

#[derive(Clone)]
struct ControlledBackend {
    history: Arc<Mutex<Vec<Vec<u8>>>>,
    fail_write_at: Option<usize>,
    read_count: usize,
}

impl HidBackend for ControlledBackend {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        Ok(vec![lcdv2_info()])
    }

    fn open(&self, _info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
        Ok(Box::new(ControlledHandle {
            history: Arc::clone(&self.history),
            fail_write_at: self.fail_write_at,
            read_count: self.read_count,
        }))
    }
}

struct ControlledHandle {
    history: Arc<Mutex<Vec<Vec<u8>>>>,
    fail_write_at: Option<usize>,
    read_count: usize,
}

impl OpenHandle for ControlledHandle {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        let mut history = self.history.lock().expect("history mutex poisoned");
        if self.fail_write_at == Some(history.len()) {
            return Err(llcontrol_hid::Error::DeviceNotFound(
                "injected failure".into(),
            ));
        }
        history.push(bytes.to_vec());
        Ok(())
    }

    fn read_timeout(&self, _buf: &mut [u8], _timeout_ms: i32) -> llcontrol_hid::Result<usize> {
        Ok(self.read_count)
    }
}

#[tokio::test]
async fn frame_batch_stops_at_first_failure_and_counts_prior_success() {
    let history = Arc::new(Mutex::new(Vec::new()));
    let backend: Arc<dyn HidBackend> = Arc::new(ControlledBackend {
        history: Arc::clone(&history),
        fail_write_at: Some(1),
        read_count: 0,
    });
    let (_sd_tx, sd_rx) = watch::channel(false);
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");
    let (reply_tx, reply_rx) = oneshot::channel();

    let packets =
        llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(&vec![
            0x55;
            1200
        ])
        .expect("build H.264 packets");
    tx.send(HidCommand::FrameBatch {
        packets: packets.iter().map(|packet| packet.to_vec()).collect(),
        read_policy: FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 },
        reply: reply_tx,
    })
    .await
    .expect("send failing batch");

    assert!(matches!(
        reply_rx.await.expect("failure reply"),
        Err(llcontrol_hid::Error::FrameBatchFailed {
            written_reports: 1,
            ..
        })
    ));
    assert_eq!(history.lock().expect("history mutex poisoned").len(), 1);
    assert_eq!(stats.hid_write_count.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn frame_batch_rejects_malformed_transactions_before_any_write() {
    let base = llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(&vec![
            0x55;
            1200
        ])
    .expect("build H.264 packets")
    .into_iter()
    .map(|packet| packet.to_vec())
    .collect::<Vec<_>>();
    let mut cases = Vec::new();
    cases.push(("empty", Vec::new()));
    let mut wrong_size = base.clone();
    wrong_size[0].pop();
    cases.push(("wrong size", wrong_size));
    let mut wrong_report = base.clone();
    wrong_report[0][0] = 0x02;
    cases.push(("wrong report", wrong_report));
    let mut mixed_command = base.clone();
    mixed_command[1][1] = 14;
    cases.push(("mixed command", mixed_command));
    let mut wrong_number = base.clone();
    wrong_number[1][8] = 9;
    cases.push(("wrong packet number", wrong_number));
    let mut wrong_length = base.clone();
    wrong_length[0][9..11].copy_from_slice(&1u16.to_be_bytes());
    cases.push(("wrong payload length", wrong_length));
    let mut wrong_total = base.clone();
    wrong_total[1][2..6].copy_from_slice(&1199u32.to_be_bytes());
    cases.push(("inconsistent total", wrong_total));
    let mut nonzero_tail = base.clone();
    nonzero_tail[2][211] = 1;
    cases.push(("non-zero tail", nonzero_tail));

    for (name, packets) in cases {
        let mock = MockBackend::new();
        mock.add_device(lcdv2_info());
        let mock_for_assert = mock.clone();
        let backend: Arc<dyn HidBackend> = Arc::new(mock);
        let (_sd_tx, sd_rx) = watch::channel(false);
        let tx = hid_io::spawn(backend, lcdv2_info(), Arc::new(Stats::new()), sd_rx)
            .expect("spawn hid_io");
        let (reply_tx, reply_rx) = oneshot::channel();
        tx.send(HidCommand::FrameBatch {
            packets,
            read_policy: FrameReadPolicy::None,
            reply: reply_tx,
        })
        .await
        .expect("send malformed batch");
        let error = reply_rx
            .await
            .expect("malformed reply")
            .expect_err("malformed batch must fail");
        assert!(
            error.to_string().contains("invalid frame batch"),
            "{name}: {error}"
        );
        assert!(
            mock_for_assert.write_history(&lcdv2_info()).is_empty(),
            "{name} reached HID write"
        );
    }
}

#[tokio::test]
async fn frame_batch_rejects_tiny_first_packets_and_actor_remains_usable() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let mock_for_assert = mock.clone();
    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let tx =
        hid_io::spawn(backend, lcdv2_info(), Arc::new(Stats::new()), sd_rx).expect("spawn hid_io");

    for length in 0..=11 {
        let mut packet = vec![0; length];
        if length >= 2 {
            packet[0] = 0x03;
            packet[1] = 13;
        }
        let (reply_tx, reply_rx) = oneshot::channel();
        tx.send(HidCommand::FrameBatch {
            packets: vec![packet],
            read_policy: FrameReadPolicy::None,
            reply: reply_tx,
        })
        .await
        .expect("send tiny batch");
        let error = reply_rx
            .await
            .expect("tiny batch reply")
            .expect_err("tiny batch must fail");
        assert!(
            error.to_string().contains("invalid frame batch"),
            "length {length}: {error}"
        );
    }
    assert!(mock_for_assert.write_history(&lcdv2_info()).is_empty());

    let valid = llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(&[1])
        .expect("build valid packet")[0]
        .to_vec();
    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(HidCommand::FrameBatch {
        packets: vec![valid],
        read_policy: FrameReadPolicy::None,
        reply: reply_tx,
    })
    .await
    .expect("send valid batch after rejection");
    let outcome = reply_rx
        .await
        .expect("valid batch reply")
        .expect("actor remains usable");
    assert_eq!(outcome.written_reports, 1);
    assert_eq!(mock_for_assert.write_history(&lcdv2_info()).len(), 1);
}

#[tokio::test]
async fn frame_batch_rejects_invalid_observation_timeout_before_write() {
    for timeout_ms in [-1, 1_001] {
        let mock = MockBackend::new();
        mock.add_device(lcdv2_info());
        let mock_for_assert = mock.clone();
        let backend: Arc<dyn HidBackend> = Arc::new(mock);
        let (_sd_tx, sd_rx) = watch::channel(false);
        let tx = hid_io::spawn(backend, lcdv2_info(), Arc::new(Stats::new()), sd_rx)
            .expect("spawn hid_io");
        let packet =
            llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(&[1])
                .expect("build observation packet")[0]
                .to_vec();
        let (reply_tx, reply_rx) = oneshot::channel();
        tx.send(HidCommand::FrameBatch {
            packets: vec![packet],
            read_policy: FrameReadPolicy::ObserveOnce512 { timeout_ms },
            reply: reply_tx,
        })
        .await
        .expect("send invalid timeout batch");
        assert!(reply_rx.await.expect("timeout validation reply").is_err());
        assert!(mock_for_assert.write_history(&lcdv2_info()).is_empty());
    }
}

#[tokio::test]
async fn frame_batch_rejects_impossible_oversized_read_count_without_panicking() {
    let history = Arc::new(Mutex::new(Vec::new()));
    let backend: Arc<dyn HidBackend> = Arc::new(ControlledBackend {
        history: Arc::clone(&history),
        fail_write_at: None,
        read_count: 513,
    });
    let (_sd_tx, sd_rx) = watch::channel(false);
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");
    let packet = llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(&[1])
        .expect("build observation packet")[0]
        .to_vec();
    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(HidCommand::FrameBatch {
        packets: vec![packet],
        read_policy: FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 },
        reply: reply_tx,
    })
    .await
    .expect("send oversized-read batch");

    let error = reply_rx
        .await
        .expect("oversized-read reply")
        .expect_err("oversized read count must fail");
    assert!(error.to_string().contains("read returned 513 bytes"));
    assert_eq!(history.lock().expect("history mutex poisoned").len(), 1);
    assert_eq!(stats.hid_write_count.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn echo_reads_reject_truncated_headers_and_declared_payloads_for_all_classes() {
    for report_id in [1, 2, 3] {
        let header = if report_id == 1 { 6 } else { 11 };
        for size in [2, header] {
            let mock = MockBackend::new();
            mock.add_device(lcdv2_info());
            let mut response = vec![0; size];
            response[0] = report_id;
            response[1] = 134;
            if size == header {
                response[header - 1] = 1;
            }
            mock.queue_response(&lcdv2_info(), response);
            let (_stop, sd) = watch::channel(false);
            let tx =
                hid_io::spawn(Arc::new(mock), lcdv2_info(), Arc::new(Stats::new()), sd).unwrap();
            let error = if report_id == 1 {
                let (reply, rx) = oneshot::channel();
                tx.send(HidCommand::WriteRead64 {
                    bytes: vec![1, 134],
                    expected_cmd: 134,
                    reply,
                })
                .await
                .unwrap();
                rx.await.unwrap().unwrap_err()
            } else {
                let (reply, rx) = oneshot::channel();
                tx.send(HidCommand::WriteRead512 {
                    bytes: vec![report_id, 134],
                    expected_report_id: report_id,
                    expected_cmd: 134,
                    reply,
                })
                .await
                .unwrap();
                rx.await.unwrap().unwrap_err()
            };
            assert!(
                matches!(
                    error,
                    llcontrol_hid::Error::EchoExhausted {
                        stale: 1,
                        timeout: 3,
                        ..
                    }
                ),
                "report{report_id} n{size}: {error}"
            );
        }
    }
}

#[tokio::test]
async fn short_complete_echo_has_no_bytes_left_from_prior_response() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let mut stale = vec![42; 64];
    stale[0] = 1;
    stale[1] = 133;
    mock.queue_response(&lcdv2_info(), stale);
    mock.queue_response(&lcdv2_info(), vec![1, 134, 0, 0, 0, 0]);
    let (_stop, sd) = watch::channel(false);
    let tx = hid_io::spawn(Arc::new(mock), lcdv2_info(), Arc::new(Stats::new()), sd).unwrap();
    let (reply, rx) = oneshot::channel();
    tx.send(HidCommand::WriteRead64 {
        bytes: vec![1, 134],
        expected_cmd: 134,
        reply,
    })
    .await
    .unwrap();
    let response = rx.await.unwrap().unwrap();
    assert!(response[6..].iter().all(|byte| *byte == 0));
}

#[tokio::test]
async fn echo_reads_reject_impossible_read_counts_without_panicking() {
    for report_id in [1, 2, 3] {
        let count = if report_id == 1 { 65 } else { 513 };
        let backend = ControlledBackend {
            history: Arc::new(Mutex::new(Vec::new())),
            fail_write_at: None,
            read_count: count,
        };
        let (_stop, sd) = watch::channel(false);
        let tx =
            hid_io::spawn(Arc::new(backend), lcdv2_info(), Arc::new(Stats::new()), sd).unwrap();
        let error = if report_id == 1 {
            let (reply, rx) = oneshot::channel();
            tx.send(HidCommand::WriteRead64 {
                bytes: vec![1, 134],
                expected_cmd: 134,
                reply,
            })
            .await
            .unwrap();
            rx.await.unwrap().unwrap_err()
        } else {
            let (reply, rx) = oneshot::channel();
            tx.send(HidCommand::WriteRead512 {
                bytes: vec![report_id, 134],
                expected_report_id: report_id,
                expected_cmd: 134,
                reply,
            })
            .await
            .unwrap();
            rx.await.unwrap().unwrap_err()
        };
        assert!(error
            .to_string()
            .contains(&format!("read returned {count} bytes")));
    }
}

#[derive(Default)]
struct SensorReadGate {
    entered: tokio::sync::Notify,
    released: Mutex<bool>,
    release: std::sync::Condvar,
}

#[derive(Clone, Default)]
struct SensorBackend {
    history: Arc<Mutex<Vec<Vec<u8>>>>,
    responses: Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
    observation_only: bool,
    fail_read: bool,
    read_gate: Option<Arc<SensorReadGate>>,
}

impl HidBackend for SensorBackend {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        Ok(vec![lcdv2_info()])
    }
    fn open(&self, _: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
        Ok(Box::new(self.clone()))
    }
}

impl OpenHandle for SensorBackend {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        self.history.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
    fn read_timeout(&self, buffer: &mut [u8], timeout_ms: i32) -> llcontrol_hid::Result<usize> {
        if let Some(gate) = &self.read_gate {
            gate.entered.notify_one();
            let released = gate.released.lock().unwrap();
            drop(
                gate.release
                    .wait_while(released, |released| !*released)
                    .unwrap(),
            );
        }
        if self.observation_only && timeout_ms != 20 {
            return Ok(0);
        }
        if self.fail_read {
            return Err(llcontrol_hid::Error::DeviceNotFound(
                "injected read failure".into(),
            ));
        }
        let Some(bytes) = self.responses.lock().unwrap().pop_front() else {
            return Ok(0);
        };
        let count = bytes.len().min(buffer.len());
        buffer[..count].copy_from_slice(&bytes[..count]);
        Ok(count)
    }
}

fn sensor_response() -> Vec<u8> {
    vec![1, 129, 0, 0, 0, 7, 4, 210, 11, 164, 1, 42, 7]
}

async fn enqueue_sensor(
    tx: &tokio::sync::mpsc::Sender<HidCommand>,
) -> oneshot::Receiver<llcontrol_hid::Result<[u8; 64]>> {
    let (reply, rx) = oneshot::channel();
    tx.send(HidCommand::PollSensors { reply }).await.unwrap();
    rx
}

async fn enqueue_sensor_frame(
    tx: &tokio::sync::mpsc::Sender<HidCommand>,
    read_policy: FrameReadPolicy,
) -> oneshot::Receiver<llcontrol_hid::Result<llcontrol_daemon::actor::FrameOutcome>> {
    let (reply, rx) = oneshot::channel();
    let packets =
        llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(&vec![
            0x55;
            1200
        ])
        .unwrap()
        .iter()
        .map(|p| p.to_vec())
        .collect();
    tx.send(HidCommand::FrameBatch {
        packets,
        read_policy,
        reply,
    })
    .await
    .unwrap();
    rx
}

#[tokio::test]
async fn delayed_sensor_response_permits_whole_frames_and_idle_completion() {
    let backend = SensorBackend::default();
    let (shutdown, shutdown_rx) = watch::channel(false);
    let (tx, task) = hid_io::spawn_tracked(
        Arc::new(backend.clone()),
        lcdv2_info(),
        Arc::new(Stats::new()),
        shutdown_rx,
    )
    .unwrap();
    let mut sensor = enqueue_sensor(&tx).await;
    let frame = enqueue_sensor_frame(&tx, FrameReadPolicy::None).await;
    tokio::time::timeout(std::time::Duration::from_millis(250), frame)
        .await
        .expect("pending sensor must not stall frame")
        .unwrap()
        .unwrap();
    assert!(matches!(
        sensor.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert_eq!(
        backend
            .history
            .lock()
            .unwrap()
            .iter()
            .map(|p| p[1])
            .collect::<Vec<_>>(),
        [129, 13, 13, 13]
    );
    backend
        .responses
        .lock()
        .unwrap()
        .push_back(sensor_response());
    let response = tokio::time::timeout(std::time::Duration::from_millis(250), sensor)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(&response[..13], sensor_response());
    shutdown.send(true).unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn sensor_response_in_frame_observation_is_routed_and_retained_raw() {
    let backend = SensorBackend {
        observation_only: true,
        ..Default::default()
    };
    backend
        .responses
        .lock()
        .unwrap()
        .push_back(sensor_response());
    let (_shutdown, shutdown_rx) = watch::channel(false);
    let tx = hid_io::spawn(
        Arc::new(backend.clone()),
        lcdv2_info(),
        Arc::new(Stats::new()),
        shutdown_rx,
    )
    .unwrap();
    let sensor = enqueue_sensor(&tx).await;
    let frame = enqueue_sensor_frame(&tx, FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 })
        .await
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frame.readback, sensor_response());
    assert_eq!(&sensor.await.unwrap().unwrap()[..13], sensor_response());
    assert_eq!(
        backend
            .history
            .lock()
            .unwrap()
            .iter()
            .map(|p| p[1])
            .collect::<Vec<_>>(),
        [129, 13, 13, 13]
    );
}

#[tokio::test]
async fn pending_sensor_rejects_malformed_and_unrelated_reports_until_timeout() {
    let backend = SensorBackend::default();
    let mut pn1 = sensor_response();
    pn1[4] = 1;
    let mut firmware = sensor_response();
    firmware[1] = 134;
    firmware[4] = 1;
    let mut truncated = sensor_response();
    truncated.pop();
    let mut too_short = sensor_response();
    too_short[5] = 6;
    backend.responses.lock().unwrap().extend([
        pn1,
        firmware,
        truncated,
        too_short,
        vec![2, 12, 0, 0, 0, 8, 0, 0, 0, 0, 8],
    ]);
    let (_shutdown, shutdown_rx) = watch::channel(false);
    let tx = hid_io::spawn(
        Arc::new(backend),
        lcdv2_info(),
        Arc::new(Stats::new()),
        shutdown_rx,
    )
    .unwrap();
    let response =
        tokio::time::timeout(std::time::Duration::from_secs(3), enqueue_sensor(&tx).await)
            .await
            .unwrap()
            .unwrap();
    assert!(matches!(
        response,
        Err(llcontrol_hid::Error::EchoExhausted {
            expected_cmd: 129,
            ..
        })
    ));
}

#[tokio::test]
async fn second_read_preserves_fifo_before_later_frame() {
    let backend = SensorBackend::default();
    let (_shutdown, shutdown_rx) = watch::channel(false);
    let tx = hid_io::spawn(
        Arc::new(backend.clone()),
        lcdv2_info(),
        Arc::new(Stats::new()),
        shutdown_rx,
    )
    .unwrap();
    let sensor = enqueue_sensor(&tx).await;
    enqueue_sensor_frame(&tx, FrameReadPolicy::None)
        .await
        .await
        .unwrap()
        .unwrap();
    let (reply, other_read) = oneshot::channel();
    tx.send(HidCommand::WriteRead64 {
        bytes: vec![1, 134, 0, 0, 0, 0],
        expected_cmd: 134,
        reply,
    })
    .await
    .unwrap();
    let mut later_frame = enqueue_sensor_frame(&tx, FrameReadPolicy::None).await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut later_frame)
            .await
            .is_err()
    );
    assert_eq!(backend.history.lock().unwrap().len(), 4);
    backend
        .responses
        .lock()
        .unwrap()
        .extend([sensor_response(), vec![1, 134, 0, 0, 0, 0]]);
    sensor.await.unwrap().unwrap();
    other_read.await.unwrap().unwrap();
    later_frame.await.unwrap().unwrap();
    assert_eq!(
        backend
            .history
            .lock()
            .unwrap()
            .iter()
            .map(|p| p[1])
            .collect::<Vec<_>>(),
        [129, 13, 13, 13, 134, 13, 13, 13]
    );
}

#[tokio::test]
async fn pending_sensor_read_errors_propagate_from_idle_and_frame_observation() {
    for observation_only in [false, true] {
        let backend = SensorBackend {
            observation_only,
            fail_read: true,
            ..Default::default()
        };
        let (_shutdown, shutdown_rx) = watch::channel(false);
        let tx = hid_io::spawn(
            Arc::new(backend),
            lcdv2_info(),
            Arc::new(Stats::new()),
            shutdown_rx,
        )
        .unwrap();
        let sensor = enqueue_sensor(&tx).await;
        if observation_only {
            assert!(matches!(
                enqueue_sensor_frame(&tx, FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 })
                    .await
                    .await
                    .unwrap(),
                Err(llcontrol_hid::Error::FrameBatchFailed {
                    written_reports: 3,
                    ..
                })
            ));
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(250), sensor)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
    }
}

#[tokio::test]
async fn shutdown_cancels_pending_sensor_and_joins_actor() {
    let backend = SensorBackend::default();
    let (shutdown, shutdown_rx) = watch::channel(false);
    let (tx, task) = hid_io::spawn_tracked(
        Arc::new(backend),
        lcdv2_info(),
        Arc::new(Stats::new()),
        shutdown_rx,
    )
    .unwrap();
    let sensor = enqueue_sensor(&tx).await;
    enqueue_sensor_frame(&tx, FrameReadPolicy::None)
        .await
        .await
        .unwrap()
        .unwrap();
    shutdown.send(true).unwrap();
    tokio::time::timeout(std::time::Duration::from_millis(250), task)
        .await
        .unwrap()
        .unwrap();
    assert!(sensor.await.is_err());
    assert!(tx.is_closed());
}

#[tokio::test]
async fn dropped_shutdown_sender_during_pending_read_cancels_deferred_command() {
    let gate = Arc::new(SensorReadGate::default());
    let backend = SensorBackend {
        read_gate: Some(Arc::clone(&gate)),
        ..Default::default()
    };
    backend
        .responses
        .lock()
        .unwrap()
        .extend([sensor_response(), vec![1, 134, 0, 0, 0, 0]]);
    let (shutdown, shutdown_rx) = watch::channel(false);
    let (tx, task) = hid_io::spawn_tracked(
        Arc::new(backend.clone()),
        lcdv2_info(),
        Arc::new(Stats::new()),
        shutdown_rx,
    )
    .unwrap();
    let sensor = enqueue_sensor(&tx).await;
    let (reply, deferred) = oneshot::channel();
    tx.send(HidCommand::WriteRead64 {
        bytes: vec![1, 134, 0, 0, 0, 0],
        expected_cmd: 134,
        reply,
    })
    .await
    .unwrap();
    // Both commands are queued before yielding; the actor defers the FIFO
    // read before entering its pending-sensor read on the blocking pool.
    gate.entered.notified().await;
    drop(shutdown);
    assert!(!task.is_finished(), "teardown must join the started read");
    *gate.released.lock().unwrap() = true;
    gate.release.notify_all();
    tokio::time::timeout(std::time::Duration::from_millis(250), task)
        .await
        .unwrap()
        .unwrap();
    sensor.await.unwrap().unwrap();
    assert!(
        deferred.await.is_err(),
        "deferred command must be cancelled after sender drop"
    );
    assert_eq!(
        backend
            .history
            .lock()
            .unwrap()
            .iter()
            .map(|p| p[1])
            .collect::<Vec<_>>(),
        [129]
    );
    assert!(tx.is_closed());
}
