//! A StatusNotifier watcher/host contract for the isolated desktop harness.
use std::{path::PathBuf, sync::Mutex};
use zbus::{Connection, ConnectionBuilder};
struct Watcher {
    root: PathBuf,
    items: Mutex<Vec<String>>,
}
#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(
        &self,
        service: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<()> {
        let item = if service.starts_with('/') {
            format!(
                "{}{service}",
                header
                    .sender()
                    .ok_or_else(|| zbus::fdo::Error::Failed("Sender required".into()))?
            )
        } else {
            format!("{service}/StatusNotifierItem")
        };
        let mut items = self.items.lock().unwrap();
        if !items.contains(&item) {
            items.push(item);
        }
        std::fs::write(
            self.root.join("tray-items.json"),
            serde_json::to_vec(&*items).unwrap(),
        )
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }
    fn register_status_notifier_host(&self, _service: String) {}
    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }
    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        !self.root.join("no-tray-host").exists()
    }
    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}
pub async fn start(root: PathBuf) -> anyhow::Result<Connection> {
    Ok(ConnectionBuilder::session()?
        .name("org.kde.StatusNotifierWatcher")?
        .serve_at(
            "/StatusNotifierWatcher",
            Watcher {
                root,
                items: Mutex::new(vec![]),
            },
        )?
        .build()
        .await?)
}
