//! Owned ScreenCast portal negotiation. No display is captured until a caller opens a session.
use anyhow::{ensure, Context};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    future::Future,
    os::fd::{AsFd, BorrowedFd},
    time::Duration,
};
use tokio::{sync::watch, task::JoinHandle};
use zbus::{
    zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Value},
    Connection, Proxy,
};
const DEST: &str = "org.freedesktop.portal.Desktop";
const ROOT: &str = "/org/freedesktop/portal/desktop";
type Results = HashMap<String, OwnedValue>;

/// Keep the session and remote descriptor alive together for the recorder.
/// Call `close` and await it on normal shutdown; Drop is only a fallback.
pub struct Session {
    proxy: Option<Proxy<'static>>,
    fd: Option<OwnedFd>,
    monitor: Option<JoinHandle<()>>,
    closed: watch::Receiver<bool>,
    pub node_id: u32,
    pub serial: Option<u64>,
}
impl Session {
    pub fn remote_fd(&self) -> Option<BorrowedFd<'_>> {
        self.fd.as_ref().map(AsFd::as_fd)
    }
    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }
    pub async fn close(&mut self) -> anyhow::Result<()> {
        self.fd = None;
        if let Some(monitor) = self.monitor.take() {
            monitor.abort();
            let _ = monitor.await;
        }
        if let Some(proxy) = self.proxy.take() {
            tokio::time::timeout(Duration::from_secs(2), proxy.call::<_, _, ()>("Close", &()))
                .await??;
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.fd = None;
        if let Some(monitor) = self.monitor.take() {
            monitor.abort();
        }
        if let Some(proxy) = self.proxy.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(2),
                        proxy.call::<_, _, ()>("Close", &()),
                    )
                    .await;
                });
            }
        }
    }
}

/// Negotiate one monitor/window stream, without persistent permissions.
/// `connection` should be a dedicated session-bus connection owned by the caller.
/// All admitted requests and sessions are closed on cancellation or failure.
pub async fn open(
    connection: Connection,
    cancel: watch::Receiver<bool>,
) -> anyhow::Result<Option<Session>> {
    if *cancel.borrow() {
        return Ok(None);
    }
    let portal = Proxy::new_owned(
        connection.clone(),
        DEST,
        ROOT,
        "org.freedesktop.portal.ScreenCast",
    )
    .await?;
    let available: u32 = portal.get_property("AvailableSourceTypes").await?;
    let version: u32 = portal.get_property("version").await?;
    let types = available & 3;
    ensure!(types != 0, "Portal offers no monitor or window source");
    let session_token = format!("llcontrol_{}", uuid::Uuid::new_v4().simple());
    let sender = sender(&connection)?;
    let path = OwnedObjectPath::try_from(format!("{ROOT}/session/{sender}/{session_token}"))?;
    let proxy = Proxy::new_owned(
        connection.clone(),
        DEST,
        path.clone(),
        "org.freedesktop.portal.Session",
    )
    .await?;
    let mut signals = proxy.receive_signal("Closed").await?;
    let (closed_tx, closed) = watch::channel(false);
    let monitor = tokio::spawn(async move {
        let _ = signals.next().await;
        let _ = closed_tx.send(true);
    });
    let mut session = Session {
        proxy: Some(proxy),
        fd: None,
        monitor: Some(monitor),
        closed,
        node_id: 0,
        serial: None,
    };
    let outcome = async {
        let created = call_request(
            &connection,
            cancel.clone(),
            session.closed.clone(),
            |token| async {
                let options = HashMap::from([
                    ("handle_token", Value::from(token)),
                    ("session_handle_token", Value::from(session_token)),
                ]);
                portal.call("CreateSession", &(options,)).await
            },
        )
        .await?;
        let Some(created) = created else {
            return Ok(false);
        };
        let handle = String::try_from(
            created
                .get("session_handle")
                .context("Missing session handle")?
                .try_clone()?,
        )?;
        ensure!(
            handle == path.as_str(),
            "Portal returned an unexpected session handle"
        );
        ensure!(
            !*session.closed.borrow(),
            "ScreenCast session closed during selection"
        );
        let selected = call_request(
            &connection,
            cancel.clone(),
            session.closed.clone(),
            |token| async {
                let options = HashMap::from([
                    ("handle_token", Value::from(token)),
                    ("types", Value::from(types)),
                    ("multiple", Value::from(false)),
                ]);
                portal.call("SelectSources", &(path.clone(), options)).await
            },
        )
        .await?;
        if selected.is_none() {
            return Ok(false);
        }
        ensure!(
            !*session.closed.borrow(),
            "ScreenCast session closed before Start"
        );
        let started = call_request(
            &connection,
            cancel.clone(),
            session.closed.clone(),
            |token| async {
                let options = HashMap::from([("handle_token", Value::from(token))]);
                portal.call("Start", &(path.clone(), "", options)).await
            },
        )
        .await?;
        let Some(started) = started else {
            return Ok(false);
        };
        let streams = Vec::<(u32, Results)>::try_from(
            started
                .get("streams")
                .context("Missing PipeWire streams")?
                .try_clone()?,
        )?;
        ensure!(streams.len() == 1, "Expected exactly one ScreenCast stream");
        let (node, properties) = streams
            .into_iter()
            .next()
            .context("Missing ScreenCast stream")?;
        ensure!(node != 0, "Invalid PipeWire node ID");
        let serial = properties
            .get("pipewire-serial")
            .map(u64::try_from)
            .transpose()?;
        ensure!(
            version < 6 || serial.is_some(),
            "Portal v6 stream lacks stable PipeWire serial"
        );
        if *cancel.borrow() {
            return Ok(false);
        }
        ensure!(!*session.closed.borrow(), "ScreenCast source closed");
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        let fd = tokio::time::timeout(
            Duration::from_secs(10),
            portal.call("OpenPipeWireRemote", &(path.clone(), options)),
        )
        .await??;
        session.fd = Some(fd);
        session.node_id = node;
        session.serial = serial;
        Ok(!*cancel.borrow() && !*session.closed.borrow())
    }
    .await;
    match outcome {
        Ok(true) => Ok(Some(session)),
        Ok(false) => {
            let _ = session.close().await;
            Ok(None)
        }
        Err(error) => {
            let _ = session.close().await;
            Err(error)
        }
    }
}
fn sender(connection: &Connection) -> anyhow::Result<String> {
    Ok(connection
        .unique_name()
        .context("Missing portal client identity")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_"))
}
async fn call_request<F, Fut>(
    connection: &Connection,
    mut cancel: watch::Receiver<bool>,
    mut closed: watch::Receiver<bool>,
    call: F,
) -> anyhow::Result<Option<Results>>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = zbus::Result<OwnedObjectPath>>,
{
    if *cancel.borrow() {
        return Ok(None);
    }
    let token = format!("llcontrol_{}", uuid::Uuid::new_v4().simple());
    let path = format!("{ROOT}/request/{}/{token}", sender(connection)?);
    let proxy = Proxy::new(
        connection,
        DEST,
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await?;
    let mut signals = proxy.receive_signal("Response").await?;
    let mut cancelled = false;
    let outcome = tokio::time::timeout(Duration::from_secs(120), async {
        let handle = tokio::time::timeout(Duration::from_secs(10), call(token)).await??;
        ensure!(handle.as_str() == path, "Unexpected portal request handle");
        let message = tokio::select! {
            biased;
            _=cancel.wait_for(|v|*v)=>{cancelled=true;return Ok(None);},
            _=closed.wait_for(|v|*v)=>anyhow::bail!("ScreenCast session closed while waiting for selection"),
            message=signals.next()=>message.context("ScreenCast portal disconnected")?,
        };
        let (code, result): (u32, Results) = message.body().deserialize()?;
        match code {
            0 => Ok(Some(result)),
            1 => Ok(None),
            _ => anyhow::bail!("ScreenCast portal rejected request ({code})"),
        }
    })
    .await;
    if cancelled || !matches!(&outcome, Ok(Ok(_))) {
        let _ = tokio::time::timeout(Duration::from_secs(2), proxy.call::<_, _, ()>("Close", &()))
            .await;
    }
    outcome.context("ScreenCast selection timed out")?
}

#[cfg(test)]
#[path = "screencast_tests.rs"]
mod tests;
