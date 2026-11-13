//! One owned thumbnail decoder; cancellation and window shutdown await its reap.
use anyhow::{bail, Result};
use llcontrol_ipc::H264Clip;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

struct Active {
    id: String,
    cancel: Arc<AtomicBool>,
}
#[derive(Default)]
struct State {
    closing: bool,
    active: Option<Active>,
}
/// Independent of import admission: at most one import and one thumbnail decoder.
#[derive(Clone, Default)]
pub struct Thumbnails(Arc<Mutex<State>>);
/// Held across daemon load and decode; not released when an invoke response is lost.
pub struct Lease {
    owner: Thumbnails,
    id: String,
    cancel: Arc<AtomicBool>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut state) = self.owner.0.lock() {
            if state.active.as_ref().is_some_and(|a| a.id == self.id) {
                state.active = None;
            }
        }
    }
}
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
impl Thumbnails {
    /// Reserve before loading bytes; reject excess requests instead of queuing them.
    pub fn begin(&self, id: String) -> Result<Lease> {
        if id.len() != 32
            || !id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            bail!("Invalid thumbnail request ID");
        }
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Thumbnail state unavailable"))?;
        if state.closing {
            bail!("Application is closing");
        }
        if state.active.is_some() {
            bail!("Thumbnail worker busy");
        }
        let cancel = Arc::new(AtomicBool::new(false));
        state.active = Some(Active {
            id: id.clone(),
            cancel: cancel.clone(),
        });
        Ok(Lease {
            owner: self.clone(),
            id,
            cancel,
        })
    }
    /// Only the matching request may cancel a decoder; late cleanup is harmless.
    pub fn cancel(&self, id: &str) -> Result<()> {
        let state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Thumbnail state unavailable"))?;
        if let Some(active) = state.active.as_ref().filter(|a| a.id == id) {
            active.cancel.store(true, Ordering::Release);
        }
        Ok(())
    }
    /// Prevent new requests and wait for in-flight loading/decoding to release admission.
    pub async fn shutdown(&self) -> Result<()> {
        let owner = self.clone();
        tokio::task::spawn_blocking(move || loop {
            {
                let mut state = owner
                    .0
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Thumbnail state unavailable"))?;
                state.closing = true;
                let Some(active) = state.active.as_ref() else {
                    return Ok(());
                };
                active.cancel.store(true, Ordering::Release);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        })
        .await?
    }
}
impl Lease {
    /// Decode off the executor; the blocking worker owns the lease until reap.
    pub async fn render(self, clip: H264Clip) -> Result<Vec<u8>> {
        self.render_with(clip, crate::media_prepare::thumbnail)
            .await
    }
    async fn render_with<F>(self, clip: H264Clip, render: F) -> Result<Vec<u8>>
    where
        F: FnOnce(&H264Clip, &AtomicBool) -> Result<Vec<u8>> + Send + 'static,
    {
        let _cancel_on_drop = CancelOnDrop(self.cancel.clone());
        tokio::task::spawn_blocking(move || {
            let result = render(&clip, &self.cancel);
            drop(self);
            result
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    #[tokio::test]
    async fn cancelled_request_keeps_admission_until_worker_finishes() {
        let service = Thumbnails::default();
        let lease = service.begin(A.into()).unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(lease.render_with(
            H264Clip {
                fps: 24,
                units: vec![],
            },
            move |_, cancel| {
                started_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                assert!(cancel.load(Ordering::Acquire));
                bail!("cancelled fixture")
            },
        ));
        started_rx.await.unwrap();
        service.cancel(B).unwrap();
        service.cancel(A).unwrap();
        assert!(service.begin(B.into()).is_err());
        release_tx.send(()).unwrap();
        assert!(task.await.unwrap().is_err());
        let next = service.begin(B.into()).unwrap();
        service.cancel(A).unwrap();
        assert!(!next.cancel.load(Ordering::Acquire));
    }
    #[tokio::test]
    async fn aborting_waiter_cancels_worker_without_releasing_its_lease_early() {
        let service = Thumbnails::default();
        let lease = service.begin(A.into()).unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(lease.render_with(
            H264Clip {
                fps: 24,
                units: vec![],
            },
            move |_, cancel| {
                started_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                assert!(cancel.load(Ordering::Acquire));
                bail!("cancelled fixture")
            },
        ));
        started_rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(service
            .0
            .lock()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .cancel
            .load(Ordering::Acquire));
        assert!(service.begin(B.into()).is_err());
        release_tx.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if service.begin(B.into()).is_ok() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn shutdown_cancels_load_lease_and_waits_before_closing_admission() {
        let service = Thumbnails::default();
        let lease = service.begin(A.into()).unwrap();
        let owner = service.clone();
        let mut task = tokio::spawn(async move { owner.shutdown().await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(60), &mut task)
                .await
                .is_err()
        );
        assert!(lease.cancel.load(Ordering::Acquire));
        drop(lease);
        task.await.unwrap().unwrap();
        assert!(service.begin(B.into()).is_err());
    }
}
