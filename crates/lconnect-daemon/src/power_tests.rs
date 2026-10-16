use super::*;
use crate::{config::Config, manager::DeviceManager, persistence::PersistenceStore, stats::Stats};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use std::{
    io::{BufRead, BufReader, Read},
    os::unix::net::UnixStream,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

struct PrivateBus {
    child: Child,
    address: String,
}
impl PrivateBus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            child,
            address: address.trim().to_owned(),
        }
    }
    async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}
impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone, Default)]
struct FakeLogind {
    sleeping: Arc<AtomicBool>,
    denied: Arc<AtomicBool>,
    attempts: Arc<AtomicUsize>,
    delay_us: Arc<AtomicU64>,
    pause_read: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
    read_started: Arc<tokio::sync::Notify>,
    read_release: Arc<tokio::sync::Notify>,
    peers: Arc<Mutex<Vec<UnixStream>>>,
}
#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl FakeLogind {
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::fdo::Result<OwnedFd> {
        assert_eq!((what, who, mode), ("sleep", "lconnectd", "delay"));
        assert!(!why.is_empty());
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if self.denied.load(Ordering::SeqCst) {
            return Err(zbus::fdo::Error::AccessDenied("fixture denial".into()));
        }
        let (fd, peer) = UnixStream::pair().unwrap();
        peer.set_nonblocking(true).unwrap();
        self.peers.lock().unwrap().push(peer);
        Ok(std::os::fd::OwnedFd::from(fd).into())
    }
    #[zbus(property)]
    async fn preparing_for_sleep(&self) -> bool {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.pause_read.swap(false, Ordering::SeqCst) {
            self.read_started.notify_one();
            self.read_release.notified().await;
        }
        self.sleeping.load(Ordering::SeqCst)
    }
    #[zbus(property, name = "InhibitDelayMaxUSec")]
    fn inhibit_delay_max_usec(&self) -> u64 {
        let delay = self.delay_us.load(Ordering::SeqCst);
        if delay == 0 {
            5_000_000
        } else {
            delay
        }
    }
}
impl FakeLogind {
    async fn emit(&self, service: &Connection, sleeping: bool) {
        self.sleeping.store(sleeping, Ordering::SeqCst);
        service
            .emit_signal(
                None::<&str>,
                PATH,
                INTERFACE,
                "PrepareForSleep",
                &(sleeping,),
            )
            .await
            .unwrap();
    }
    fn released(&self, index: usize) -> bool {
        let mut guard = self.peers.lock().unwrap();
        matches!(guard[index].read(&mut [0]), Ok(0))
    }
}

async fn wait_for<F: std::future::Future<Output = bool>>(mut check: impl FnMut() -> F) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("fixture condition");
}

async fn fixture_manager() -> (ManagerHandle, tempfile::TempDir) {
    let mock = MockBackend::new();
    mock.add_device(DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("power-fixture".into()),
        "/mock/power".into(),
    ));
    let tmp = tempfile::tempdir().unwrap();
    let manager = DeviceManager::new(
        Arc::new(mock),
        Arc::new(PersistenceStore::new_with_root(tmp.path().into())),
        Config::default(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    wait_for(|| async { !manager.list().await.is_empty() }).await;
    (manager, tmp)
}

#[tokio::test]
async fn private_bus_sleep_wake_releases_and_reacquires_delay_inhibitor() {
    let bus = PrivateBus::start();
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    let client = bus.connect().await;
    let (manager, _tmp) = fixture_manager().await;
    let old = manager.list().await[0].id.clone();
    let handles = manager
        .with_device(&old, |d| d.handles.clone())
        .await
        .unwrap();
    let monitor_manager = manager.clone();
    let task =
        tokio::spawn(async move { monitor_connection(&client, &monitor_manager, false).await });
    wait_for(|| async { !fake.peers.lock().unwrap().is_empty() }).await;
    assert!(!fake.released(0));
    fake.emit(&service, true).await;
    wait_for(|| async { fake.released(0) }).await;
    assert!(manager.list().await.is_empty());
    assert!(*handles.teardown_rx.borrow());
    fake.emit(&service, false).await;
    wait_for(|| async { !manager.list().await.is_empty() }).await;
    assert_ne!(manager.list().await[0].id, old);
    assert_eq!(fake.peers.lock().unwrap().len(), 2);
    assert!(!fake.released(1));
    task.abort();
    let _ = task.await;
    wait_for(|| async { fake.released(1) }).await;
    manager.shutdown().await;
}

#[tokio::test]
async fn initial_sleep_and_denied_inhibitor_still_pause_and_recover() {
    let bus = PrivateBus::start();
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    fake.sleeping.store(true, Ordering::SeqCst);
    fake.denied.store(true, Ordering::SeqCst);
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    let client = bus.connect().await;
    let (manager, _tmp) = fixture_manager().await;
    let monitor_manager = manager.clone();
    let task =
        tokio::spawn(async move { monitor_connection(&client, &monitor_manager, false).await });
    wait_for(|| async { manager.list().await.is_empty() }).await;
    assert!(fake.attempts.load(Ordering::SeqCst) > 0);
    assert!(fake.peers.lock().unwrap().is_empty());
    fake.denied.store(false, Ordering::SeqCst);
    fake.emit(&service, false).await;
    wait_for(|| async { !manager.list().await.is_empty() }).await;
    assert_eq!(fake.peers.lock().unwrap().len(), 1);
    task.abort();
    let _ = task.await;
    manager.shutdown().await;
}

#[tokio::test]
async fn owner_loss_ends_monitor_and_reconnection_reconciles_current_state() {
    let bus = PrivateBus::start();
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    let client = bus.connect().await;
    let (manager, _tmp) = fixture_manager().await;
    let monitor_manager = manager.clone();
    let task =
        tokio::spawn(async move { monitor_connection(&client, &monitor_manager, false).await });
    wait_for(|| async { !fake.peers.lock().unwrap().is_empty() }).await;
    fake.emit(&service, true).await;
    wait_for(|| async { manager.list().await.is_empty() }).await;
    service.release_name(SERVICE).await.unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(4), task)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(manager.list().await.is_empty());
    let replacement = bus.connect().await;
    let next_fake = FakeLogind::default();
    replacement
        .object_server()
        .at(PATH, next_fake.clone())
        .await
        .unwrap();
    replacement.request_name(SERVICE).await.unwrap();
    let client = bus.connect().await;
    let monitor_manager = manager.clone();
    let task =
        tokio::spawn(async move { monitor_connection(&client, &monitor_manager, false).await });
    wait_for(|| async { !manager.list().await.is_empty() }).await;
    assert_eq!(next_fake.peers.lock().unwrap().len(), 1);
    task.abort();
    let _ = task.await;
    manager.shutdown().await;
}

struct WriteGate {
    released: Mutex<bool>,
    wake: std::sync::Condvar,
    started: tokio::sync::Notify,
}
impl WriteGate {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
struct ReleaseOnDrop(Arc<WriteGate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
struct GatedBackend {
    mock: MockBackend,
    gate: Arc<WriteGate>,
}
struct GatedHandle {
    inner: Box<dyn lconnect_hid::OpenHandle>,
    gate: Arc<WriteGate>,
}
impl lconnect_hid::HidBackend for GatedBackend {
    fn enumerate(&self) -> lconnect_hid::Result<Vec<DeviceInfo>> {
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> lconnect_hid::Result<Box<dyn lconnect_hid::OpenHandle>> {
        Ok(Box::new(GatedHandle {
            inner: self.mock.open(info)?,
            gate: self.gate.clone(),
        }))
    }
}
impl lconnect_hid::OpenHandle for GatedHandle {
    fn write(&self, bytes: &[u8]) -> lconnect_hid::Result<()> {
        if bytes.get(1) == Some(&14) {
            self.gate.started.notify_one();
            let mut released = self.gate.released.lock().unwrap();
            while !*released {
                released = self.gate.wake.wait(released).unwrap();
            }
        }
        self.inner.write(bytes)
    }
    fn read_timeout(&self, bytes: &mut [u8], ms: i32) -> lconnect_hid::Result<usize> {
        self.inner.read_timeout(bytes, ms)
    }
}

async fn blocked_write_scenario(exceed_deadline: bool) {
    let bus = PrivateBus::start();
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    if exceed_deadline {
        fake.delay_us.store(220_000, Ordering::SeqCst);
    }
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    let client = bus.connect().await;
    let mock = MockBackend::new();
    mock.add_device(DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("blocked-power".into()),
        "/mock/blocked-power".into(),
    ));
    let gate = Arc::new(WriteGate {
        released: Mutex::new(false),
        wake: std::sync::Condvar::new(),
        started: tokio::sync::Notify::new(),
    });
    let _release = ReleaseOnDrop(gate.clone());
    let tmp = tempfile::tempdir().unwrap();
    let manager = DeviceManager::new(
        Arc::new(GatedBackend {
            mock,
            gate: gate.clone(),
        }),
        Arc::new(PersistenceStore::new_with_root(tmp.path().into())),
        Config::default(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    wait_for(|| async { !manager.list().await.is_empty() }).await;
    let old = manager.list().await[0].id.clone();
    let handles = manager
        .with_device(&old, |d| d.handles.clone())
        .await
        .unwrap();
    let (reply, response) = tokio::sync::oneshot::channel();
    handles
        .lcd_tx
        .send(crate::actor::LcdCommand::SetImage {
            jpeg: bytes::Bytes::from_static(&[255, 216, 255, 217]),
            reply,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), gate.started.notified())
        .await
        .unwrap();
    let monitor_manager = manager.clone();
    let task =
        tokio::spawn(async move { monitor_connection(&client, &monitor_manager, false).await });
    wait_for(|| async { !fake.peers.lock().unwrap().is_empty() }).await;
    fake.emit(&service, true).await;
    wait_for(|| async { manager.list().await.is_empty() }).await;
    assert!(!*handles.teardown_rx.borrow());
    if exceed_deadline {
        wait_for(|| async { fake.released(0) }).await;
        assert!(
            !*handles.teardown_rx.borrow(),
            "timeout must not pretend physical I/O finished"
        );
    } else {
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !fake.released(0),
            "inhibitor must cover the blocked physical write"
        );
    }
    gate.release();
    let _ = response.await;
    wait_for(|| async { *handles.teardown_rx.borrow() }).await;
    wait_for(|| async { fake.released(0) }).await;
    fake.emit(&service, false).await;
    wait_for(|| async { !manager.list().await.is_empty() }).await;
    assert_ne!(manager.list().await[0].id, old);
    task.abort();
    let _ = task.await;
    manager.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inhibitor_covers_blocked_hid_write_until_join() {
    blocked_write_scenario(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deadline_releases_inhibitor_but_does_not_cancel_manager_join() {
    blocked_write_scenario(true).await;
}

#[tokio::test]
async fn queued_sleep_wake_is_not_lost_when_property_already_says_awake() {
    let bus = PrivateBus::start();
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    fake.pause_read.store(true, Ordering::SeqCst);
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    let client = bus.connect().await;
    let (manager, _tmp) = fixture_manager().await;
    let old = manager.list().await[0].id.clone();
    let monitor_manager = manager.clone();
    let task =
        tokio::spawn(async move { monitor_connection(&client, &monitor_manager, false).await });
    tokio::time::timeout(Duration::from_secs(3), fake.read_started.notified())
        .await
        .unwrap();
    fake.emit(&service, true).await;
    fake.emit(&service, false).await;
    fake.read_release.notify_one();
    wait_for(|| async { manager.list().await.first().is_some_and(|d| d.id != old) }).await;
    assert_eq!(fake.peers.lock().unwrap().len(), 2);
    assert!(fake.released(0));
    task.abort();
    let _ = task.await;
    manager.shutdown().await;
}

#[tokio::test]
async fn supervisor_recovers_new_owner_and_shutdown_releases_inhibitor() {
    let bus = PrivateBus::start();
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    let (manager, _tmp) = fixture_manager().await;
    let old = manager.list().await[0].id.clone();
    let (stop, shutdown) = watch::channel(false);
    let address = bus.address.clone();
    let monitor_manager = manager.clone();
    let task = tokio::spawn(run(monitor_manager, shutdown, move || {
        let address = address.clone();
        async move {
            zbus::connection::Builder::address(address.as_str())?
                .build()
                .await
        }
    }));
    wait_for(|| async { !fake.peers.lock().unwrap().is_empty() }).await;
    service.release_name(SERVICE).await.unwrap();
    let replacement = bus.connect().await;
    let next = FakeLogind::default();
    replacement
        .object_server()
        .at(PATH, next.clone())
        .await
        .unwrap();
    replacement.request_name(SERVICE).await.unwrap();
    wait_for(|| async { manager.list().await.first().is_some_and(|d| d.id != old) }).await;
    assert!(fake.released(0));
    assert!(!next.peers.lock().unwrap().is_empty());
    let current = next.peers.lock().unwrap().len() - 1;
    assert!(!next.released(current));
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    wait_for(|| async { next.released(current) }).await;
    manager.shutdown().await;
}

#[tokio::test]
async fn missing_logind_at_start_does_not_stop_devices_and_later_recovers() {
    let bus = PrivateBus::start();
    let (manager, _tmp) = fixture_manager().await;
    let old = manager.list().await[0].id.clone();
    let attempts = Arc::new(AtomicUsize::new(0));
    let connect_attempts = attempts.clone();
    let (stop, shutdown) = watch::channel(false);
    let address = bus.address.clone();
    let monitor_manager = manager.clone();
    let task = tokio::spawn(run(monitor_manager, shutdown, move || {
        connect_attempts.fetch_add(1, Ordering::SeqCst);
        let address = address.clone();
        async move {
            zbus::connection::Builder::address(address.as_str())?
                .build()
                .await
        }
    }));
    // At least one complete failed attempt and the retry delay, not a startup race.
    wait_for(|| async { attempts.load(Ordering::SeqCst) >= 2 }).await;
    assert_eq!(manager.list().await[0].id, old);
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    wait_for(|| async { manager.list().await.first().is_some_and(|d| d.id != old) }).await;
    let current = fake.peers.lock().unwrap().len() - 1;
    assert!(!fake.released(current));
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    wait_for(|| async { fake.released(current) }).await;
    manager.shutdown().await;
}

#[tokio::test]
async fn supervisor_reconnects_after_private_bus_process_is_replaced() {
    let mut bus = PrivateBus::start();
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    let (manager, _tmp) = fixture_manager().await;
    let old = manager.list().await[0].id.clone();
    let address = Arc::new(Mutex::new(bus.address.clone()));
    let connect_address = address.clone();
    let (stop, shutdown) = watch::channel(false);
    let monitor_manager = manager.clone();
    let task = tokio::spawn(run(monitor_manager, shutdown, move || {
        let address = connect_address.lock().unwrap().clone();
        async move {
            zbus::connection::Builder::address(address.as_str())?
                .build()
                .await
        }
    }));
    wait_for(|| async { !fake.peers.lock().unwrap().is_empty() }).await;
    fake.emit(&service, true).await;
    wait_for(|| async { manager.list().await.is_empty() }).await;
    let replacement_bus = PrivateBus::start();
    let replacement = replacement_bus.connect().await;
    let next = FakeLogind::default();
    replacement
        .object_server()
        .at(PATH, next.clone())
        .await
        .unwrap();
    replacement.request_name(SERVICE).await.unwrap();
    *address.lock().unwrap() = replacement_bus.address.clone();
    // Kill the owned bus itself, keeping both fake service connections alive.
    bus.child.kill().unwrap();
    bus.child.wait().unwrap();
    wait_for(|| async { manager.list().await.first().is_some_and(|d| d.id != old) }).await;
    assert!(fake.released(0));
    let current = next.peers.lock().unwrap().len() - 1;
    assert!(!next.released(current));
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    wait_for(|| async { next.released(current) }).await;
    manager.shutdown().await;
}

#[tokio::test]
async fn sleep_signal_preempts_a_stalled_health_property_query() {
    let bus = PrivateBus::start();
    let service = bus.connect().await;
    let fake = FakeLogind::default();
    service
        .object_server()
        .at(PATH, fake.clone())
        .await
        .unwrap();
    service.request_name(SERVICE).await.unwrap();
    let client = bus.connect().await;
    let (manager, _tmp) = fixture_manager().await;
    let monitor_manager = manager.clone();
    let task =
        tokio::spawn(async move { monitor_connection(&client, &monitor_manager, false).await });
    // Two completed/started calls imply initialization reached the health loop.
    wait_for(|| async { fake.reads.load(Ordering::SeqCst) >= 2 }).await;
    fake.pause_read.store(true, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(3), fake.read_started.notified())
        .await
        .unwrap();
    fake.emit(&service, true).await;
    // Less than the RPC timeout, and the server is never released until after
    // observing teardown. An implementation awaiting health RPC would fail.
    tokio::time::timeout(Duration::from_secs(1), async {
        while !fake.released(0) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("sleep must preempt stalled health query");
    assert!(manager.list().await.is_empty());
    fake.read_release.notify_one();
    task.abort();
    let _ = task.await;
    manager.shutdown().await;
}
