//! Owned blocking media reader with bounded prefetch and a joined stop barrier.
use crate::media_assets::PinnedMedia;
use llcontrol_ipc::H264AccessUnit;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::{sync::mpsc, task::JoinHandle};

const PREFETCH_FRAMES: usize = 2;

pub struct MediaReader {
    frames: mpsc::Receiver<anyhow::Result<H264AccessUnit>>,
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<Option<PinnedMedia>>>,
    next_index: usize,
    fps: u8,
    frame_count: usize,
}
impl MediaReader {
    /// Call inside a Tokio runtime after acquiring the source on a blocking job.
    /// The worker owns the source (and its deletion pin) until it actually exits.
    pub fn spawn(source: PinnedMedia) -> Self {
        Self::spawn_at(source, 0)
    }
    pub(crate) fn spawn_at(source: PinnedMedia, index: usize) -> Self {
        let fps = source.fps();
        let frames = source.frames();
        Self::spawn_source(fps, frames, index, ReaderSource::Media(source))
    }
    #[cfg(test)]
    pub(crate) fn spawn_with(
        fps: u8,
        frame_count: usize,
        read: impl FnMut(usize) -> anyhow::Result<H264AccessUnit> + Send + 'static,
    ) -> Self {
        Self::spawn_source(fps, frame_count, 0, ReaderSource::Test(Box::new(read)))
    }
    fn spawn_source(fps: u8, frame_count: usize, start: usize, mut source: ReaderSource) -> Self {
        let (send, frames) = mpsc::channel(PREFETCH_FRAMES);
        let cancel = Arc::new(AtomicBool::new(false));
        let stopped = cancel.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let mut index = start;
            while !stopped.load(Ordering::Acquire) {
                // A source always has at least one validated frame. Keep the
                // guard here so an invalid future source cannot spin or divide by zero.
                let unit = if frame_count == 0 {
                    Err(anyhow::anyhow!("empty media source"))
                } else {
                    source.read(index)
                };
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                let failed = unit.is_err();
                if send.blocking_send(unit).is_err() || failed {
                    break;
                }
                index = (index + 1) % frame_count;
            }
            source.into_media()
        });
        Self {
            frames,
            cancel,
            worker: Some(worker),
            fps,
            frame_count,
            next_index: start,
        }
    }
    pub fn fps(&self) -> u8 {
        self.fps
    }
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
    /// Receiver cancellation does not consume a queued AU or cancel its producer.
    /// Pacing and HID writes belong to the LCD actor; this worker only reads.
    pub async fn next_unit(&mut self) -> anyhow::Result<H264AccessUnit> {
        if self.cancel.load(Ordering::Acquire) {
            anyhow::bail!("media reader stopped");
        }
        let unit = self
            .frames
            .recv()
            .await
            .unwrap_or_else(|| Err(anyhow::anyhow!("media reader closed")))?;
        self.next_index = (self.next_index + 1) % self.frame_count;
        Ok(unit)
    }
    /// The next undelivered index, independent of bounded reader prefetch.
    pub(crate) fn next_index(&self) -> usize {
        self.next_index
    }
    /// Join the old reader before transferring its source to a different pipeline.
    pub(crate) async fn take_source(&mut self) -> anyhow::Result<PinnedMedia> {
        self.cancel();
        let result = self.join().await?;
        result.ok_or_else(|| anyhow::anyhow!("media reader source already released"))
    }

    fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Release);
        // Wake a worker blocked by backpressure and discard any unsent generation.
        self.frames.close();
        while self.frames.try_recv().is_ok() {}
    }
    /// No replacement source may start until this finishes. Cancellation of the
    /// stop future retains the join handle, so a later stop can still join it.
    /// An in-flight filesystem read cannot be force-aborted; its source/pin stays
    /// owned until the read returns and the worker exits.
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        self.cancel();
        self.join().await?;
        Ok(())
    }
    async fn join(&mut self) -> anyhow::Result<Option<PinnedMedia>> {
        let mut source = None;
        if let Some(worker) = self.worker.as_mut() {
            let result = worker.await;
            self.worker = None;
            // A send that already reserved capacity can finish after close().
            // Joining first makes this the final drain of the old generation.
            while self.frames.try_recv().is_ok() {}
            source = result?;
        }
        Ok(source)
    }
}
enum ReaderSource {
    Media(PinnedMedia),
    #[cfg(test)]
    Test(Box<dyn FnMut(usize) -> anyhow::Result<H264AccessUnit> + Send>),
}
impl ReaderSource {
    fn read(&mut self, index: usize) -> anyhow::Result<H264AccessUnit> {
        match self {
            Self::Media(source) => source.read_unit(index),
            #[cfg(test)]
            Self::Test(read) => read(index),
        }
    }
    fn into_media(self) -> Option<PinnedMedia> {
        match self {
            Self::Media(source) => Some(source),
            #[cfg(test)]
            Self::Test(_) => None,
        }
    }
}
impl Drop for MediaReader {
    /// Best-effort cancellation only. Lifecycle callers must await stop for a barrier.
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;
    fn unit(index: usize) -> H264AccessUnit {
        H264AccessUnit {
            bytes: vec![index as u8],
            keyframe: index == 0,
        }
    }
    #[tokio::test]
    async fn prefetch_is_bounded_and_loop_order_is_preserved() {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let mut reader = MediaReader::spawn_with(24, 5, move |index| {
            seen.fetch_add(1, Ordering::SeqCst);
            Ok(unit(index))
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while calls.load(Ordering::SeqCst) < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "two queued AUs plus one blocked send"
        );
        for expected in [0, 1, 2, 3, 4, 0, 1] {
            assert_eq!(reader.next_unit().await.unwrap().bytes, vec![expected]);
        }
        reader.stop().await.unwrap();
        reader.stop().await.unwrap();
        assert!(reader.next_unit().await.is_err());
    }
    #[tokio::test]
    async fn cancelled_stop_keeps_join_until_blocked_read_and_owner_finish() {
        let (entered, started) = tokio::sync::oneshot::channel();
        let mut entered = Some(entered);
        let (release, wait) = std::sync::mpsc::channel();
        let dropped = Arc::new(AtomicBool::new(false));
        struct Owner(Arc<AtomicBool>);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let owner = Owner(dropped.clone());
        let mut reader = MediaReader::spawn_with(24, 1, move |_| {
            let _ = &owner;
            if let Some(entered) = entered.take() {
                entered.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            Ok(unit(0))
        });
        started.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(10), reader.next_unit())
                .await
                .is_err()
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), reader.stop())
                .await
                .is_err()
        );
        assert!(reader.worker.is_some());
        assert!(!dropped.load(Ordering::SeqCst));
        assert!(reader.next_unit().await.is_err());
        release.send(()).unwrap();
        reader.stop().await.unwrap();
        assert!(dropped.load(Ordering::SeqCst));
        assert!(reader.next_unit().await.is_err());
    }
    #[tokio::test]
    async fn read_failure_ends_stream_without_repeating_the_last_frame() {
        let mut reader = MediaReader::spawn_with(24, 3, |index| {
            if index == 1 {
                anyhow::bail!("corrupt frame")
            } else {
                Ok(unit(index))
            }
        });
        assert_eq!(reader.next_unit().await.unwrap().bytes, vec![0]);
        assert!(reader
            .next_unit()
            .await
            .unwrap_err()
            .to_string()
            .contains("corrupt frame"));
        assert!(reader
            .next_unit()
            .await
            .unwrap_err()
            .to_string()
            .contains("closed"));
        reader.stop().await.unwrap();
    }

    #[tokio::test]
    async fn panicked_worker_is_joined_once_and_stop_remains_repeatable() {
        let mut reader = MediaReader::spawn_with(24, 1, |_| panic!("reader failure"));
        assert!(reader.next_unit().await.is_err());
        let error = reader.stop().await.unwrap_err();
        assert!(error
            .downcast_ref::<tokio::task::JoinError>()
            .unwrap()
            .is_panic());
        assert!(reader.worker.is_none());
        reader.stop().await.unwrap();
    }

    #[tokio::test]
    async fn drop_cancels_backpressure_and_releases_worker_owner() {
        let (released, done) = tokio::sync::oneshot::channel();
        struct Owner(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for Owner {
            fn drop(&mut self) {
                let _ = self.0.take().unwrap().send(());
            }
        }
        let owner = Owner(Some(released));
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let reader = MediaReader::spawn_with(24, 1, move |_| {
            let _ = &owner;
            seen.fetch_add(1, Ordering::SeqCst);
            Ok(unit(0))
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while calls.load(Ordering::SeqCst) < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(reader);
        tokio::time::timeout(Duration::from_secs(1), done)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn reserved_send_after_cancel_is_never_delivered_and_join_drains_it() {
        let (send, frames) = mpsc::channel(2);
        let permit = send.reserve_owned().await.unwrap();
        let (release, wait) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(async move {
            wait.await.unwrap();
            permit.send(Ok(unit(0)));
            None
        });
        let mut reader = MediaReader {
            frames,
            cancel: Arc::new(AtomicBool::new(false)),
            worker: Some(worker),
            fps: 24,
            frame_count: 1,
            next_index: 0,
        };
        {
            let stop = reader.stop();
            tokio::pin!(stop);
            // Poll through close + initial drain, then pause at join. Only now
            // let the worker send via the permit obtained before close.
            assert!(futures::poll!(&mut stop).is_pending());
            release.send(()).unwrap();
            stop.await.unwrap();
        }
        assert!(reader.frames.is_empty());
        assert!(reader.next_unit().await.is_err());
    }
}
