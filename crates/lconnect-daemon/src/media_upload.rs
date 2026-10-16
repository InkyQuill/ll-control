//! Capability-addressed media staging. All methods run on blocking jobs.
//! The session lock serializes finish/cancel; a cancelled caller cannot abandon
//! a commit in progress. A receipt uses the upload id as the permanent asset key.
use crate::{media_assets::MediaAssetStore, media_file::MediaFileWriter};
use anyhow::{ensure, Context};
use lconnect_ipc::{
    validate_h264_access_unit, H264AccessUnit, MediaUploadPhase as Phase,
    MediaUploadRequest as Request, MediaUploadState as State, PreparedClipInfo,
    MAX_MEDIA_UPLOAD_BYTES, MAX_MEDIA_UPLOAD_FRAMES,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_ACTIVE: usize = 4;
const MAX_SESSIONS: usize = 64;
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

struct Session {
    state: State,
    writer: Option<MediaFileWriter>,
    last: Option<H264AccessUnit>,
    touched: Instant,
    cleanup_owner: Option<crate::admission::Lease>,
}
impl Drop for Session {
    fn drop(&mut self) {
        let writer = self.writer.take();
        if let Some(owner) = self.cleanup_owner.take() {
            owner.cleanup(move || {
                drop(writer);
                Ok(())
            });
        }
    }
}
#[derive(Clone)]
pub struct MediaUploads {
    admission: crate::admission::Admission,
    closed: Arc<std::sync::atomic::AtomicBool>,
    store: MediaAssetStore,
    sessions: Arc<Mutex<BTreeMap<String, Session>>>,
}
impl MediaUploads {
    pub(crate) fn with_admission(mut self, admission: crate::admission::Admission) -> Self {
        self.admission = admission;
        self
    }
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
    pub fn new(store: MediaAssetStore) -> Self {
        Self {
            admission: Default::default(),
            closed: Arc::default(),
            store,
            sessions: Default::default(),
        }
    }
    pub fn handle(&self, request: Request) -> anyhow::Result<State> {
        let id = match &request {
            Request::Begin { id, .. }
            | Request::Chunk { id, .. }
            | Request::Status { id }
            | Request::Finish { id }
            | Request::Cancel { id } => id.clone(),
        };
        ensure!(
            id.len() == 32
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid media upload id"
        );
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("media uploads lock poisoned"))?;
        anyhow::ensure!(
            !self.closed.load(std::sync::atomic::Ordering::Acquire),
            "upload sessions closed for maintenance"
        );
        let now = Instant::now();
        // Lazy expiry on the next request, or Drop on daemon shutdown. Both
        // discard staging; neither ever unlinks a published file.
        sessions.retain(|_, session| now.duration_since(session.touched) < IDLE_TIMEOUT);
        if let Request::Begin {
            name,
            frames,
            encoded_bytes,
            ..
        } = &request
        {
            ensure!(
                (1..=MAX_MEDIA_UPLOAD_FRAMES).contains(frames),
                "invalid media frame count"
            );
            ensure!(
                (1..=MAX_MEDIA_UPLOAD_BYTES).contains(encoded_bytes),
                "invalid media byte count"
            );
            if let Some(session) = sessions.get_mut(&id) {
                ensure!(
                    session.state.name == *name
                        && session.state.frames == *frames
                        && session.state.encoded_bytes == *encoded_bytes,
                    "upload id belongs to different media"
                );
                if session.state.phase == Phase::Published {
                    ensure!(
                        self.store.published_file(&id)?.is_some(),
                        "published media no longer exists"
                    );
                }
                session.touched = now;
                return Ok(session.state.clone());
            }
            if let Some(info) = self.store.published_file(&id)? {
                ensure!(
                    info.name == *name
                        && info.frames as usize == *frames
                        && info.encoded_bytes == *encoded_bytes,
                    "upload id already published different media"
                );
                return Ok(receipt(info));
            }
            ensure!(
                sessions.len() < MAX_SESSIONS,
                "media upload receipt capacity exhausted; wait for idle expiry"
            );
            ensure!(
                sessions.values().filter(|s| s.writer.is_some()).count() < MAX_ACTIVE,
                "media upload capacity exhausted"
            );
            let cleanup_owner = Some(self.admission.enter()?);
            let writer = self.store.create_file(name.clone())?;
            let state = State {
                id: id.clone(),
                name: name.clone(),
                frames: *frames,
                encoded_bytes: *encoded_bytes,
                received_frames: 0,
                received_bytes: 0,
                phase: Phase::Receiving,
                asset: None,
                error: None,
            };
            sessions.insert(
                id,
                Session {
                    cleanup_owner,
                    state: state.clone(),
                    writer: Some(writer),
                    last: None,
                    touched: now,
                },
            );
            return Ok(state);
        }
        let Some(session) = sessions.get_mut(&id) else {
            if let Some(info) = self.store.published_file(&id)? {
                ensure!(
                    !matches!(request, Request::Chunk { .. }),
                    "media already published"
                );
                return Ok(receipt(info));
            }
            anyhow::bail!("unknown or expired media upload");
        };
        session.touched = now;
        // Retry/readback can recover a commit that succeeded before a sync or
        // reply failed. Never consume another writer or publish another key.
        if matches!(session.state.phase, Phase::Failed | Phase::Published) {
            if let Some(info) = self.store.published_file(&id)? {
                ensure!(
                    info.name == session.state.name
                        && info.frames as usize == session.state.frames
                        && info.encoded_bytes == session.state.encoded_bytes,
                    "publication receipt mismatch"
                );
                session.state = receipt(info);
            } else if session.state.phase == Phase::Published {
                anyhow::bail!("published media no longer exists");
            }
        }
        match request {
            Request::Chunk { index, unit, .. } => {
                ensure!(
                    session.state.phase == Phase::Receiving,
                    "upload is not receiving"
                );
                validate_h264_access_unit(&unit, index == 0)?;
                if session.state.received_frames > 0 && index == session.state.received_frames - 1 {
                    ensure!(
                        session.last.as_ref() == Some(&unit),
                        "media chunk retry differs"
                    );
                    return Ok(session.state.clone());
                }
                ensure!(
                    index == session.state.received_frames && index < session.state.frames,
                    "unexpected media frame index"
                );
                let total = session
                    .state
                    .received_bytes
                    .checked_add(unit.bytes.len() as u64)
                    .context("media size overflow")?;
                ensure!(
                    total <= session.state.encoded_bytes,
                    "media exceeds declared bytes"
                );
                let result = session
                    .writer
                    .as_mut()
                    .context("media staging unavailable")?
                    .append(&unit);
                if let Err(error) = result {
                    session.writer = None;
                    session.last = None;
                    session.state.phase = Phase::Failed;
                    session.state.error = Some(error.to_string());
                } else {
                    session.state.received_frames += 1;
                    session.state.received_bytes = total;
                    session.last = Some(unit);
                }
            }
            Request::Finish { .. } if session.state.phase != Phase::Published => {
                ensure!(
                    session.state.phase == Phase::Receiving,
                    "upload cannot be finished"
                );
                ensure!(
                    session.state.received_frames == session.state.frames
                        && session.state.received_bytes == session.state.encoded_bytes,
                    "media upload incomplete"
                );
                let writer = session.writer.take().context("media staging unavailable")?;
                session.last = None;
                match self.store.publish_file_at(&id, writer) {
                    Ok(info) => session.state = receipt(info),
                    Err(error) => {
                        session.state.phase = Phase::Failed;
                        session.state.error = Some(error.to_string());
                        if let Some(info) = self.store.published_file(&id)? {
                            ensure!(
                                info.name == session.state.name
                                    && info.frames as usize == session.state.frames
                                    && info.encoded_bytes == session.state.encoded_bytes,
                                "publication receipt mismatch"
                            );
                            session.state = receipt(info);
                        }
                    }
                }
            }
            Request::Cancel { .. } if session.state.phase != Phase::Published => {
                session.writer = None;
                session.last = None;
                session.state.phase = Phase::Cancelled;
                session.state.error = None;
            }
            Request::Status { .. } | Request::Cancel { .. } | Request::Finish { .. } => {}
            Request::Begin { .. } => unreachable!(),
        }
        Ok(session.state.clone())
    }
}

fn receipt(info: PreparedClipInfo) -> State {
    State {
        id: info.key.clone(),
        name: info.name.clone(),
        frames: info.frames as usize,
        encoded_bytes: info.encoded_bytes,
        received_frames: info.frames as usize,
        received_bytes: info.encoded_bytes,
        phase: Phase::Published,
        asset: Some(info),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_expiry_drops_staging_and_restores_active_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let uploads = MediaUploads::new(MediaAssetStore::new(dir.path().into()));
        for i in 0..MAX_ACTIVE {
            uploads
                .handle(Request::Begin {
                    id: format!("{i:032x}"),
                    name: "Video".into(),
                    frames: 1,
                    encoded_bytes: 15,
                })
                .unwrap();
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        for session in uploads.sessions.lock().unwrap().values_mut() {
            session.touched = Instant::now() - IDLE_TIMEOUT;
        }
        uploads
            .handle(Request::Begin {
                id: "f".repeat(32),
                name: "New".into(),
                frames: 1,
                encoded_bytes: 15,
            })
            .unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert_eq!(uploads.sessions.lock().unwrap().len(), 1);
        drop(uploads);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
