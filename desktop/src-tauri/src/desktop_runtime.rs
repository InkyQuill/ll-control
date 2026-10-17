//! Tauri desktop lifecycle around durable preferences. No device/daemon writer.
use crate::desktop_settings::{Preferences, Readback, Store};
use anyhow::{Context, Result};
use lconnect_ipc::InterfaceLanguage;
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
const TRAY_ID: &str = "lconnect-main";
const JOBS: u32 = 2;
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayErrorCode {
    HostUnconfirmed,
    Unavailable,
}
#[derive(Clone)]
enum TrayIssue {
    HostUnconfirmed,
    Unavailable(String),
}
const LEGACY_HOST_ERROR: &str = "В сессии нет зарегистрированного tray host для приложения.";
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(flatten)]
    pub settings: Readback,
    pub tray_ready: bool,
    pub tray_error: Option<String>,
    pub tray_error_code: Option<TrayErrorCode>,
}
type TrayMenu = (MenuItem<tauri::Wry>, MenuItem<tauri::Wry>);
#[derive(Default)]
struct NativeMenu {
    language: InterfaceLanguage,
    items: Option<TrayMenu>,
}
fn menu_labels(language: InterfaceLanguage) -> (&'static str, &'static str) {
    match language {
        InterfaceLanguage::English => ("Show L-Connect Linux", "Exit application"),
        InterfaceLanguage::Russian => ("Показать L-Connect Linux", "Выйти из приложения"),
    }
}
fn apply_menu_language(items: &TrayMenu, language: InterfaceLanguage) -> Result<()> {
    let (show, exit) = menu_labels(language);
    items.0.set_text(show)?;
    items.1.set_text(exit)?;
    Ok(())
}
pub struct Desktop {
    store: Store,
    menu: Mutex<NativeMenu>,
    preferences: Mutex<Option<Preferences>>,
    ready: AtomicBool,
    tray_error: Mutex<Option<TrayIssue>>,
    closed: AtomicBool,
    jobs: Arc<tokio::sync::Semaphore>,
}
impl Desktop {
    /// UI labels only; no settings persistence, device access or lifecycle action.
    pub fn set_language(&self, language: InterfaceLanguage) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let items = {
            let mut menu = self
                .menu
                .lock()
                .map_err(|_| anyhow::anyhow!("Native menu state unavailable"))?;
            menu.language = language;
            menu.items.clone()
        };
        // GTK operations may dispatch to the main thread. Never hold our lock there.
        if let Some(items) = items {
            apply_menu_language(&items, language)?;
        }
        Ok(())
    }

    fn cache(&self, p: Option<Preferences>) {
        if let Ok(mut value) = self.preferences.lock() {
            *value = p;
        }
    }
    fn preferences(&self) -> Option<Preferences> {
        self.preferences.lock().ok().and_then(|p| p.clone())
    }
    pub fn hide_on_close(&self) -> bool {
        self.ready.load(Ordering::Acquire) && self.preferences().is_some_and(|p| p.close_to_tray)
    }
    fn snapshot(&self, settings: Readback) -> Snapshot {
        let tray_ready = self.ready.load(Ordering::Acquire);
        let issue = self.tray_error.lock().ok().and_then(|e| e.clone());
        let (tray_error, tray_error_code) = match issue {
            Some(TrayIssue::HostUnconfirmed) if tray_ready => (None, None),
            Some(TrayIssue::HostUnconfirmed) => (
                Some(LEGACY_HOST_ERROR.to_owned()),
                Some(TrayErrorCode::HostUnconfirmed),
            ),
            Some(TrayIssue::Unavailable(diagnostic)) => {
                (Some(diagnostic), Some(TrayErrorCode::Unavailable))
            }
            None => (None, None),
        };
        Snapshot {
            settings,
            tray_ready,
            tray_error,
            tray_error_code,
        }
    }
    pub async fn read(self: Arc<Self>) -> Result<Snapshot> {
        self.work(None).await
    }
    pub async fn save(self: Arc<Self>, expected: u64, prefs: Preferences) -> Result<Snapshot> {
        self.work(Some((expected, prefs))).await
    }
    async fn work(self: Arc<Self>, update: Option<(u64, Preferences)>) -> Result<Snapshot> {
        anyhow::ensure!(!self.closed.load(Ordering::Acquire), "Desktop is closing");
        let permit = self
            .jobs
            .clone()
            .try_acquire_owned()
            .context("Desktop settings busy")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            anyhow::ensure!(!self.closed.load(Ordering::Acquire), "Desktop is closing");
            let result = match update {
                Some((expected, prefs)) => self.store.apply(expected, prefs),
                None => self.store.read(),
            };
            match result {
                Ok(read) => {
                    self.cache(Some(read.preferences.clone()));
                    Ok(self.snapshot(read))
                }
                Err(e) => {
                    self.cache(None);
                    Err(e)
                }
            }
        })
        .await?
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.closed.store(true, Ordering::Release);
        let _drained = self.jobs.clone().acquire_many_owned(JOBS).await?;
        // Menu handles own an AppHandle. Release them before application teardown,
        // outside our lock because their drop can dispatch GTK work.
        let items = self
            .menu
            .lock()
            .map_err(|_| anyhow::anyhow!("Native menu state unavailable"))?
            .items
            .take();
        drop(items);
        Ok(())
    }
}
pub fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
pub fn setup(app: &mut tauri::App, on_exit: fn(&tauri::AppHandle)) -> Result<()> {
    let dirs = std::env::var_os("XDG_CONFIG_DIRS")
        .map(|s| {
            std::env::split_paths(&s)
                .filter(|p| p.is_absolute())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["/etc/xdg".into()]);
    let store = Store::new(
        app.path().app_config_dir()?,
        app.path().config_dir()?,
        std::env::current_exe()?,
    )?
    .with_config_dirs(dirs)?;
    let preferences = match store.read() {
        Ok(read) => Some(read.preferences),
        Err(e) => {
            eprintln!("Desktop preferences unavailable: {e}");
            None
        }
    };
    let minimize = preferences.as_ref().is_some_and(|p| p.start_minimized);
    let desktop = Arc::new(Desktop {
        store,
        menu: Mutex::new(NativeMenu::default()),
        preferences: Mutex::new(preferences),
        ready: AtomicBool::new(false),
        tray_error: Mutex::new(None),
        closed: AtomicBool::new(false),
        jobs: Arc::new(tokio::sync::Semaphore::new(JOBS as usize)),
    });
    app.manage(desktop.clone());
    // Some Linux indicator backends panic when their optional runtime library is
    // unavailable. Keep the window usable; packaging must still supply that lib.
    let tray = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        let show_item = MenuItem::with_id(
            app,
            "desktop-show",
            menu_labels(InterfaceLanguage::default()).0,
            true,
            None::<&str>,
        )?;
        let exit_item = MenuItem::with_id(
            app,
            "desktop-exit",
            menu_labels(InterfaceLanguage::default()).1,
            true,
            None::<&str>,
        )?;
        let menu = Menu::with_items(app, &[&show_item, &exit_item])?;
        let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
        TrayIconBuilder::with_id(TRAY_ID)
            .icon(icon)
            .tooltip("L-Connect Linux")
            .menu(&menu)
            .on_menu_event(move |app, event| match event.id.as_ref() {
                "desktop-show" => show(app),
                "desktop-exit" => on_exit(app),
                _ => {}
            })
            .build(app)?;
        let language = {
            let mut menu = desktop
                .menu
                .lock()
                .map_err(|_| anyhow::anyhow!("Native menu state unavailable"))?;
            menu.items = Some((show_item.clone(), exit_item.clone()));
            menu.language
        };
        apply_menu_language(&(show_item, exit_item), language)?;
        Ok(())
    }))
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Tray indicator library unavailable")));
    if let Err(e) = tray {
        if let Ok(mut error) = desktop.tray_error.lock() {
            *error = Some(TrayIssue::Unavailable(e.to_string()));
        }
        return Ok(());
    }
    if minimize {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        let mut first = true;
        let startup_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            let ready = tokio::time::timeout(Duration::from_secs(2), registered_item())
                .await
                .unwrap_or(Ok(false))
                .unwrap_or(false);
            if first && minimize && !ready && tokio::time::Instant::now() < startup_deadline {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            let was_ready = desktop.ready.swap(ready, Ordering::AcqRel);
            #[cfg(feature = "native-smoke")]
            if first || was_ready != ready {
                eprintln!("native desktop: tray-ready {ready}");
            }

            if let Ok(mut error) = desktop.tray_error.lock() {
                *error = (!ready).then_some(TrayIssue::HostUnconfirmed);
            }
            if !ready && (first && minimize || was_ready) {
                show(&handle);
            }
            first = false;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    Ok(())
}
async fn registered_item() -> Result<bool> {
    let connection = zbus::Connection::session().await?;
    let watcher = zbus::Proxy::new(
        &connection,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )
    .await?;
    if !watcher
        .get_property::<bool>("IsStatusNotifierHostRegistered")
        .await?
    {
        return Ok(false);
    }
    let items = watcher
        .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
        .await?;
    anyhow::ensure!(items.len() <= 128, "Tray item inventory exceeds budget");
    let bus = zbus::fdo::DBusProxy::new(&connection).await?;
    for item in items {
        let (service, path) = item
            .split_once('/')
            .map(|(s, p)| (s, format!("/{p}")))
            .unwrap_or((&item, "/StatusNotifierItem".into()));
        let Ok(name) = zbus::names::BusName::try_from(service) else {
            continue;
        };
        if bus.get_connection_unix_process_id(name).await.ok() != Some(std::process::id()) {
            continue;
        }
        let proxy = zbus::Proxy::new(
            &connection,
            service,
            path.as_str(),
            "org.kde.StatusNotifierItem",
        )
        .await?;
        let id = proxy.get_property::<String>("Id").await?;
        if id == format!("tray-icon tray app {TRAY_ID}") {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
