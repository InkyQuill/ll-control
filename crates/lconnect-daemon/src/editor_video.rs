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

type DecodedFrame = Option<Result<(usize, Arc<RgbImage>), String>>;

pub(crate) struct EditorVideo {
    frame: watch::Receiver<DecodedFrame>,
    requested: watch::Sender<usize>,
    stop: watch::Sender<bool>,
}
impl Drop for EditorVideo {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
impl EditorVideo {
    pub async fn frame_at(&self, target: usize) -> Result<Arc<RgbImage>> {
        self.requested.send_replace(target);
        let mut frames = self.frame.clone();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match frames.borrow_and_update().as_ref() {
                    Some(Ok((index, frame))) if *index == target => return Ok(frame.clone()),
                    Some(Err(error)) => anyhow::bail!(error.clone()),
                    _ => {}
                }
                frames.changed().await.context("editor decoder stopped")?;
            }
        })
        .await
        .context("editor frame deadline")?
    }
    pub fn spawn(source: PinnedMedia, permit: WorkPermit, target: usize) -> Self {
        let (requested, requests) = watch::channel(target);
        let (send, frame) = watch::channel(None);
        let (stop, mut cancelled) = watch::channel(false);
        tokio::spawn(async move {
            let result = run(source, target, requests, send.clone(), &mut cancelled).await;
            if let Err(error) = result {
                send.send_replace(Some(Err(error.to_string())));
            }
            drop(permit);
        });
        Self {
            frame,
            stop,
            requested,
        }
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
    target: usize,
    mut requests: watch::Receiver<usize>,
    send: watch::Sender<DecodedFrame>,
    stop: &mut watch::Receiver<bool>,
) -> Result<()> {
    let (source, start, primer) = tokio::task::spawn_blocking(move || {
        let mut source = source;
        let start = source.keyframe_before(target)?;
        anyhow::ensure!(target - start <= 1024, "editor seek exceeds warmup budget");
        let primer = if start == 0 {
            None
        } else {
            Some(source.read_unit(0)?)
        };
        Ok::<_, anyhow::Error>((source, start, primer))
    })
    .await??;
    let skip_primer = usize::from(primer.is_some());
    let mut child = crate::media_composition::decoder_command(OsStr::new("ffmpeg"), false)
        .spawn()
        .context("editor decoder")?;
    let mut input = child.stdin.take().context("editor decoder stdin")?;
    let mut output = child.stdout.take().context("editor decoder stdout")?;
    let (cancel, shutdown) = watch::channel(false);
    let mut tasks = JoinSet::new();
    let mut writing_stop = shutdown.clone();
    tasks.spawn(async move {
        let mut reader=MediaReader::spawn_at(source,start);
        let outcome:Result<()>=async {
            if let Some(primer)=primer {
                tokio::select!{biased;_=stop_requested(&mut writing_stop)=>return Ok(()),result=input.write_all(&primer.bytes)=>result.context("editor parameter primer")?};
            }
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
        let mut index=start;
        let mut primer_remaining=skip_primer;
        loop {
            let target=*requests.borrow_and_update();
            if index>target && primer_remaining==0 {
                tokio::select!{biased;_=stop_requested(&mut reading_stop)=>return Ok(()),result=requests.changed()=>{result.context("editor request channel closed")?;}}
                continue;
            }
            let mut bytes=vec![0;480*480*3];
            tokio::select!{biased;_=stop_requested(&mut reading_stop)=>return Ok(()),result=output.read_exact(&mut bytes)=>{result.context("editor decode frame")?;}}
            if primer_remaining>0 {primer_remaining-=1;continue;}
            if index==target {
                let frame=RgbImage::from_raw(480,480,bytes).context("editor frame dimensions")?;
                send.send_replace(Some(Ok((index,Arc::new(frame)))));
            }
            index+=1;
        }
    });
    let outcome = tokio::select! {biased;_=stop_requested(stop)=>Ok(()),result=tasks.join_next()=>match result{Some(Ok(result))=>result,Some(Err(error))=>Err(error.into()),None=>Ok(())}};
    cancel.send_replace(true);
    let _ = child.start_kill();
    while tasks.join_next().await.is_some() {}
    child.wait().await.context("editor decoder reap")?;
    outcome
}
