//! Interactive portal capture. The returned bytes are an editor draft, never a publication.
use anyhow::{ensure, Context};
use futures_util::StreamExt;
use std::{
    collections::HashMap, io::Read, os::unix::fs::OpenOptionsExt, sync::Arc, time::Duration,
};
use tokio::sync::{watch, Mutex};
use zbus::{
    zvariant::{OwnedObjectPath, OwnedValue, Value},
    Connection, Proxy,
};

#[derive(Clone, Default)]
pub struct Screenshots(Arc<Mutex<State>>);
#[derive(Default)]
struct State {
    active: Option<Job>,
    closed: bool,
}
struct Job {
    id: String,
    cancel: watch::Sender<bool>,
    done: watch::Receiver<bool>,
    completion: Option<watch::Sender<bool>>,
    started: bool,
}
impl Screenshots {
    /// Reserve before requesting capture, so cancellation cannot race admission.
    pub async fn begin(&self) -> anyhow::Result<String> {
        let mut state = self.0.lock().await;
        ensure!(!state.closed, "Screenshot service is closing");
        ensure!(state.active.is_none(), "A screenshot is already pending");
        let id = uuid::Uuid::new_v4().simple().to_string();
        let (cancel, _) = watch::channel(false);
        let (completion, done) = watch::channel(false);
        state.active = Some(Job {
            id: id.clone(),
            cancel,
            done,
            completion: Some(completion),
            started: false,
        });
        // A lost begin reply must not reserve the service indefinitely.
        let owner = self.clone();
        let reserved = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let mut state = owner.0.lock().await;
            if state
                .active
                .as_ref()
                .is_some_and(|j| j.id == reserved && !j.started)
            {
                state.active = None;
            }
        });
        Ok(id)
    }
    pub async fn capture(&self, id: &str) -> anyhow::Result<Vec<u8>> {
        self.capture_with(id, Connection::session()).await
    }
    async fn capture_with<F>(&self, id: &str, connection: F) -> anyhow::Result<Vec<u8>>
    where
        F: std::future::Future<Output = zbus::Result<Connection>> + Send + 'static,
    {
        let mut state = self.0.lock().await;
        ensure!(!state.closed, "Screenshot service is closing");
        let job = state
            .active
            .as_mut()
            .filter(|j| j.id == id)
            .context("Screenshot reservation expired or cancelled")?;
        ensure!(!job.started, "Screenshot already started");
        job.started = true;
        let mut cancel = job.cancel.subscribe();
        let completion = job
            .completion
            .take()
            .context("Missing screenshot completion owner")?;
        let owner = self.clone();
        let id = id.to_owned();
        // Owned work survives a dropped invoke future; cancel/shutdown joins it.
        let task = tokio::spawn(async move {
            let result = async {
                let connection = tokio::select! {
                    biased;
                    _=cancel.wait_for(|v|*v)=>return Ok(Vec::new()),
                    result=tokio::time::timeout(Duration::from_secs(10),connection)=>result??,
                };
                let uri = request(&connection, Duration::from_secs(120), cancel.clone()).await?;
                if *cancel.borrow() {
                    return Ok(Vec::new());
                }
                let Some(uri) = uri else {
                    return Ok(Vec::new());
                };
                let bytes = tokio::task::spawn_blocking(move || read_image(&uri)).await??;
                // A file read already in progress is joined, but never delivered after cancel.
                if *cancel.borrow() {
                    Ok(Vec::new())
                } else {
                    Ok(bytes)
                }
            }
            .await;
            let mut state = owner.0.lock().await;
            if state.active.as_ref().is_some_and(|j| j.id == id) {
                state.active = None;
            }
            let _ = completion.send(true);
            result
        });
        drop(state);
        task.await?
    }
    pub async fn cancel(&self, id: &str) {
        let done = {
            let mut state = self.0.lock().await;
            let Some(job) = state.active.as_mut().filter(|j| j.id == id) else {
                return;
            };
            if !job.started {
                state.active = None;
                return;
            }
            let _ = job.cancel.send(true);
            job.done.clone()
        };
        let mut done = done;
        let _ = done.wait_for(|v| *v).await;
    }
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let id = {
            let mut state = self.0.lock().await;
            state.closed = true;
            state.active.as_ref().map(|j| j.id.clone())
        };
        if let Some(id) = id {
            self.cancel(&id).await;
        }
        Ok(())
    }
}

async fn request(
    connection: &Connection,
    deadline: Duration,
    mut cancel: watch::Receiver<bool>,
) -> anyhow::Result<Option<String>> {
    let token = format!("lconnect_{}", uuid::Uuid::new_v4().simple());
    let sender = connection
        .unique_name()
        .context("Missing portal client identity")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let portal = Proxy::new(
        connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot",
    )
    .await?;
    let request = Proxy::new(
        connection,
        "org.freedesktop.portal.Desktop",
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await?;
    // Subscribe before Screenshot: some backends emit Response before returning the handle.
    let mut responses = request.receive_signal("Response").await?;
    let options = HashMap::from([
        ("handle_token", Value::from(token.as_str())),
        ("interactive", Value::from(true)),
        ("modal", Value::from(true)),
    ]);
    let mut cancelled = false;
    let outcome = tokio::time::timeout(deadline, async {
        if *cancel.borrow() {
            cancelled = true;
            return Ok(None);
        }
        // Obtain the handle before Close, even if cancellation arrives while the
        // method is in flight. The connection is dedicated to this operation.
        let handle: OwnedObjectPath = tokio::time::timeout(
            Duration::from_secs(10),
            portal.call("Screenshot", &("", options)),
        )
        .await??;
        ensure!(
            handle.as_str() == path,
            "Portal returned an unexpected request handle"
        );
        let message = tokio::select! {
            biased;
            _=cancel.wait_for(|v|*v)=>{cancelled=true;return Ok(None);},
            message=responses.next()=>message.context("Screenshot portal disconnected")?,
        };
        let (code, values): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
        match code {
            0 => Ok(Some(String::try_from(
                values
                    .get("uri")
                    .context("Screenshot response has no URI")?
                    .try_clone()?,
            )?)),
            1 => Ok(None),
            _ => anyhow::bail!("Screenshot portal rejected capture (response {code})"),
        }
    })
    .await;
    // Close on failure/timeout as well; do not leave a consent dialog orphaned.
    if cancelled || !matches!(&outcome, Ok(Ok(_))) {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            request.call::<_, _, ()>("Close", &()),
        )
        .await;
    }
    outcome.context("Screenshot selection timed out")?
}

fn read_image(uri: &str) -> anyhow::Result<Vec<u8>> {
    let url = url::Url::parse(uri)?;
    ensure!(
        url.scheme() == "file" && url.query().is_none() && url.fragment().is_none(),
        "Screenshot portal must return a local file URI"
    );
    let path = url
        .to_file_path()
        .map_err(|()| anyhow::anyhow!("Invalid local screenshot URI"))?;
    // A malformed portal response must not block on a FIFO or grow an unbounded read.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    let max = lconnect_image::assets::MAX_ASSET_BYTES;
    ensure!(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= max as u64,
        "Screenshot must be a regular image within 16 MiB"
    );
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= max, "Screenshot exceeds 16 MiB");
    // Validate dimensions/decoder budgets before bytes cross the IPC boundary.
    lconnect_image::assets::decode_asset(&bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_remote_directory_symlink_and_invalid_images() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_image("https://example.invalid/screenshot.png").is_err());
        assert!(read_image(url::Url::from_directory_path(dir.path()).unwrap().as_str()).is_err());
        let source = dir.path().join("bad.png");
        std::fs::write(&source, b"not a PNG").unwrap();
        assert!(read_image(url::Url::from_file_path(&source).unwrap().as_str()).is_err());
        let link = dir.path().join("link.png");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(read_image(url::Url::from_file_path(&link).unwrap().as_str()).is_err());
    }
    #[test]
    fn bounds_sparse_files_before_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.png");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(lconnect_image::assets::MAX_ASSET_BYTES as u64 + 1)
            .unwrap();
        assert!(read_image(url::Url::from_file_path(path).unwrap().as_str()).is_err());
    }
}

#[cfg(test)]
#[path = "screenshot_tests.rs"]
mod portal_tests;
