//! Finite, pinned source reads for gallery editing. No playback or device access.
use crate::{builtin_media, media_assets::MediaAssetStore};
use anyhow::{ensure, Context};
use futures::{stream, stream::BoxStream, StreamExt};
use lconnect_ipc::H264AccessUnit;
use std::sync::{Arc, Mutex};

/// Two downloads may coexist; each owns one pinned descriptor and one frame read.
#[derive(Clone)]
pub struct MediaExports {
    store: MediaAssetStore,
    slots: crate::admission::WorkSlots,
    stop: tokio::sync::watch::Sender<bool>,
}

/// Source metadata and a finite sequence in original access-unit order.
/// The consumer must discard an incomplete transfer rather than edit a prefix.
pub struct EditorMedia {
    pub fps: u8,
    pub frames: usize,
    pub encoded_bytes: u64,
    pub units: BoxStream<'static, anyhow::Result<H264AccessUnit>>,
}

impl MediaExports {
    pub(crate) fn with_admission(mut self, admission: crate::admission::Admission) -> Self {
        self.slots = self.slots.with_admission(admission.clone());
        self
    }
    pub(crate) fn close_staging(&self) {
        self.stop.send_replace(true);
    }
    pub fn new(store: MediaAssetStore) -> Self {
        Self {
            stop: tokio::sync::watch::channel(false).0,
            store,
            slots: crate::admission::WorkSlots::new(2, Default::default()),
        }
    }

    /// Admission is fail-fast. The blocking opener/read owns its permit even
    /// when its awaiting future is cancelled, so cancellation never hides I/O.
    pub async fn open(&self, key: String) -> anyhow::Result<EditorMedia> {
        ensure!(key != builtin_media::KEY, "Builtin media cannot be edited");
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .context("Media export is busy")?;
        let store = self.store.clone();
        let stop = self.stop.subscribe();
        tokio::task::spawn_blocking(move || {
            let source = store.acquire_source(&key)?;
            let fps = source.fps();
            let frames = source.frames();
            let encoded_bytes = source.encoded_bytes();
            let mut reader = ExportReader {
                source: Some(source),
                permit: Some(permit),
            };
            let units = cancelable_units(frames, move |index| reader.read(index), stop);
            Ok(EditorMedia {
                fps,
                frames,
                encoded_bytes,
                units,
            })
        })
        .await?
    }
}

struct ExportReader {
    source: Option<crate::media_assets::PinnedMedia>,
    permit: Option<crate::admission::WorkPermit>,
}
impl ExportReader {
    fn read(&mut self, index: usize) -> anyhow::Result<H264AccessUnit> {
        self.source
            .as_mut()
            .context("export already retired")?
            .read_unit(index)
    }
}
impl Drop for ExportReader {
    fn drop(&mut self) {
        if let (Some(source), Some(permit)) = (self.source.take(), self.permit.take()) {
            source.retire_export(permit);
        }
    }
}
struct ReaderOwner<F> {
    read: Mutex<Option<F>>,
    closed: tokio::sync::watch::Sender<bool>,
}
impl<F> ReaderOwner<F> {
    fn close(&self) {
        self.closed.send_replace(true);
        let read = self
            .read
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        drop(read);
    }
    fn take(&self) -> Option<F> {
        if *self.closed.borrow() {
            return None;
        }
        self.read
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
    fn put(&self, read: F) {
        let mut slot = self
            .read
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *self.closed.borrow() {
            drop(slot);
            drop(read);
        } else {
            *slot = Some(read);
        }
    }
}
struct ExportGuard<F>(Arc<ReaderOwner<F>>);
impl<F> Drop for ExportGuard<F> {
    fn drop(&mut self) {
        self.0.close();
    }
}
fn cancelable_units<F>(
    frames: usize,
    read: F,
    mut stop: tokio::sync::watch::Receiver<bool>,
) -> BoxStream<'static, anyhow::Result<H264AccessUnit>>
where
    F: FnMut(usize) -> anyhow::Result<H264AccessUnit> + Send + 'static,
{
    let owner = Arc::new(ReaderOwner {
        read: Mutex::new(Some(read)),
        closed: tokio::sync::watch::channel(false).0,
    });
    let weak = Arc::downgrade(&owner);
    let mut gone = owner.closed.subscribe();
    tokio::spawn(async move {
        if !*stop.borrow_and_update() {
            tokio::select! { _ = stop.changed() => (), _ = gone.changed() => return }
        }
        if let Some(owner) = weak.upgrade() {
            owner.close();
        }
    });
    stream::unfold(Some((ExportGuard(owner), 0)), move |state| async move {
        let (guard, index) = state?;
        if index >= frames || *guard.0.closed.borrow() {
            return None;
        }
        let mut read = guard.0.take()?;
        let worker_owner = guard.0.clone();
        let mut cancel = guard.0.closed.subscribe();
        let worker = tokio::task::spawn_blocking(move || {
            if *worker_owner.closed.borrow() {
                drop(read);
                return None;
            }
            let result = read(index);
            let more = result.is_ok() && index + 1 < frames;
            if more {
                worker_owner.put(read);
            } else {
                drop(read);
            }
            Some((result, more))
        });
        let result = tokio::select! {
            biased;
            _ = cancel.changed() => return None,
            result = worker => result,
        };
        match result {
            Ok(Some((result, true))) => Some((result, Some((guard, index + 1)))),
            Ok(Some((result, false))) => Some((result, None)),
            Ok(None) => None,
            Err(error) => Some((Err(error.into()), None)),
        }
    })
    .fuse()
    .boxed()
}

#[cfg(test)]
fn finite_units(
    frames: usize,
    read: impl FnMut(usize) -> anyhow::Result<H264AccessUnit> + Send + 'static,
) -> BoxStream<'static, anyhow::Result<H264AccessUnit>> {
    stream::unfold(Some((read, 0)), move |state| async move {
        let (mut read, index) = state?;
        if index >= frames {
            return None;
        }
        // The closure owns descriptor/pin/admission through a possibly blocked
        // read. Dropping the future leaves that owner with the blocking task.
        match tokio::task::spawn_blocking(move || {
            let result = read(index);
            (read, result)
        })
        .await
        {
            Ok((read, Ok(unit))) if index + 1 < frames => Some((Ok(unit), Some((read, index + 1)))),
            Ok((read, result)) => {
                // Release the pin before yielding the final frame or error, even
                // if the consumer never polls the stream for its final None.
                drop(read);
                Some((result, None))
            }
            Err(error) => Some((Err(error.into()), None)),
        }
    })
    .fuse()
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    #[tokio::test]
    async fn maintenance_cancel_drops_idle_export_owner_without_consumer_polling() {
        let gate = crate::admission::Admission::default();
        let lease = gate.enter().unwrap();
        let (stop, rx) = tokio::sync::watch::channel(false);
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = reads.clone();
        let mut receiver = cancelable_units(
            100,
            move |_| {
                let _lease = &lease;
                reads.fetch_add(1, Ordering::SeqCst);
                Ok(H264AccessUnit {
                    bytes: vec![0, 0, 0, 1, 0x65],
                    keyframe: true,
                })
            },
            rx,
        );
        receiver.next().await.unwrap().unwrap();
        assert_eq!(observed.load(Ordering::SeqCst), 1);
        let maintenance = gate.begin_maintenance().unwrap();
        stop.send_replace(true);
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            maintenance.wait_checked(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(gate.active(), 0);
        assert_eq!(observed.load(Ordering::SeqCst), 1);
        drop(receiver);
    }
    #[tokio::test]
    async fn canceling_export_waits_for_detached_blocking_read_to_release_its_owner() {
        let gate = crate::admission::Admission::default();
        let lease = gate.enter().unwrap();
        let (stop, rx) = tokio::sync::watch::channel(false);
        let (started, ready) = tokio::sync::oneshot::channel();
        let mut started = Some(started);
        let (finish, blocked) = mpsc::channel();
        let mut receiver = cancelable_units(
            1,
            move |_| {
                let _lease = &lease;
                started.take().unwrap().send(()).unwrap();
                blocked.recv().unwrap();
                Ok(H264AccessUnit {
                    bytes: vec![0, 0, 0, 1, 0x65],
                    keyframe: true,
                })
            },
            rx,
        );
        let caller = tokio::spawn(async move { receiver.next().await });
        ready.await.unwrap();
        let maintenance = gate.begin_maintenance().unwrap();
        stop.send_replace(true);
        caller.await.unwrap();
        let drain = tokio::spawn(maintenance.wait_checked());
        tokio::task::yield_now().await;
        assert!(!drain.is_finished());
        finish.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), drain)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(gate.active(), 0);
    }
    #[tokio::test]
    async fn cancelled_blocked_read_retains_owner_until_worker_exits() {
        struct Owner(Arc<AtomicBool>);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let owner = Owner(dropped.clone());
        let (started, wait_start) = tokio::sync::oneshot::channel();
        let mut started = Some(started);
        let (release, wait_release) = mpsc::channel();
        let units = finite_units(2, move |_| {
            let _owner = &owner;
            started.take().unwrap().send(()).unwrap();
            wait_release.recv().unwrap();
            Ok(H264AccessUnit {
                bytes: vec![1],
                keyframe: true,
            })
        });
        let worker = tokio::spawn(async move {
            let mut units = units;
            units.next().await
        });
        wait_start.await.unwrap();
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());
        assert!(!dropped.load(Ordering::Acquire));
        release.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !dropped.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}
