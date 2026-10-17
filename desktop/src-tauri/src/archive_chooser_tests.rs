use super::*;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
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
struct Fixture {
    uris: Vec<String>,
    silent: bool,
    closed: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
    expected_title: Option<&'static str>,
    expected_filter: Option<&'static str>,
}
struct RequestFixture(Arc<AtomicBool>);
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl RequestFixture {
    fn close(&self) {
        self.0.store(true, Ordering::Release);
    }
}
impl Fixture {
    async fn respond(
        &self,
        save: bool,
        parent: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
        connection: &Connection,
        header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(parent.is_empty());
        if let Some(expected) = self.expected_title {
            assert_eq!(title, expected);
        }
        if let Some(expected) = self.expected_filter {
            let filters: Vec<(String, Vec<(u32, String)>)> = options
                .get("filters")
                .unwrap()
                .try_clone()
                .unwrap()
                .try_into()
                .unwrap();
            assert_eq!(filters[0].0, expected);
        }
        assert!(options.contains_key("filters"));
        if save {
            let name = <&str>::try_from(options.get("current_name").unwrap()).unwrap();
            assert!(!name.contains('/'));
        } else {
            assert!(!bool::try_from(options.get("multiple").unwrap()).unwrap());
        }
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
        self.started.store(true, Ordering::Release);
        if !self.silent {
            let values = HashMap::from([("uris", Value::from(self.uris.clone()))]);
            connection
                .emit_signal(
                    None::<&str>,
                    path.clone(),
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(0u32, values),
                )
                .await
                .unwrap();
        }
        Ok(path)
    }
}
#[zbus::interface(name = "org.freedesktop.portal.FileChooser")]
impl Fixture {
    async fn open_file(
        &self,
        parent: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.respond(false, parent, title, options, connection, header)
            .await
    }
    async fn save_file(
        &self,
        parent: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.respond(true, parent, title, options, connection, header)
            .await
    }
}
#[tokio::test]
async fn portal_receives_localized_profile_and_group_titles_and_filters_without_changing_the_grant()
{
    for (language, group, save, title, filter) in [
        (
            InterfaceLanguage::English,
            false,
            true,
            "Export L-Connect Linux profile",
            "L-Connect Linux profile",
        ),
        (
            InterfaceLanguage::English,
            false,
            false,
            "Import L-Connect Linux profile",
            "L-Connect Linux profile",
        ),
        (
            InterfaceLanguage::English,
            true,
            true,
            "Export L-Connect Linux settings group",
            "L-Connect Linux settings group",
        ),
        (
            InterfaceLanguage::English,
            true,
            false,
            "Import L-Connect Linux settings group",
            "L-Connect Linux settings group",
        ),
        (
            InterfaceLanguage::Russian,
            false,
            true,
            "Экспорт профиля L-Connect Linux",
            "Профиль L-Connect Linux",
        ),
        (
            InterfaceLanguage::Russian,
            false,
            false,
            "Импорт профиля L-Connect Linux",
            "Профиль L-Connect Linux",
        ),
        (
            InterfaceLanguage::Russian,
            true,
            true,
            "Экспорт группы настроек L-Connect Linux",
            "Группа настроек L-Connect Linux",
        ),
        (
            InterfaceLanguage::Russian,
            true,
            false,
            "Импорт группы настроек L-Connect Linux",
            "Группа настроек L-Connect Linux",
        ),
        (
            InterfaceLanguage::English,
            true,
            false,
            "Import Windows L-Connect3 settings",
            "Windows L-Connect3 ZIP",
        ),
        (
            InterfaceLanguage::Russian,
            true,
            false,
            "Импорт настроек Windows L-Connect3",
            "Windows L-Connect3 ZIP",
        ),
    ] {
        let bus = Bus::start();
        let service = bus.connect().await;
        service
            .object_server()
            .at(
                "/org/freedesktop/portal/desktop",
                Fixture {
                    uris: vec!["file:///tmp/kept%20archive.lcp".into()],
                    silent: false,
                    closed: Arc::new(AtomicBool::new(false)),
                    started: Arc::new(AtomicBool::new(false)),
                    expected_title: Some(title),
                    expected_filter: Some(filter),
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.freedesktop.portal.Desktop")
            .await
            .unwrap();
        let (_signal, cancel) = watch::channel(false);
        let grant = choose_filtered(
            &bus.connect().await,
            "Настройки {1}.lcp",
            cancel,
            Duration::from_secs(2),
            Dialog {
                save,
                group,
                windows: title.contains("Windows"),
                language,
            },
        )
        .await
        .unwrap();
        assert_eq!(grant, Some(PathBuf::from("/tmp/kept archive.lcp")));
    }
}

#[test]
fn local_grant_errors_use_the_dialog_language_and_valid_names_are_not_translated() {
    for (language, expected) in [
        (
            InterfaceLanguage::English,
            "File chooser must return a local file URI",
        ),
        (
            InterfaceLanguage::Russian,
            "Диалог должен вернуть локальный файл архива",
        ),
    ] {
        let dialog = Dialog {
            save: false,
            group: false,
            windows: false,
            language,
        };
        for uri in [
            "https://example.com/archive.lcp",
            "file:///tmp/a?query",
            "file:///tmp/a#fragment",
        ] {
            assert_eq!(local_path(uri, dialog).unwrap_err().to_string(), expected);
        }
        assert_eq!(
            local_path("file://user:secret@localhost/tmp/a", dialog)
                .unwrap_err()
                .to_string(),
            if language == InterfaceLanguage::English {
                "Invalid archive file URI"
            } else {
                "Некорректный URI файла архива"
            }
        );
        assert_eq!(
            local_path("file:///tmp/Настройки%20%7B1%7D.lcp", dialog).unwrap(),
            PathBuf::from("/tmp/Настройки {1}.lcp")
        );
    }
}

#[tokio::test]
async fn open_and_save_accept_early_responses_and_reject_nonlocal_or_multiple_paths() {
    for (save, uris, success) in [
        (false, vec!["file:///tmp/open%20archive.lcp".into()], true),
        (true, vec!["file:///tmp/save.lcp".into()], true),
        (false, vec!["https://example.com/a".into()], false),
        (
            true,
            vec!["file:///tmp/a".into(), "file:///tmp/b".into()],
            false,
        ),
        (false, vec!["file:///tmp/a?query".into()], false),
    ] {
        let bus = Bus::start();
        let service = bus.connect().await;
        let closed = Arc::new(AtomicBool::new(false));
        let started = Arc::new(AtomicBool::new(false));
        service
            .object_server()
            .at(
                "/org/freedesktop/portal/desktop",
                Fixture {
                    uris,
                    silent: false,
                    closed: closed.clone(),
                    started,
                    expected_title: None,
                    expected_filter: None,
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.freedesktop.portal.Desktop")
            .await
            .unwrap();
        let client = bus.connect().await;
        let (_signal, cancel) = watch::channel(false);
        let result = choose_filtered(
            &client,
            "nested/name.lcp",
            cancel,
            Duration::from_secs(2),
            Dialog {
                save,
                group: false,
                windows: false,
                language: InterfaceLanguage::English,
            },
        )
        .await;
        assert_eq!(result.is_ok(), success, "{result:?}");
        if !success {
            assert!(closed.load(Ordering::Acquire));
        }
    }
}
#[tokio::test]
async fn cancellation_closes_a_silent_dialog_without_selecting_a_file() {
    let bus = Bus::start();
    let service = bus.connect().await;
    let closed = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicBool::new(false));
    service
        .object_server()
        .at(
            "/org/freedesktop/portal/desktop",
            Fixture {
                uris: vec![],
                silent: true,
                closed: closed.clone(),
                started: started.clone(),
                expected_title: None,
                expected_filter: None,
            },
        )
        .await
        .unwrap();
    service
        .request_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
    let client = bus.connect().await;
    let (signal, cancel) = watch::channel(false);
    let task = tokio::spawn(async move {
        choose_filtered(
            &client,
            "archive.lcp",
            cancel,
            Duration::from_secs(3),
            Dialog {
                save: false,
                group: false,
                windows: false,
                language: InterfaceLanguage::English,
            },
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !started.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    signal.send(true).unwrap();
    assert!(task.await.unwrap().unwrap().is_none());
    assert!(closed.load(Ordering::Acquire));
}
