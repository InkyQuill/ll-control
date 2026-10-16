//! Wire-shaped firmware replies, injected only after the single cmd134 write.
use lconnect_daemon::actor::{hid_io, FirmwareResponses, HidCommand};
use lconnect_daemon::stats::Stats;
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use lconnect_protocol::galahad2_lcdv2::{firmware::FirmwareVersion, packets::ACommandPacket};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use tokio::sync::{oneshot, watch};

fn info() -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        None,
        PathBuf::from("/mock/firmware"),
    )
}
fn packet(pn: u16, data: &[u8]) -> Vec<u8> {
    let mut bytes = ACommandPacket::new_request(134, data).as_bytes().to_vec();
    bytes[3..5].copy_from_slice(&pn.to_be_bytes());
    bytes
}
fn version() -> Vec<u8> {
    packet(0, b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6")
}
fn date() -> Vec<u8> {
    packet(1, b"Apr 12 2024 14:33:21")
}
#[derive(Clone)]
struct Script {
    mock: MockBackend,
    replies: Arc<Vec<Vec<u8>>>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    entered: Arc<AtomicBool>,
}
struct Handle {
    inner: Box<dyn OpenHandle>,
    script: Script,
    info: DeviceInfo,
}
impl HidBackend for Script {
    fn enumerate(&self) -> lconnect_hid::Result<Vec<DeviceInfo>> {
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> lconnect_hid::Result<Box<dyn OpenHandle>> {
        Ok(Box::new(Handle {
            inner: self.mock.open(info)?,
            script: self.clone(),
            info: info.clone(),
        }))
    }
}
impl OpenHandle for Handle {
    fn write(&self, bytes: &[u8]) -> lconnect_hid::Result<()> {
        self.inner.write(bytes)?;
        if bytes.starts_with(&[1, 134]) {
            for reply in self.script.replies.iter() {
                self.script.mock.queue_response(&self.info, reply.clone());
            }
        }
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], timeout: i32) -> lconnect_hid::Result<usize> {
        self.script.entered.store(true, Ordering::Release);
        let (lock, condition) = &*self.script.gate;
        let guard = lock.lock().unwrap();
        let _guard = condition
            .wait_timeout_while(guard, std::time::Duration::from_millis(500), |released| {
                !*released
            })
            .unwrap();
        self.inner.read_timeout(bytes, timeout)
    }
}
fn script(replies: Vec<Vec<u8>>, released: bool) -> Script {
    let mock = MockBackend::new();
    mock.add_device(info());
    Script {
        mock,
        replies: Arc::new(replies),
        gate: Arc::new((Mutex::new(released), Condvar::new())),
        entered: Arc::new(AtomicBool::new(false)),
    }
}
async fn probe(replies: Vec<Vec<u8>>) -> lconnect_hid::Result<FirmwareResponses> {
    let script = script(replies, true);
    let stats = Arc::new(Stats::new());
    let (stop, shutdown) = watch::channel(false);
    let (tx, task) =
        hid_io::spawn_tracked(Arc::new(script.clone()), info(), stats.clone(), shutdown).unwrap();
    let (reply, rx) = oneshot::channel();
    tx.send(HidCommand::Firmware { reply }).await.unwrap();
    let result = rx.await.unwrap();
    assert_eq!(
        script.mock.write_history(&info()),
        vec![lconnect_protocol::galahad2_lcdv2::firmware::build_request().to_vec()]
    );
    assert_eq!(stats.hid_write_count.load(Ordering::Relaxed), 1);
    stop.send(true).unwrap();
    task.await.unwrap();
    result
}

#[tokio::test]
async fn one_query_classifies_packet_numbers_instead_of_read_order_and_ignores_unrelated_or_short_reports(
) {
    for replies in [
        vec![version(), date()],
        vec![date(), version()],
        vec![version(), version(), date()],
        vec![
            vec![1, 134, 0, 0, 0, 58],
            vec![1, 129, 0, 0, 0, 0],
            date(),
            version(),
        ],
        vec![packet(2, b"wrong role"), date(), version()],
    ] {
        let result = probe(replies).await.unwrap();
        let parsed = FirmwareVersion::from_optional_responses(
            &result.version,
            result.datetime.as_ref().map(|b| b.as_slice()),
        )
        .unwrap();
        assert_eq!(parsed.user_version, "1.6");
        assert_eq!(parsed.raw_datetime, "Apr 12 2024 14:33:21");
    }
}
#[tokio::test]
async fn duplicate_or_missing_date_never_reuses_version_and_missing_version_fails_after_one_write()
{
    for replies in [vec![version()], vec![version(), version()]] {
        let result = probe(replies).await.unwrap();
        assert!(result.datetime.is_none());
    }
    for replies in [vec![], vec![date()], vec![packet(2, b"wrong role")]] {
        assert!(probe(replies).await.is_err());
    }
}
#[tokio::test]
async fn another_writer_cannot_split_the_firmware_pair_and_shutdown_joins_the_transaction() {
    let script = script(vec![date(), version()], false);
    let (stop, shutdown) = watch::channel(false);
    let (tx, task) = hid_io::spawn_tracked(
        Arc::new(script.clone()),
        info(),
        Arc::new(Stats::new()),
        shutdown,
    )
    .unwrap();
    let (reply, firmware) = oneshot::channel();
    tx.send(HidCommand::Firmware { reply }).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !script.entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (reply, control) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![1, 135],
        reply,
    })
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert_eq!(script.mock.write_history(&info()).len(), 1);
    *script.gate.0.lock().unwrap() = true;
    script.gate.1.notify_all();
    assert!(firmware.await.unwrap().unwrap().datetime.is_some());
    control.await.unwrap().unwrap();
    assert_eq!(
        script
            .mock
            .write_history(&info())
            .iter()
            .map(|bytes| bytes[1])
            .collect::<Vec<_>>(),
        vec![134, 135]
    );
    stop.send(true).unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn shutdown_waits_for_started_firmware_read_and_discards_queued_writes() {
    let script = script(vec![version(), date()], false);
    let (stop, shutdown) = watch::channel(false);
    let (tx, task) = hid_io::spawn_tracked(
        Arc::new(script.clone()),
        info(),
        Arc::new(Stats::new()),
        shutdown,
    )
    .unwrap();
    let (reply, firmware) = oneshot::channel();
    tx.send(HidCommand::Firmware { reply }).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !script.entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (reply, control) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![1, 135],
        reply,
    })
    .await
    .unwrap();
    stop.send(true).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(!task.is_finished());
    *script.gate.0.lock().unwrap() = true;
    script.gate.1.notify_all();
    assert!(firmware.await.unwrap().unwrap().datetime.is_some());
    task.await.unwrap();
    assert!(control.await.is_err());
    assert_eq!(script.mock.write_history(&info()).len(), 1);
}

#[tokio::test]
async fn firmware_waits_for_pending_sensor_reply_instead_of_consuming_it() {
    let script = script(vec![date(), version()], true);
    let (stop, shutdown) = watch::channel(false);
    let (tx, task) = hid_io::spawn_tracked(
        Arc::new(script.clone()),
        info(),
        Arc::new(Stats::new()),
        shutdown,
    )
    .unwrap();
    let (reply, sensor) = oneshot::channel();
    tx.send(HidCommand::PollSensors { reply }).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while script.mock.write_history(&info()).is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (reply, firmware) = oneshot::channel();
    tx.send(HidCommand::Firmware { reply }).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert_eq!(
        script
            .mock
            .write_history(&info())
            .iter()
            .map(|bytes| bytes[1])
            .collect::<Vec<_>>(),
        vec![129]
    );
    script.mock.queue_response(
        &info(),
        ACommandPacket::new_request(129, &[0, 1, 0, 2, 1, 35, 2])
            .as_bytes()
            .to_vec(),
    );
    let sensor = sensor.await.unwrap().unwrap();
    assert_eq!(sensor[1], 129);
    assert!(firmware.await.unwrap().unwrap().datetime.is_some());
    assert_eq!(
        script
            .mock
            .write_history(&info())
            .iter()
            .map(|bytes| bytes[1])
            .collect::<Vec<_>>(),
        vec![129, 134]
    );
    stop.send(true).unwrap();
    task.await.unwrap();
}
