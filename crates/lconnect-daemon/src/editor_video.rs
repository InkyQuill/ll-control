//! Bounded read-only video decode for the editor. No HID commands or encoding.
use crate::{admission::WorkPermit, media_assets::PinnedMedia, media_reader::MediaReader};
use anyhow::{Context, Result};
use lconnect_image::theme::RgbImage;
use std::{ffi::OsStr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::watch,
    task::JoinSet,
};

pub(crate) struct EditorVideo {
    frame: watch::Receiver<Option<Result<Arc<RgbImage>, String>>>,
    stop: watch::Sender<bool>,
}
impl Drop for EditorVideo {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
impl EditorVideo {
    pub async fn first_frame(&self) -> Result<Arc<RgbImage>> {
        let mut frames = self.frame.clone();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(frame) = self.latest()? {
                    return Ok(frame);
                }
                frames.changed().await.context("editor decoder stopped")?;
            }
        })
        .await
        .context("editor first frame timeout")?
    }
    pub fn latest(&self) -> Result<Option<Arc<RgbImage>>> {
        match self.frame.borrow().as_ref() {
            Some(Ok(frame)) => Ok(Some(frame.clone())),
            Some(Err(error)) => anyhow::bail!(error.clone()),
            None => Ok(None),
        }
    }
    pub fn spawn(source: PinnedMedia, permit: WorkPermit) -> Self {
        let (send, frame) = watch::channel(None);
        let (stop, mut cancelled) = watch::channel(false);
        tokio::spawn(async move {
            let result = run(source, send.clone(), &mut cancelled).await;
            if let Err(error) = result {
                send.send_replace(Some(Err(error.to_string())));
            }
            drop(permit);
        });
        Self { frame, stop }
    }
}
async fn stop_requested(stop: &mut watch::Receiver<bool>) {
    while !*stop.borrow() {
        if stop.changed().await.is_err() {
            return;
        }
    }
}
async fn run(
    source: PinnedMedia,
    send: watch::Sender<Option<Result<Arc<RgbImage>, String>>>,
    stop: &mut watch::Receiver<bool>,
) -> Result<()> {
    let fps = source.fps().max(1);
    let mut child = crate::media_composition::decoder_command(OsStr::new("ffmpeg"), false)
        .spawn()
        .context("editor decoder")?;
    let mut input = child.stdin.take().context("editor decoder stdin")?;
    let mut output = child.stdout.take().context("editor decoder stdout")?;
    let (cancel, shutdown) = watch::channel(false);
    let mut tasks = JoinSet::new();
    let mut writing_stop = shutdown.clone();
    tasks.spawn(async move {
        let mut reader=MediaReader::spawn(source);
        let outcome:Result<()>=async {
            loop {
                let unit=tokio::select!{biased;_=stop_requested(&mut writing_stop)=>return Ok(()),unit=reader.next_unit()=>unit?};
                tokio::select!{biased;_=stop_requested(&mut writing_stop)=>return Ok(()),result=input.write_all(&unit.bytes)=>result.context("editor decode input")?};
            }
        }.await;
        let cleanup=reader.stop().await;
        outcome?;cleanup.map(|_|())
    });
    let mut reading_stop = shutdown;
    tasks.spawn(async move {
        let mut pace=tokio::time::interval(Duration::from_secs_f64(1.0/f64::from(fps)));
        pace.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select!{biased;_=stop_requested(&mut reading_stop)=>return Ok(()),_=pace.tick()=>{}}
            let mut bytes=vec![0;480*480*3];
            tokio::select!{biased;_=stop_requested(&mut reading_stop)=>return Ok(()),result=tokio::time::timeout(Duration::from_secs(10),output.read_exact(&mut bytes))=>{result.context("editor decode timeout")?.context("editor decode frame")?;}}
            let frame=RgbImage::from_raw(480,480,bytes).context("editor frame dimensions")?;
            send.send_replace(Some(Ok(Arc::new(frame))));
        }
    });
    let outcome = tokio::select! {biased;_=stop_requested(stop)=>Ok(()),result=tasks.join_next()=>match result{Some(Ok(result))=>result,Some(Err(error))=>Err(error.into()),None=>Ok(())}};
    cancel.send_replace(true);
    let _ = child.start_kill();
    while tasks.join_next().await.is_some() {}
    child.wait().await.context("editor decoder reap")?;
    outcome
}
