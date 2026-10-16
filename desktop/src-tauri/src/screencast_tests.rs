use super::*;
use std::{
    io::{BufRead, BufReader, Read},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
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
#[derive(Clone, Copy)]
enum Mode {
    Success,
    CancelSelect,
    SilentStart,
    InvalidStreams,
    MissingSerial,
    RemoteFailure,
}
#[derive(Default)]
struct Observed {
    calls: Vec<&'static str>,
    request_closes: usize,
    session_closes: usize,
    session_path: String,
}
type Shared = Arc<Mutex<Observed>>;
struct Request(Shared);
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl Request {
    fn close(&self) {
        self.0.lock().unwrap().request_closes += 1;
    }
}
struct SessionFixture(Shared);
#[zbus::interface(name = "org.freedesktop.portal.Session")]
impl SessionFixture {
    fn close(&self) {
        self.0.lock().unwrap().session_closes += 1;
    }
}
struct Portal {
    mode: Mode,
    observed: Shared,
}
impl Portal {
    async fn respond(
        &self,
        connection: &Connection,
        header: &zbus::message::Header<'_>,
        options: &Results,
        code: u32,
        results: HashMap<&str, Value<'_>>,
        silent: bool,
    ) -> OwnedObjectPath {
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let path = OwnedObjectPath::try_from(format!("{ROOT}/request/{sender}/{token}")).unwrap();
        connection
            .object_server()
            .at(path.clone(), Request(self.observed.clone()))
            .await
            .unwrap();
        if !silent {
            connection
                .emit_signal(
                    None::<&str>,
                    path.clone(),
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(code, results),
                )
                .await
                .unwrap();
        }
        path
    }
}
#[zbus::interface(name = "org.freedesktop.portal.ScreenCast")]
impl Portal {
    #[zbus(property)]
    fn available_source_types(&self) -> u32 {
        1
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        6
    }
    async fn create_session(
        &self,
        options: Results,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> OwnedObjectPath {
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = <&str>::try_from(options.get("session_handle_token").unwrap()).unwrap();
        let path = format!("{ROOT}/session/{sender}/{token}");
        {
            let mut s = self.observed.lock().unwrap();
            s.calls.push("create");
            s.session_path = path.clone();
        }
        connection
            .object_server()
            .at(path.clone(), SessionFixture(self.observed.clone()))
            .await
            .unwrap();
        self.respond(
            connection,
            &header,
            &options,
            0,
            HashMap::from([("session_handle", Value::from(path))]),
            false,
        )
        .await
    }
    async fn select_sources(
        &self,
        session: OwnedObjectPath,
        options: Results,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> OwnedObjectPath {
        assert_eq!(session.as_str(), self.observed.lock().unwrap().session_path);
        assert_eq!(u32::try_from(options.get("types").unwrap()).unwrap(), 1);
        assert!(!bool::try_from(options.get("multiple").unwrap()).unwrap());
        assert!(!options.contains_key("persist_mode") && !options.contains_key("restore_token"));
        self.observed.lock().unwrap().calls.push("select");
        self.respond(
            connection,
            &header,
            &options,
            if matches!(self.mode, Mode::CancelSelect) {
                1
            } else {
                0
            },
            HashMap::new(),
            false,
        )
        .await
    }
    async fn start(
        &self,
        session: OwnedObjectPath,
        parent: &str,
        options: Results,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> OwnedObjectPath {
        assert!(parent.is_empty());
        assert_eq!(session.as_str(), self.observed.lock().unwrap().session_path);
        self.observed.lock().unwrap().calls.push("start");
        let properties: HashMap<&str, Value<'_>> = if matches!(self.mode, Mode::MissingSerial) {
            HashMap::new()
        } else {
            HashMap::from([("pipewire-serial", Value::from(9001u64))])
        };
        let streams = if matches!(self.mode, Mode::InvalidStreams) {
            vec![]
        } else {
            vec![(17u32, properties)]
        };
        self.respond(
            connection,
            &header,
            &options,
            0,
            HashMap::from([("streams", Value::from(streams))]),
            matches!(self.mode, Mode::SilentStart),
        )
        .await
    }
    async fn open_pipe_wire_remote(
        &self,
        session: OwnedObjectPath,
        _options: Results,
    ) -> zbus::fdo::Result<OwnedFd> {
        assert_eq!(session.as_str(), self.observed.lock().unwrap().session_path);
        self.observed.lock().unwrap().calls.push("remote");
        if matches!(self.mode, Mode::RemoteFailure) {
            return Err(zbus::fdo::Error::Failed("fixture remote failure".into()));
        }
        Ok(OwnedFd::from(std::os::fd::OwnedFd::from(
            std::fs::File::open("/dev/null").unwrap(),
        )))
    }
}
async fn fixture(mode: Mode) -> (Bus, Connection, Connection, Shared) {
    let bus = Bus::start();
    let server = bus.connect().await;
    let observed = Shared::default();
    server
        .object_server()
        .at(
            ROOT,
            Portal {
                mode,
                observed: observed.clone(),
            },
        )
        .await
        .unwrap();
    server.request_name(DEST).await.unwrap();
    let client = bus.connect().await;
    (bus, server, client, observed)
}
#[tokio::test]
async fn negotiates_one_advertised_source_fd_and_remote_close_signal() {
    let (_bus, server, client, observed) = fixture(Mode::Success).await;
    let (_tx, cancel) = watch::channel(false);
    let mut session = open(client, cancel).await.unwrap().unwrap();
    assert_eq!(session.node_id, 17);
    assert_eq!(session.serial, Some(9001));
    let mut fd = std::fs::File::from(session.remote_fd().unwrap().try_clone_to_owned().unwrap());
    assert_eq!(fd.read(&mut [0u8; 1]).unwrap(), 0);
    drop(fd);
    assert_eq!(
        observed.lock().unwrap().calls,
        vec!["create", "select", "start", "remote"]
    );
    let path = observed.lock().unwrap().session_path.clone();
    server
        .emit_signal(
            None::<&str>,
            path,
            "org.freedesktop.portal.Session",
            "Closed",
            &(HashMap::<&str, Value<'_>>::new(),),
        )
        .await
        .unwrap();
    let mut closed = session.closed();
    tokio::time::timeout(Duration::from_secs(2), closed.wait_for(|v| *v))
        .await
        .unwrap()
        .unwrap();
    session.close().await.unwrap();
    assert!(session.remote_fd().is_none());
    assert_eq!(observed.lock().unwrap().session_closes, 1);
}
#[tokio::test]
async fn cancelled_and_invalid_negotiation_close_session_without_leaking_remote() {
    for mode in [
        Mode::CancelSelect,
        Mode::InvalidStreams,
        Mode::MissingSerial,
        Mode::RemoteFailure,
    ] {
        let (_bus, _server, client, observed) = fixture(mode).await;
        let (_tx, cancel) = watch::channel(false);
        let result = open(client, cancel).await;
        if matches!(mode, Mode::CancelSelect) {
            assert!(result.unwrap().is_none());
        } else {
            assert!(result.is_err());
        }
        assert_eq!(observed.lock().unwrap().session_closes, 1);
        if !matches!(mode, Mode::RemoteFailure) {
            assert!(!observed.lock().unwrap().calls.contains(&"remote"));
        }
    }
}
#[tokio::test]
async fn cancellation_during_start_closes_request_and_session() {
    let (_bus, _server, client, observed) = fixture(Mode::SilentStart).await;
    let (tx, cancel) = watch::channel(false);
    let operation = tokio::spawn(open(client, cancel));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !observed.lock().unwrap().calls.contains(&"start") {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    tx.send(true).unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .is_none());
    assert_eq!(observed.lock().unwrap().session_closes, 1);
    assert_eq!(observed.lock().unwrap().request_closes, 1);
}

#[tokio::test]
async fn portal_session_closed_interrupts_pending_selection() {
    let (_bus, server, client, observed) = fixture(Mode::SilentStart).await;
    let (_tx, cancel) = watch::channel(false);
    let operation = tokio::spawn(open(client, cancel));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !observed.lock().unwrap().calls.contains(&"start") {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let path = observed.lock().unwrap().session_path.clone();
    server
        .emit_signal(
            None::<&str>,
            path,
            "org.freedesktop.portal.Session",
            "Closed",
            &(HashMap::<&str, Value<'_>>::new(),),
        )
        .await
        .unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert_eq!(observed.lock().unwrap().request_closes, 1);
    assert_eq!(observed.lock().unwrap().session_closes, 1);
    assert!(!observed.lock().unwrap().calls.contains(&"remote"));
}
