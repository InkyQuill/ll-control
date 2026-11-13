//! Shared bounded upload sessions. Run calls on blocking workers, never HID actors.
use crate::theme_fonts::ThemeFontStore;
use llcontrol_ipc::{
    FontUploadRequest as Request, FontUploadState, FONT_CHUNK_BYTES, MAX_FONT_BYTES,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const MAX_UPLOADS: usize = 4;
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
struct Session {
    state: FontUploadState,
    bytes: Vec<u8>,
    touched: Instant,
}
#[derive(Clone)]
pub struct FontUploads {
    closed: Arc<std::sync::atomic::AtomicBool>,
    store: ThemeFontStore,
    sessions: Arc<Mutex<BTreeMap<String, Session>>>,
}
impl FontUploads {
    pub(crate) fn close_staging(&self, lease: crate::admission::Lease) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Release);
        let sessions = self.sessions.clone();
        lease.cleanup(move || {
            sessions
                .lock()
                .map_err(|_| anyhow::anyhow!("upload sessions poisoned"))?
                .clear();
            Ok(())
        });
    }
    pub fn new(store: ThemeFontStore) -> Self {
        Self {
            closed: Arc::default(),
            store,
            sessions: Default::default(),
        }
    }
    pub fn handle(&self, request: Request) -> anyhow::Result<FontUploadState> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("font uploads lock poisoned"))?;
        anyhow::ensure!(
            !self.closed.load(std::sync::atomic::Ordering::Acquire),
            "upload sessions closed for maintenance"
        );
        let now = Instant::now();
        sessions.retain(|_, session| now.duration_since(session.touched) < IDLE_TIMEOUT);
        if let Request::Begin { size } = request {
            anyhow::ensure!(
                (1..=MAX_FONT_BYTES).contains(&size),
                "font size must be 1..=16 MiB"
            );
            anyhow::ensure!(
                sessions.len() < MAX_UPLOADS,
                "font upload capacity exhausted; cancel existing uploads or wait for idle expiry"
            );
            let id = format!("{:032x}", rand::random::<u128>());
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(size)?;
            let state = FontUploadState {
                id: id.clone(),
                size,
                received: 0,
                asset: None,
                cancelled: false,
            };
            sessions.insert(
                id,
                Session {
                    state: state.clone(),
                    bytes,
                    touched: now,
                },
            );
            return Ok(state);
        }
        let id = match &request {
            Request::Begin { .. } => unreachable!(),
            Request::Chunk { id, .. }
            | Request::Status { id }
            | Request::Finish { id }
            | Request::Cancel { id } => id,
        };
        anyhow::ensure!(id.len() == 32, "invalid upload id");
        if matches!(request, Request::Cancel { .. }) {
            let mut session = sessions
                .remove(id)
                .ok_or_else(|| anyhow::anyhow!("unknown or expired upload"))?;
            session.state.cancelled = true;
            return Ok(session.state);
        }
        let session = sessions
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("unknown or expired upload"))?;
        match request {
            Request::Chunk { offset, bytes, .. } => {
                anyhow::ensure!(
                    !bytes.is_empty() && bytes.len() <= FONT_CHUNK_BYTES,
                    "invalid font chunk size"
                );
                let end = offset
                    .checked_add(bytes.len())
                    .ok_or_else(|| anyhow::anyhow!("offset overflow"))?;
                anyhow::ensure!(
                    end <= session.state.size,
                    "chunk exceeds declared font size"
                );
                if end <= session.state.received {
                    // Exact retries are harmless, but changed/overlapping data cannot rewrite history.
                    anyhow::ensure!(
                        session.bytes.get(offset..end) == Some(bytes.as_slice()),
                        "chunk retry differs"
                    );
                } else {
                    anyhow::ensure!(
                        session.state.asset.is_none() && offset == session.state.received,
                        "unexpected chunk offset"
                    );
                    session.bytes.extend_from_slice(&bytes);
                    session.state.received = end;
                }
            }
            Request::Finish { .. } => {
                anyhow::ensure!(
                    session.state.received == session.state.size,
                    "font upload incomplete"
                );
                if session.state.asset.is_none() {
                    self.store.import_key(&session.state.id, &session.bytes)?;
                    session.state.asset = Some(session.state.id.clone());
                }
            }
            Request::Status { .. } => {}
            Request::Begin { .. } | Request::Cancel { .. } => unreachable!(),
        }
        session.touched = Instant::now();
        Ok(session.state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_sessions_release_capacity_without_creating_fonts() {
        let dir = tempfile::tempdir().unwrap();
        let uploads = FontUploads::new(ThemeFontStore::new(dir.path().join("fonts")));
        let id = uploads.handle(Request::Begin { size: 100 }).unwrap().id;
        uploads
            .sessions
            .lock()
            .unwrap()
            .get_mut(&id)
            .unwrap()
            .touched = Instant::now() - IDLE_TIMEOUT;
        assert!(uploads.handle(Request::Status { id }).is_err());
        assert!(uploads.sessions.lock().unwrap().is_empty());
        assert!(!dir.path().join("fonts").exists());
    }
}
