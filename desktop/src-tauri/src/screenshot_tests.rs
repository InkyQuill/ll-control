use super::*;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};

struct Bus {
    child: Child,
    address: String,
}
impl Bus {
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
            address: address.trim().into(),
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
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
struct Portal {
    code: u32,
    uri: String,
    silent: bool,
    closed: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
}
struct RequestFixture(Arc<AtomicBool>);
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl RequestFixture {
    fn close(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl Portal {
    async fn screenshot(
        &self,
        parent: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(parent.is_empty());
        assert!(bool::try_from(options.get("interactive").unwrap()).unwrap());
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let path = OwnedObjectPath::try_from(format!(
            "/org/freedesktop/portal/desktop/request/{sender}/{token}"
        ))
        .unwrap();
        connection
            .object_server()
            .at(path.clone(), RequestFixture(self.closed.clone()))
            .await
            .unwrap();
        self.started.store(true, Ordering::SeqCst);
        if !self.silent {
            let values = HashMap::from([("uri", Value::from(self.uri.as_str()))]);
            // Emit before the method reply to exercise the real response race.
            connection
                .emit_signal(
                    None::<&str>,
                    path.clone(),
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(self.code, values),
                )
                .await
                .unwrap();
        }
        Ok(path)
    }
}
#[tokio::test]
async fn private_portal_success_cancel_rejection_and_timeout_close() {
    for (code, silent) in [(0, false), (1, false), (2, false), (0, true)] {
        let bus = Bus::start();
        let server = bus.connect().await;
        let closed = Arc::new(AtomicBool::new(false));
        server
            .object_server()
            .at(
                "/org/freedesktop/portal/desktop",
                Portal {
                    code,
                    uri: "file:///tmp/test%20capture.png".into(),
                    silent,
                    closed: closed.clone(),
                    started: Arc::new(AtomicBool::new(false)),
                },
            )
            .await
            .unwrap();
        server
            .request_name("org.freedesktop.portal.Desktop")
            .await
            .unwrap();
        let client = bus.connect().await;
        let (_cancel, receiver) = watch::channel(false);
        let result = request(&client, Duration::from_millis(200), receiver).await;
        match (code, silent) {
            (0, false) => assert_eq!(
                result.unwrap().as_deref(),
                Some("file:///tmp/test%20capture.png")
            ),
            (1, false) => assert!(result.unwrap().is_none()),
            _ => {
                assert!(result.is_err());
                assert!(closed.load(Ordering::SeqCst));
            }
        }
    }
}
#[test]
fn valid_screenshot_bytes_are_preserved_and_portal_file_is_not_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("capture space.png");
    let png = include_bytes!("../../../crates/lconnect-image/assets/defaults/midnight.png");
    std::fs::write(&path, png).unwrap();
    assert_eq!(
        read_image(url::Url::from_file_path(&path).unwrap().as_str()).unwrap(),
        png
    );
    assert!(path.exists());
}

#[tokio::test]
async fn cancel_before_capture_revokes_reservation_without_connecting() {
    let owner = Screenshots::default();
    let id = owner.begin().await.unwrap();
    assert!(owner.begin().await.is_err());
    owner.cancel("unrelated").await;
    assert!(owner.begin().await.is_err());
    owner.cancel(&id).await;
    let connected = Arc::new(AtomicBool::new(false));
    let flag = connected.clone();
    assert!(owner
        .capture_with(&id, async move {
            flag.store(true, Ordering::SeqCst);
            Err(zbus::Error::Failure("must not connect".into()))
        })
        .await
        .is_err());
    assert!(!connected.load(Ordering::SeqCst));
    let next = owner.begin().await.unwrap();
    owner.cancel(&id).await;
    assert!(owner.begin().await.is_err());
    owner.cancel(&next).await;
    owner.shutdown().await.unwrap();
    assert!(owner.begin().await.is_err());
}

#[tokio::test]
async fn cancel_and_shutdown_join_portal_even_after_invoke_is_dropped() {
    for shutdown in [false, true] {
        let bus = Bus::start();
        let server = bus.connect().await;
        let closed = Arc::new(AtomicBool::new(false));
        let started = Arc::new(AtomicBool::new(false));
        server
            .object_server()
            .at(
                "/org/freedesktop/portal/desktop",
                Portal {
                    code: 0,
                    uri: String::new(),
                    silent: true,
                    closed: closed.clone(),
                    started: started.clone(),
                },
            )
            .await
            .unwrap();
        server
            .request_name("org.freedesktop.portal.Desktop")
            .await
            .unwrap();
        let client = bus.connect().await;
        let owner = Screenshots::default();
        let id = owner.begin().await.unwrap();
        let worker = owner.clone();
        let capture_id = id.clone();
        let invoke = tokio::spawn(async move {
            worker
                .capture_with(&capture_id, async move { Ok(client) })
                .await
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !started.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        assert!(owner.begin().await.is_err());
        invoke.abort();
        let _ = invoke.await;
        tokio::time::timeout(Duration::from_secs(2), async {
            if shutdown {
                owner.shutdown().await.unwrap();
            } else {
                owner.cancel(&id).await;
            }
        })
        .await
        .unwrap();
        assert!(closed.load(Ordering::SeqCst));
        assert!(owner.0.lock().await.active.is_none());
        if shutdown {
            assert!(owner.begin().await.is_err());
        } else {
            let next = owner.begin().await.unwrap();
            owner.cancel(&next).await;
        }
    }
}
