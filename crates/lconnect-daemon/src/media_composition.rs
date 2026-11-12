//! Owned FFmpeg decode → live layers → H.264 pipeline. No device access.
use crate::{media_assets::PinnedMedia, media_reader::MediaReader, theme_runtime::ThemeRuntime};
use anyhow::{bail, Context, Result};
use lconnect_image::theme::RgbImage;
use lconnect_ipc::{validate_h264_access_unit, H264AccessUnit};
use std::{
    ffi::{OsStr, OsString},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::{mpsc, watch},
    task::{JoinHandle, JoinSet},
};

const FRAME_BYTES: usize = 480 * 480 * 3;
const MAX_AU: usize = 128 * 1024;
const PREFETCH: usize = 2;
// Bound render advance through FFmpeg/OS pipes as well as the output channel.
// AUD framing needs the next frame; four credits permit that and a short prefetch.
const PIPELINE_FRAMES: usize = 4;

struct CompositionInput {
    source: PinnedMedia,
    start: usize,
    skip: usize,
}

/// A validated first AU and its continuing producer, prepared before LCD mutation.
pub struct PreparedComposition {
    pub(crate) worker: MediaComposition,
    pub(crate) first: H264AccessUnit,
    pub(crate) runtime: Arc<ThemeRuntime>,
    pub(crate) pin: Option<crate::media_assets::MediaPin>,
}
impl std::fmt::Debug for PreparedComposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedComposition")
            .field("theme", self.runtime.theme())
            .finish_non_exhaustive()
    }
}
impl PreparedComposition {
    pub async fn prepare(
        store: crate::media_assets::MediaAssetStore,
        runtime: Arc<ThemeRuntime>,
    ) -> Result<Self> {
        Self::prepare_with_permit(store, runtime, None).await
    }
    pub(crate) async fn prepare_with_permit(
        store: crate::media_assets::MediaAssetStore,
        runtime: Arc<ThemeRuntime>,
        permit: Option<crate::admission::WorkPermit>,
    ) -> Result<Self> {
        runtime.theme().validate()?;
        if !runtime.theme().requires_streaming() {
            bail!("composition requires video or procedural weather");
        }
        let key = runtime.theme().background_video.clone();
        let (source, permit) = tokio::task::spawn_blocking(move || {
            key.map(|key| store.acquire_source(&key))
                .transpose()
                .map(|source| (source, permit))
        })
        .await??;
        let pin = source.as_ref().map(PinnedMedia::pin);
        let mut worker = MediaComposition::spawn_owned(
            source,
            runtime.clone(),
            OsString::from("ffmpeg"),
            permit,
        );
        match worker.next_unit().await {
            Ok(first) => Ok(Self {
                worker,
                first,
                runtime,
                pin,
            }),
            Err(error) => {
                worker
                    .stop()
                    .await
                    .context("failed composition preparation cleanup")?;
                Err(error)
            }
        }
    }
}

/// The supervisor owns children, joined tasks and the source pin after this handle
/// is dropped. Explicit stop is the lifecycle barrier; Drop only requests it.
pub struct MediaComposition {
    frames: mpsc::Receiver<Result<H264AccessUnit>>,
    credits: Arc<tokio::sync::Semaphore>,
    cancel: watch::Sender<bool>,
    supervisor: Option<JoinHandle<()>>,
    stopped: bool,
    receive_deadline: Option<tokio::time::Instant>,
    preparation_permit: Arc<std::sync::Mutex<Option<crate::admission::WorkPermit>>>,
}
impl MediaComposition {
    /// Real failed JoinHandle without subprocesses, confined to fault tests.
    #[cfg(test)]
    pub(crate) fn panicked_supervisor_for_test() -> Self {
        let (send, frames) = mpsc::channel(1);
        drop(send);
        Self {
            frames,
            credits: Arc::new(tokio::sync::Semaphore::new(0)),
            cancel: watch::channel(false).0,
            supervisor: Some(tokio::spawn(async {
                panic!("synthetic codec cleanup panic");
            })),
            stopped: false,
            receive_deadline: None,
            preparation_permit: Arc::new(std::sync::Mutex::new(None)),
        }
    }
    pub fn spawn(source: PinnedMedia, runtime: Arc<ThemeRuntime>) -> Self {
        Self::spawn_with_program(source, runtime, OsString::from("ffmpeg"))
    }
    fn spawn_with_program(
        source: PinnedMedia,
        runtime: Arc<ThemeRuntime>,
        program: OsString,
    ) -> Self {
        Self::spawn_owned(Some(source), runtime, program, None)
    }
    fn spawn_owned(
        source: Option<PinnedMedia>,
        runtime: Arc<ThemeRuntime>,
        program: OsString,
        permit: Option<crate::admission::WorkPermit>,
    ) -> Self {
        Self::spawn_input(
            source.map(|source| CompositionInput {
                source,
                start: 0,
                skip: 0,
            }),
            runtime,
            program,
            permit,
        )
    }
    /// Decode from the preceding IDR but expose exactly the requested next frame.
    pub(crate) fn spawn_at(
        source: PinnedMedia,
        runtime: Arc<ThemeRuntime>,
        next: usize,
    ) -> Result<Self> {
        let start = source.keyframe_before(next)?;
        Ok(Self::spawn_input(
            Some(CompositionInput {
                source,
                start,
                skip: next - start + usize::from(start != 0),
            }),
            runtime,
            OsString::from("ffmpeg"),
            None,
        ))
    }
    fn spawn_input(
        source: Option<CompositionInput>,
        runtime: Arc<ThemeRuntime>,
        program: OsString,
        permit: Option<crate::admission::WorkPermit>,
    ) -> Self {
        let preparation_permit = Arc::new(std::sync::Mutex::new(permit));
        let cleanup_permit = preparation_permit.clone();
        let (send, frames) = mpsc::channel(PREFETCH);
        let credits = Arc::new(tokio::sync::Semaphore::new(PIPELINE_FRAMES));
        let render_credits = credits.clone();
        let (cancel, stop) = watch::channel(false);
        let supervisor_cancel = cancel.clone();
        let supervisor = tokio::spawn(async move {
            // If the caller disappears before enqueueing, retain the slot until
            // all child processes, renderer and reader have really completed.
            let _cleanup_permit = cleanup_permit;
            supervise(
                source,
                runtime,
                send,
                supervisor_cancel,
                stop,
                program,
                render_credits,
            )
            .await;
        });
        Self {
            credits,
            frames,
            cancel,
            supervisor: Some(supervisor),
            stopped: false,
            receive_deadline: None,
            preparation_permit,
        }
    }
    pub(crate) fn release_preparation_permit(&self) {
        self.preparation_permit
            .lock()
            .expect("private preparation lease poisoned")
            .take();
    }
    /// Cancellation of this receive does not cancel its producer or consume an AU.
    pub async fn next_unit(&mut self) -> Result<H264AccessUnit> {
        if self.stopped {
            bail!("composition stopped");
        }
        // Commands may cancel this receive repeatedly. Keep its original
        // deadline until an AU arrives so a stalled codec still expires.
        let deadline = *self
            .receive_deadline
            .get_or_insert_with(|| tokio::time::Instant::now() + Duration::from_secs(10));
        match tokio::time::timeout_at(deadline, self.frames.recv()).await {
            Ok(Some(unit)) => {
                self.receive_deadline = None;
                if unit.is_ok() && self.credits.available_permits() < PIPELINE_FRAMES {
                    self.credits.add_permits(1);
                }
                unit
            }
            Ok(None) => anyhow::bail!("composition worker closed"),
            Err(_) => {
                self.cancel();
                bail!("composition frame timed out");
            }
        }
    }
    fn cancel(&mut self) {
        self.stopped = true;
        self.cancel.send_replace(true);
        self.frames.close();
        while self.frames.try_recv().is_ok() {}
    }
    /// Retain the borrowed JoinHandle if the caller drops its stop future.
    pub async fn stop(&mut self) -> Result<()> {
        self.cancel();
        if let Some(supervisor) = self.supervisor.as_mut() {
            let result = supervisor.await;
            self.supervisor = None;
            while self.frames.try_recv().is_ok() {}
            self.release_preparation_permit();
            result?;
        }
        Ok(())
    }
}
impl Drop for MediaComposition {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn command(program: &OsStr) -> Command {
    let mut command = Command::new(program);
    command
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-threads",
            "1",
            "-filter_threads",
            "1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    command
}
fn decoder_command(program: &OsStr, first_only: bool) -> Command {
    let mut decoder = command(program);
    decoder.args([
        "-xerror",
        "-protocol_whitelist",
        "pipe",
        "-probesize",
        "32",
        "-analyzeduration",
        "0",
        "-f",
        "h264",
        "-max_pixels",
        // FFmpeg checks the aligned decode allocation (512x480 for
        // our 480x480 source), not just the visible canvas.
        "262144",
        "-i",
        "pipe:0",
        "-an",
        "-sn",
        "-dn",
        "-vf",
        "scale=480:480",
        "-pix_fmt",
        "rgb24",
        "-threads",
        "1",
    ]);
    if first_only {
        decoder.args(["-frames:v", "1"]);
    }
    decoder.args(["-f", "rawvideo", "pipe:1"]);
    decoder
}

/// Run inside the owned IPC preparation job, retaining its concurrency permit.
/// Preview uses the first source frame and the requested overlay cycle position.
pub(crate) async fn preview(
    store: crate::media_assets::MediaAssetStore,
    runtime: Arc<ThemeRuntime>,
    elapsed_seconds: u64,
) -> Result<Vec<u8>> {
    let key = runtime
        .theme()
        .background_video
        .clone()
        .context("preview requires video theme")?;
    let (unit, pin) = tokio::task::spawn_blocking(move || {
        let mut source = store.acquire_source(&key)?;
        Ok::<_, anyhow::Error>((source.read_unit(0)?, source.pin()))
    })
    .await??;
    let rendered = preview_unit(runtime, unit, elapsed_seconds).await;
    drop(pin);
    rendered
}

/// Canvas geometry and pixels share the decoded background and live snapshot.
/// The owned theme job retains admission until decoder and renderer both finish.
pub(crate) async fn editor_preview(
    store: crate::media_assets::MediaAssetStore,
    runtime: Arc<ThemeRuntime>,
    elapsed_seconds: u64,
) -> Result<lconnect_ipc::LcdEditorPreview> {
    let key = runtime
        .theme()
        .background_video
        .clone()
        .context("preview requires video theme")?;
    let (unit, pin) = tokio::task::spawn_blocking(move || {
        let mut source = store.acquire_source(&key)?;
        Ok::<_, anyhow::Error>((source.read_unit(0)?, source.pin()))
    })
    .await??;
    let frame = decode_preview_frame(unit, OsStr::new("ffmpeg"), Duration::from_secs(10)).await?;
    let rendered = tokio::task::spawn_blocking(move || {
        let _pin = pin;
        runtime.editor_preview(Some(frame), elapsed_seconds)
    })
    .await??;
    Ok(rendered)
}

/// Same joined/limited decoder for a privately staged source access unit.
/// The caller retains the owning preparation and its admission through this job.
pub(crate) async fn preview_unit(
    runtime: Arc<ThemeRuntime>,
    unit: H264AccessUnit,
    elapsed_seconds: u64,
) -> Result<Vec<u8>> {
    let frame = decode_preview_frame(unit, OsStr::new("ffmpeg"), Duration::from_secs(10)).await?;
    let rendered = tokio::task::spawn_blocking(move || {
        let frame = runtime.compose_video_frame(frame, elapsed_seconds)?;
        Ok::<_, anyhow::Error>(
            lconnect_image::prepare_jpeg_from_image(
                frame.into(),
                lconnect_image::FitMode::Stretch,
                lconnect_image::Quality::default(),
            )?
            .bytes,
        )
    })
    .await?;
    rendered
}

async fn decode_preview_frame(
    unit: H264AccessUnit,
    program: &OsStr,
    timeout: Duration,
) -> Result<RgbImage> {
    let mut child = decoder_command(program, true)
        .spawn()
        .context("Cannot start preview decoder")?;
    let mut input = child.stdin.take().expect("preview piped stdin");
    let output = child.stdout.take().expect("preview piped stdout");
    let work = async {
        let send = async move {
            input.write_all(&unit.bytes).await?;
            input.shutdown().await?;
            // ChildStdin shutdown does not close the pipe descriptor. The H264
            // parser needs EOF to finish the sole access unit.
            drop(input);
            Ok::<_, anyhow::Error>(())
        };
        let read = async {
            let mut bytes = Vec::with_capacity(FRAME_BYTES);
            output
                .take((FRAME_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .await?;
            anyhow::ensure!(
                bytes.len() == FRAME_BYTES,
                "preview decoder returned an invalid frame size"
            );
            Ok::<_, anyhow::Error>(bytes)
        };
        let ((), bytes, status) = tokio::try_join!(send, read, async {
            child.wait().await.map_err(anyhow::Error::from)
        })?;
        anyhow::ensure!(status.success(), "preview decoder failed: {status}");
        RgbImage::from_raw(480, 480, bytes).context("invalid preview frame")
    };
    match tokio::time::timeout(timeout, work).await {
        Ok(Ok(frame)) => Ok(frame),
        outcome => {
            let _ = child.start_kill();
            child.wait().await.context("preview decoder reap")?;
            match outcome {
                Ok(Err(error)) => Err(error),
                Err(_) => bail!("preview decoder timed out"),
                Ok(Ok(_)) => unreachable!("success handled above"),
            }
        }
    }
}

async fn cancelled(stop: &mut watch::Receiver<bool>) {
    if *stop.borrow() {
        return;
    }
    let _ = stop.changed().await;
}
async fn supervise(
    source: Option<CompositionInput>,
    runtime: Arc<ThemeRuntime>,
    send: mpsc::Sender<Result<H264AccessUnit>>,
    cancel: watch::Sender<bool>,
    mut stop: watch::Receiver<bool>,
    program: OsString,
    credits: Arc<tokio::sync::Semaphore>,
) {
    let result = pipeline(
        source,
        runtime,
        send.clone(),
        cancel,
        &mut stop,
        &program,
        credits,
    )
    .await;
    if let Err(error) = result {
        let _ = send.send(Err(error)).await;
    }
}
async fn pipeline(
    source: Option<CompositionInput>,
    runtime: Arc<ThemeRuntime>,
    send: mpsc::Sender<Result<H264AccessUnit>>,
    cancel: watch::Sender<bool>,
    stop: &mut watch::Receiver<bool>,
    program: &OsStr,
    credits: Arc<tokio::sync::Semaphore>,
) -> Result<()> {
    if *stop.borrow() {
        return Ok(());
    }
    let mut decoder = if source.is_some() {
        Some(
            decoder_command(program, false)
                .spawn()
                .context("Cannot start composition decoder")?,
        )
    } else {
        None
    };
    let encoder = command(program)
        .args([
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgb24",
            "-video_size",
            "480x480",
            "-framerate",
            "24",
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-profile:v",
            "baseline",
            "-level:v",
            "3.0",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-crf",
            "25",
            "-g",
            "24",
            "-bf",
            "0",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
            "-x264-params",
            "aud=1:repeat-headers=1",
            "-flush_packets",
            "1",
            "-f",
            "h264",
            "pipe:1",
        ])
        .spawn();
    let mut encoder = match encoder {
        Ok(child) => child,
        Err(error) => {
            if let Some(decoder) = &mut decoder {
                let _ = decoder.start_kill();
                decoder.wait().await?;
            }
            return Err(error.into());
        }
    };
    let input = decoder.as_mut().and_then(|child| child.stdin.take());
    let mut decoded = decoder.as_mut().and_then(|child| child.stdout.take());
    let mut raw = encoder
        .stdin
        .take()
        .expect("encoder configured with piped stdin");
    let mut encoded = encoder
        .stdout
        .take()
        .expect("encoder configured with piped stdout");
    let mut tasks = JoinSet::new();
    let mut feeding_stop = stop.clone();
    let skip = source.as_ref().map_or(0, |source| source.skip);
    if let Some(source) = source {
        let mut input = input.context("Missing decoder input")?;
        tasks.spawn(async move {
            // Later IDRs need not repeat SPS/PPS. Prime from validated AU0,
            // discard that decoded picture, then seek to the preceding IDR.
            let (source, primer) = tokio::task::spawn_blocking(move || {
                let mut source = source;
                let primer = if source.start == 0 { None } else { Some(source.source.read_unit(0)?) };
                Ok::<_, anyhow::Error>((source, primer))
            }).await??;
            let mut reader = MediaReader::spawn_at(source.source, source.start);
            let result: Result<()> = async {
                if let Some(primer) = primer {
                    tokio::select! {
                        biased;
                        _ = cancelled(&mut feeding_stop) => return Ok(()),
                        result = input.write_all(&primer.bytes) => result.context("decoder parameter primer")?,
                    }
                }
                loop {
                    let unit = tokio::select! {
                        biased;
                        _ = cancelled(&mut feeding_stop) => return Ok(()),
                        unit = reader.next_unit() => unit?,
                    };
                    tokio::select! {
                        biased;
                        _ = cancelled(&mut feeding_stop) => return Ok(()),
                        written = input.write_all(&unit.bytes) => written.context("decoder input")?,
                    }
                }
            }
            .await;
            reader.stop().await?;
            result
        });
    }
    let mut rendering_stop = stop.clone();
    tasks.spawn(async move {
        // Warm decoder reference pictures without rendering, encoding or spending
        // output credits. Never expose these older frames to the consumer.
        if let Some(decoded) = &mut decoded {
            let mut discarded = vec![0; FRAME_BYTES];
            for _ in 0..skip {
                tokio::select! {
                    biased;
                    _ = cancelled(&mut rendering_stop) => return Ok(()),
                    result = decoded.read_exact(&mut discarded) => { result.context("decoder seek warmup")?; },
                }
            }
        }
        let mut frame_index = 0u64;
        loop {
            if *rendering_stop.borrow() { return Ok(()); }
            tokio::select! {
                biased;
                _=cancelled(&mut rendering_stop)=>return Ok(()),
                permit=credits.clone().acquire_owned()=>permit.context("composition frame credits closed")?.forget(),
            }
            let bytes = if let Some(decoded) = &mut decoded {
                let mut bytes = vec![0; FRAME_BYTES];
                tokio::select! {
                    biased;
                    _ = cancelled(&mut rendering_stop) => return Ok(()),
                    result = decoded.read_exact(&mut bytes) => { result.context("decoder frame")?; },
                }
                Some(bytes)
            } else { None };
            let renderer = runtime.clone();
            let elapsed_millis = frame_index.saturating_mul(1000) / 24;
            frame_index = frame_index.saturating_add(1);
            // Never cancel the blocking render: retain ownership until it returns.
            let frame = tokio::task::spawn_blocking(move || {
                let frame = bytes.map(|bytes| RgbImage::from_raw(480,480,bytes).context("invalid decoded frame")).transpose()?;
                renderer
                    .compose_frame_at_millis(frame, elapsed_millis)
                    .map_err(anyhow::Error::from)
            })
            .await??;
            tokio::select! {
                biased;
                _ = cancelled(&mut rendering_stop) => return Ok(()),
                result = raw.write_all(frame.as_raw()) => result.context("encoder input")?,
            }
        }
    });
    let mut encoding_stop = stop.clone();
    tasks.spawn(async move {
        let mut splitter = AudSplitter::default();
        let mut bytes = [0u8; 8192];
        loop {
            let count = tokio::select! {
                biased;
                _ = cancelled(&mut encoding_stop) => return Ok(()),
                read = encoded.read(&mut bytes) => read.context("encoder output")?,
            };
            if count == 0 { bail!("composition encoder closed unexpectedly"); }
            splitter.push(&bytes[..count])?;
            while let Some(unit) = splitter.next()? {
                tokio::select! {
                    biased;
                    _ = cancelled(&mut encoding_stop) => return Ok(()),
                    result = send.send(Ok(unit)) => { result.context("composition receiver closed")?; },
                }
            }
        }
    });
    let outcome = tokio::select! {
        biased;
        _ = cancelled(stop) => Ok(()),
        result = tasks.join_next() => match result {
            Some(Ok(result)) => result,
            Some(Err(error)) => Err(anyhow::anyhow!("composition task panicked: {error}")),
            None => Err(anyhow::anyhow!("composition has no tasks")),
        },
    };
    cancel.send_replace(true);
    // Kill both sides before joining pipe tasks: blocked pipe IO must wake.
    if let Some(decoder) = &mut decoder {
        let _ = decoder.start_kill();
    }
    let _ = encoder.start_kill();
    while tasks.join_next().await.is_some() {}
    let decoder_result = if let Some(decoder) = &mut decoder {
        Some(decoder.wait().await)
    } else {
        None
    };
    let encoder_result = encoder.wait().await;
    if let Some(result) = decoder_result {
        result.context("decoder reap")?;
    }
    encoder_result.context("encoder reap")?;
    outcome
}

/// Encoder-local Annex-B framing. AUD is explicitly enabled; a following AUD
/// delimits the preceding AU. This is not a general H.264 bitstream parser.
#[derive(Default)]
struct AudSplitter {
    bytes: Vec<u8>,
    emitted: bool,
}
impl AudSplitter {
    fn push(&mut self, bytes: &[u8]) -> Result<()> {
        // One maximum AU plus one read chunk and its incomplete prefix.
        if self.bytes.len() + bytes.len() > MAX_AU + 8192 + 4 {
            bail!("encoded AU exceeds budget");
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn next(&mut self) -> Result<Option<H264AccessUnit>> {
        let starts: Vec<usize> = self
            .bytes
            .windows(4)
            .enumerate()
            .filter(|(_, w)| w[..3] == [0, 0, 1] && w[3] & 31 == 9)
            .map(|(index, _)| {
                if index > 0 && self.bytes[index - 1] == 0 {
                    index - 1
                } else {
                    index
                }
            })
            .take(2)
            .collect();
        if let Some(&start) = starts.first() {
            if start != 0 {
                bail!("encoder stream must begin with AUD");
            }
        }
        let Some(&end) = starts.get(1) else {
            if self.bytes.len() > MAX_AU + 4 {
                bail!("encoded AU exceeds budget");
            }
            return Ok(None);
        };
        if end > MAX_AU {
            bail!("encoded AU exceeds budget");
        }
        let rest = self.bytes.split_off(end);
        let bytes = std::mem::replace(&mut self.bytes, rest);
        let keyframe = bytes
            .windows(4)
            .any(|w| w[..3] == [0, 0, 1] && w[3] & 31 == 5);
        let unit = H264AccessUnit { bytes, keyframe };
        validate_h264_access_unit(&unit, !self.emitted)?;
        self.emitted = true;
        Ok(Some(unit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{media_assets::MediaAssetStore, sensors::SensorCache};
    use lconnect_image::theme::ThemeAssets;
    use lconnect_ipc::{H264Clip, LcdTheme, SensorReading, SensorStatus, SensorUnit};
    use std::os::unix::fs::PermissionsExt;

    fn theme() -> LcdTheme {
        serde_json::from_value(serde_json::json!({"version":1,"name":"Live video","background":[0,0,0],"layers":[
            {"kind":"sensor","sensor_id":"cpu","label":"CPU","unit":"celsius","temperature_unit":"celsius","decimals":1,
             "style":{"x":240,"y":200,"size":48,"color":[255,255,255],"align":"center"}}
        ]})).unwrap()
    }
    fn update(cache: &SensorCache, value: f64) {
        cache.publish(
            std::time::Instant::now(),
            vec![SensorReading {
                id: "cpu".into(),
                label: "CPU".into(),
                unit: SensorUnit::Celsius,
                value: Some(value),
                status: SensorStatus::Valid,
                observed_unix_ms: None,
                age_ms: Some(0),
            }],
        );
    }
    fn source() -> (tempfile::TempDir, MediaAssetStore, String, PinnedMedia) {
        source_with_size("480x480")
    }
    fn source_with_size(size: &str) -> (tempfile::TempDir, MediaAssetStore, String, PinnedMedia) {
        source_with_filter(&format!("color=c=blue:s={size}:r=24"))
    }
    fn source_with_filter(
        filter: &str,
    ) -> (tempfile::TempDir, MediaAssetStore, String, PinnedMedia) {
        source_with_filter_gop(filter, "1")
    }
    fn source_with_filter_gop(
        filter: &str,
        gop: &str,
    ) -> (tempfile::TempDir, MediaAssetStore, String, PinnedMedia) {
        source_with_filter_video(filter, gop, "4")
    }
    fn source_with_filter_video(
        filter: &str,
        gop: &str,
        frame_count: &str,
    ) -> (tempfile::TempDir, MediaAssetStore, String, PinnedMedia) {
        let root = tempfile::tempdir().unwrap();
        let encoded = root.path().join("input.h264");
        let output = std::process::Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                filter,
                "-frames:v",
                frame_count,
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "zerolatency",
                "-profile:v",
                "baseline",
                "-pix_fmt",
                "yuv420p",
                "-g",
                gop,
                "-threads",
                "1",
            ])
            .arg(&encoded)
            .output()
            .unwrap();
        assert!(output.status.success());
        let report = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "packet=pos,size,flags",
                "-of",
                "json",
            ])
            .arg(&encoded)
            .output()
            .unwrap();
        assert!(report.status.success());
        let packets: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
        let bytes = std::fs::read(encoded).unwrap();
        let units = packets["packets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|packet| {
                let start: usize = packet["pos"].as_str().unwrap().parse().unwrap();
                let size: usize = packet["size"].as_str().unwrap().parse().unwrap();
                H264AccessUnit {
                    bytes: bytes[start..start + size].to_vec(),
                    keyframe: packet["flags"].as_str().unwrap().contains('K'),
                }
            })
            .collect();
        let store = MediaAssetStore::new(root.path().join("media"));
        let key = store
            .import("blue".into(), H264Clip { fps: 24, units })
            .unwrap()
            .key;
        let source = store.acquire_source(&key).unwrap();
        (root, store, key, source)
    }
    #[tokio::test]
    async fn preview_uses_live_cache_and_requested_cycle_position_then_releases_pin() {
        let (_root, store, key, source) = tokio::task::spawn_blocking(source).await.unwrap();
        drop(source);
        let cache = SensorCache::default();
        update(&cache, 40.0);
        let mut value = serde_json::to_value(theme()).unwrap();
        let style = value["layers"][0]["style"].clone();
        value["version"] = 2.into();
        value["background_video"] = key.clone().into();
        value["layers"] = serde_json::json!([{"kind":"sensor_cycle","interval_seconds":2,"style":style,
        "sensors":[
            {"sensor_id":"cpu","label":"CPU","unit":"celsius","temperature_unit":"celsius","decimals":1},
            {"sensor_id":"cpu","label":"Second","unit":"celsius","temperature_unit":"celsius","decimals":1}
        ]}]);
        let runtime = Arc::new(
            ThemeRuntime::new(
                serde_json::from_value(value).unwrap(),
                ThemeAssets::new(),
                vec![cache.clone()],
            )
            .unwrap(),
        );
        let first = preview(store.clone(), runtime.clone(), 0).await.unwrap();
        let second = preview(store.clone(), runtime.clone(), 2).await.unwrap();
        let editor = editor_preview(store.clone(), runtime.clone(), 2)
            .await
            .unwrap();
        assert_eq!(editor.jpeg, second);
        assert_eq!(editor.layers.len(), 1);
        assert!(editor.layers[0].is_some());
        assert!(first != second, "preview ignored cycle position");
        update(&cache, 80.0);
        let changed = preview(store.clone(), runtime, 0).await.unwrap();
        assert!(first != changed, "preview ignored updated reading");
        store.delete_unreferenced(&key, || Ok(false)).unwrap();
    }

    #[tokio::test]
    async fn preview_timeout_and_output_budget_reap_decoder_before_returning() {
        for oversized in [false, true] {
            let (root, _store, _key, mut source) =
                tokio::task::spawn_blocking(source).await.unwrap();
            let unit = source.read_unit(0).unwrap();
            let script = root.path().join("bad-preview-codec");
            let pid_path = root.path().join("pid");
            let behavior = if oversized {
                "os.write(1,b'x'*700000)"
            } else {
                "time.sleep(60)"
            };
            std::fs::write(&script,format!("#!/usr/bin/python3\nimport os,time\nwith open({:?},'w') as f: f.write(str(os.getpid()))\n{behavior}\n",pid_path.to_str().unwrap())).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
            let error = decode_preview_frame(unit, script.as_os_str(), Duration::from_secs(1))
                .await
                .unwrap_err();
            if oversized {
                assert!(error.to_string().contains("frame size"), "{error:#}");
            } else {
                assert!(error.to_string().contains("timed out"), "{error:#}");
            }
            let pid = std::fs::read_to_string(pid_path).unwrap();
            assert!(
                !std::path::Path::new(&format!("/proc/{pid}")).exists(),
                "decoder not reaped"
            );
        }
    }

    #[tokio::test]
    async fn real_pipeline_loops_updates_sensors_and_releases_pin_after_stop() {
        let (root, store, key, source) = tokio::task::spawn_blocking(source).await.unwrap();
        let cache = SensorCache::default();
        update(&cache, 40.0);
        let runtime =
            Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![cache.clone()]).unwrap());
        let mut worker = MediaComposition::spawn(source, runtime);
        let mut output = Vec::new();
        for frame in 0..60 {
            if frame == 20 {
                update(&cache, 80.0);
            }
            let unit = worker.next_unit().await.unwrap();
            validate_h264_access_unit(&unit, frame == 0).unwrap();
            output.extend_from_slice(&unit.bytes);
        }
        assert!(store.delete_unreferenced(&key, || Ok(false)).is_err());
        tokio::time::timeout(Duration::from_secs(3), worker.stop())
            .await
            .unwrap()
            .unwrap();
        worker.stop().await.unwrap();
        assert!(worker.next_unit().await.is_err());
        store.delete_unreferenced(&key, || Ok(false)).unwrap();
        tokio::task::spawn_blocking(move || {
            let encoded = root.path().join("output.h264");
            std::fs::write(&encoded, output).unwrap();
            let decoded = std::process::Command::new("ffmpeg")
                .args(["-nostdin", "-v", "error", "-i"])
                .arg(&encoded)
                .args(["-threads", "1", "-pix_fmt", "rgb24", "-f", "rawvideo", "-"])
                .output()
                .unwrap();
            assert!(decoded.status.success());
            assert_eq!(decoded.stdout.len(), FRAME_BYTES * 60);
            let first = &decoded.stdout[..FRAME_BYTES];
            let last = &decoded.stdout[59 * FRAME_BYTES..];
            assert!(
                first.chunks_exact(3).any(|p| p.iter().all(|v| *v > 200)),
                "sensor text missing"
            );
            let changed = first
                .chunks_exact(3)
                .zip(last.chunks_exact(3))
                .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 40))
                .count();
            assert!(
                changed > 30,
                "live sensor change did not reach encoded frames: {changed}"
            );
            assert!(
                first[2] > 220 && first[0] < 20 && first[1] < 20,
                "video background replaced"
            );
        })
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn oversized_decode_fails_before_timeout_and_releases_source() {
        // The store validates AU framing, not decoded dimensions. Exercise the
        // real decoder allocation limit with a valid but oversized bitstream.
        let (_root, store, key, source) =
            tokio::task::spawn_blocking(|| source_with_size("1024x1024"))
                .await
                .unwrap();
        let runtime = Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![]).unwrap());
        let mut worker = MediaComposition::spawn(source, runtime);
        let error = tokio::time::timeout(Duration::from_secs(3), worker.next_unit())
            .await
            .expect("decoder should fail promptly")
            .unwrap_err();
        assert!(!error.to_string().contains("timed out"), "{error:#}");
        worker.stop().await.unwrap();
        store.delete_unreferenced(&key, || Ok(false)).unwrap();
    }

    #[tokio::test]
    async fn stop_kills_and_reaps_blocked_children_before_releasing_source() {
        for dropped in [false, true] {
            let (root, store, key, source) = tokio::task::spawn_blocking(source).await.unwrap();
            let script = root.path().join("stalled-codec");
            let pids = root.path().join("pids");
            // No shell subprocess: the deliberately stalled child itself is killed/reaped.
            std::fs::write(&script, format!("#!/usr/bin/python3\nimport os,time\nwith open({:?},'a') as f: f.write(str(os.getpid())+'\\n')\ntime.sleep(60)\n", pids.to_str().unwrap())).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
            let runtime = Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![]).unwrap());
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let permit = slots.clone().acquire_owned().await.unwrap();
            let mut worker = MediaComposition::spawn_owned(
                Some(source),
                runtime,
                script.into_os_string(),
                Some(permit.into()),
            );
            let ids = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Ok(text) = tokio::fs::read_to_string(&pids).await {
                        let ids: Vec<_> = text.lines().map(str::to_owned).collect();
                        if ids.len() == 2 {
                            break ids;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert!(store.delete_unreferenced(&key, || Ok(false)).is_err());
            if dropped {
                drop(worker);
                // Current-thread runtime cannot run the supervisor between these
                // synchronous operations; the lease must still belong to cleanup.
                assert!(slots.try_acquire().is_err());
                let _released = tokio::time::timeout(Duration::from_secs(3), slots.acquire())
                    .await
                    .unwrap()
                    .unwrap();
                for pid in &ids {
                    assert!(
                        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
                        "slot released before reap"
                    );
                }
                store.delete_unreferenced(&key, || Ok(false)).unwrap();
                continue;
            }
            let deadline = tokio::time::Instant::now() + Duration::from_millis(50);
            worker.receive_deadline = Some(deadline);
            assert!(
                tokio::time::timeout(Duration::from_millis(5), worker.next_unit())
                    .await
                    .is_err()
            );
            tokio::time::sleep_until(deadline).await;
            let error = tokio::time::timeout(Duration::from_millis(100), worker.next_unit())
                .await
                .expect("cancelled receive must retain its timeout")
                .unwrap_err();
            assert!(error.to_string().contains("timed out"));
            tokio::time::timeout(Duration::from_secs(3), worker.stop())
                .await
                .unwrap()
                .unwrap();
            for pid in ids {
                assert!(
                    !std::path::Path::new(&format!("/proc/{pid}")).exists(),
                    "child not reaped"
                );
            }
            assert!(slots.try_acquire().is_ok());
            store.delete_unreferenced(&key, || Ok(false)).unwrap();
        }
    }
    #[tokio::test]
    async fn codec_faults_reap_both_children_and_release_preparation_lease() {
        for (fault, expected) in [
            ("truncated", "decoder frame"),
            ("malformed", "encoder stream must begin with AUD"),
            ("oversize", "encoded AU exceeds budget"),
            ("closed", "composition encoder closed unexpectedly"),
        ] {
            let (root, store, key, source) = tokio::task::spawn_blocking(source).await.unwrap();
            let script = root.path().join("fault-codec");
            let pids = root.path().join("pids");
            // Each child records its PID before either produces a fault. The
            // non-failing peer deliberately blocks so cleanup must terminate it.
            std::fs::write(
                &script,
                format!(
                    r#"#!/usr/bin/python3
import os,sys,time,pathlib
pids=pathlib.Path({:?})
with pids.open('a') as f: f.write(str(os.getpid())+'\n')
while len(pids.read_text().splitlines()) < 2: time.sleep(.01)
encoder='-pixel_format' in sys.argv
fault={fault:?}
if not encoder and fault=='truncated':
    os.write(1,b'partial RGB frame')
    os.close(1)  # Keep stdin open: only decoder-frame EOF should win.
if encoder:
    if fault=='malformed': os.write(1,b'bad'+bytes([0,0,1,9,240]))
    if fault=='oversize':
        for _ in range(20): os.write(1,b'x'*8192)
    if fault=='closed': sys.exit(0)
time.sleep(60)
"#,
                    pids.to_str().unwrap()
                ),
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
            let runtime = Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![]).unwrap());
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let permit = slots.clone().acquire_owned().await.unwrap();
            let mut worker = MediaComposition::spawn_owned(
                Some(source),
                runtime,
                script.into_os_string(),
                Some(permit.into()),
            );
            let error = tokio::time::timeout(Duration::from_secs(3), worker.next_unit())
                .await
                .expect("fault must reach consumer before frame timeout")
                .unwrap_err();
            assert!(
                format!("{error:#}").contains(expected),
                "{fault}: {error:#}"
            );
            tokio::time::timeout(Duration::from_secs(3), worker.stop())
                .await
                .unwrap()
                .unwrap();
            let ids = std::fs::read_to_string(&pids).unwrap();
            assert_eq!(ids.lines().count(), 2);
            for pid in ids.lines() {
                assert!(
                    !std::path::Path::new(&format!("/proc/{pid}")).exists(),
                    "{fault}: child not reaped"
                );
            }
            assert!(
                slots.try_acquire().is_ok(),
                "{fault}: lease retained after stop"
            );
            store.delete_unreferenced(&key, || Ok(false)).unwrap();
            assert!(worker
                .next_unit()
                .await
                .unwrap_err()
                .to_string()
                .contains("stopped"));
        }
    }

    #[tokio::test]
    async fn stop_drains_full_real_codec_queue_without_waiting_for_consumer() {
        let (_root, store, key, source) = tokio::task::spawn_blocking(source).await.unwrap();
        let runtime = Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![]).unwrap());
        let mut worker = MediaComposition::spawn(source, runtime);
        tokio::time::timeout(Duration::from_secs(3), async {
            while worker.frames.len() != PREFETCH {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("real encoder must fill the bounded queue");
        assert!(store.delete_unreferenced(&key, || Ok(false)).is_err());
        // No receive drains the queue: Stop must unblock a producer on its own.
        tokio::time::timeout(Duration::from_secs(3), worker.stop())
            .await
            .unwrap()
            .unwrap();
        assert!(worker.frames.is_empty());
        assert!(worker.supervisor.is_none());
        store.delete_unreferenced(&key, || Ok(false)).unwrap();
        worker.stop().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "30-second software acceptance; run alone in release mode with --ignored --nocapture"]
    async fn composition_paced_acceptance() {
        use std::collections::BTreeMap;
        fn getconf(name: &str) -> u64 {
            let out = std::process::Command::new("getconf")
                .arg(name)
                .output()
                .unwrap();
            assert!(out.status.success());
            std::str::from_utf8(&out.stdout)
                .unwrap()
                .trim()
                .parse()
                .unwrap()
        }
        fn resources(page_size: u64) -> BTreeMap<u32, (u64, u64)> {
            let mut ids = std::collections::BTreeSet::from([std::process::id()]);
            for task in std::fs::read_dir("/proc/self/task").unwrap() {
                let path = task.unwrap().path().join("children");
                if let Ok(children) = std::fs::read_to_string(path) {
                    ids.extend(
                        children
                            .split_whitespace()
                            .map(|id| id.parse::<u32>().unwrap()),
                    );
                }
            }
            ids.into_iter()
                .filter_map(|pid| {
                    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
                    let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
                    let ticks =
                        fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();
                    let rss = fields[21].parse::<u64>().unwrap() * page_size;
                    Some((pid, (ticks, rss)))
                })
                .collect()
        }
        let (root, store, key, source) = tokio::task::spawn_blocking(source).await.unwrap();
        let cache = SensorCache::default();
        update(&cache, 40.0);
        let mut composition = theme();
        composition.version = 2;
        composition.background_video = Some(key.clone());
        composition.layers.push(
            serde_json::from_value(serde_json::json!({
                "kind":"clock","twenty_four_hour":true,"show_seconds":true,
                "style":{"x":240,"y":300,"size":40,"color":[255,255,255],"align":"center"}
            }))
            .unwrap(),
        );
        let runtime = Arc::new(
            ThemeRuntime::new(composition, ThemeAssets::new(), vec![cache.clone()]).unwrap(),
        );
        let hz = getconf("CLK_TCK");
        let page_size = getconf("PAGESIZE");
        let mut last = resources(page_size);
        let mut cpu_ticks = 0;
        let mut peak_rss = 0;
        let mut peak_children = 0;
        let mut samples = Vec::new();
        let started = tokio::time::Instant::now();
        let mut worker = MediaComposition::spawn(source, runtime);
        let first = worker.next_unit().await.unwrap();
        let startup_ms = started.elapsed().as_secs_f64() * 1000.0;
        let mut output = first.bytes;
        let paced_start = tokio::time::Instant::now();
        let period = Duration::from_secs_f64(1.0 / 24.0);
        let mut waits = Vec::new();
        let mut late_frames = 0;
        let mut max_queue = 0;
        for frame in 1..720_u32 {
            let due = paced_start + period * frame;
            tokio::time::sleep_until(due).await;
            if frame.is_multiple_of(24) {
                update(&cache, if frame < 360 { 40.0 } else { 80.0 });
            }
            max_queue = max_queue.max(worker.frames.len());
            let receive = tokio::time::Instant::now();
            let unit = worker.next_unit().await.unwrap();
            waits.push(receive.elapsed().as_secs_f64() * 1000.0);
            late_frames += usize::from(tokio::time::Instant::now() > due + period);
            output.extend_from_slice(&unit.bytes);
            if frame.is_multiple_of(24) || frame == 719 {
                let current = resources(page_size);
                for (pid, (ticks, _)) in &current {
                    cpu_ticks += ticks.saturating_sub(last.get(pid).map_or(0, |v| v.0));
                }
                let rss = current.values().map(|v| v.1).sum::<u64>();
                peak_rss = peak_rss.max(rss);
                peak_children = peak_children.max(current.len() - 1);
                samples
                    .push(serde_json::json!({"frame":frame,"rss_bytes":rss,"cpu_ticks":cpu_ticks}));
                last = current;
                if frame.is_multiple_of(120) {
                    eprintln!("composition acceptance: {frame}/720 frames");
                }
            }
        }
        let elapsed = started.elapsed().as_secs_f64();
        worker.stop().await.unwrap();
        store.delete_unreferenced(&key, || Ok(false)).unwrap();
        waits.sort_by(f64::total_cmp);
        let encoded = root.path().join("paced.h264");
        std::fs::write(&encoded, output).unwrap();
        let decoded = tokio::task::spawn_blocking(move || {
            let result = std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(encoded)
                .args([
                    "-vf",
                    "select=eq(n\\,0)+eq(n\\,719)",
                    "-fps_mode",
                    "passthrough",
                    "-threads",
                    "1",
                    "-pix_fmt",
                    "rgb24",
                    "-f",
                    "rawvideo",
                    "-",
                ])
                .output()
                .unwrap();
            assert!(result.status.success());
            result.stdout
        })
        .await
        .unwrap();
        assert_eq!(decoded.len(), FRAME_BYTES * 2, "720th frame must decode");
        // Restrict comparison to the sensor region, excluding the clock below it.
        let changed = (170..260)
            .flat_map(|y| (50..430).map(move |x| (y * 480 + x) * 3))
            .filter(|i| {
                decoded[*i..*i + 3]
                    .iter()
                    .zip(&decoded[FRAME_BYTES + *i..FRAME_BYTES + *i + 3])
                    .any(|(a, b)| a.abs_diff(*b) > 40)
            })
            .count();
        assert!(changed > 30, "sensor update did not reach paced output");
        assert!(max_queue <= PREFETCH);
        assert_eq!(
            peak_children, 2,
            "resource sampling must include both codecs"
        );
        let report = serde_json::json!({
            "frames":720,"requested_fps":24,"startup_ms":startup_ms,"elapsed_seconds":elapsed,
            "receive_wait_p95_ms":waits[waits.len()*95/100],"receive_wait_max_ms":waits.last(),
            "frames_over_one_period_late":late_frames,"max_output_queue":max_queue,
            "cpu_seconds_parent_and_codecs":cpu_ticks as f64/hz as f64,
            "cpu_percent_one_core":100.0*cpu_ticks as f64/hz as f64/elapsed,
            "peak_sampled_rss_parent_and_codecs_bytes":peak_rss,"resource_samples":samples,
            "sensor_pixels_changed":changed,"hardware":false,"release":!cfg!(debug_assertions),
            "scope":"paced consumer, sensor+clock; excludes actor/HID, post-run decode and teardown resources"
        });
        println!("COMPOSITION_REPORT={report}");
    }

    #[test]
    fn aud_splitter_handles_every_byte_boundary_and_rejects_oversize() {
        let first = [
            0, 0, 0, 1, 9, 0xf0, 0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1,
        ];
        let next = [0, 0, 1, 9, 0xf0, 0, 0, 1, 1, 1];
        for chunk in 1..=16 {
            let bytes = [first.as_slice(), next.as_slice(), next.as_slice()].concat();
            let mut splitter = AudSplitter::default();
            let mut units = Vec::new();
            for part in bytes.chunks(chunk) {
                splitter.push(part).unwrap();
                while let Some(unit) = splitter.next().unwrap() {
                    units.push(unit);
                }
            }
            assert_eq!(units.len(), 2);
            assert_eq!(units[0].bytes, first);
            assert_eq!(units[1].bytes, next);
            assert!(units[0].keyframe);
            assert!(!units[1].keyframe);
        }
        assert!(AudSplitter::default()
            .push(&vec![0; MAX_AU + 8197])
            .is_err());
        let mut malformed = AudSplitter::default();
        malformed.push(&[1, 2, 3, 0, 0, 1, 9, 0xf0]).unwrap();
        assert!(malformed.next().is_err());
    }
    #[tokio::test]
    async fn procedural_weather_encodes_without_media_and_live_sensor_changes_reach_h264() {
        let root = tempfile::tempdir().unwrap();
        let store = MediaAssetStore::new(root.path().join("media"));
        let cache = SensorCache::default();
        update(&cache, 20.0);
        let config:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":"Snow","background":[0,0,0],"layers":[{"kind":"weather","effect":{"kind":"snow","intensity":1,"seed":42,"dynamic":true,"sensor":{"sensor_id":"cpu","unit":"celsius","low":20,"high":80}}}]})).unwrap();
        let runtime =
            Arc::new(ThemeRuntime::new(config, ThemeAssets::new(), vec![cache.clone()]).unwrap());
        let mut prepared = PreparedComposition::prepare(store, runtime).await.unwrap();
        assert!(prepared.pin.is_none());
        let first = decode_preview_frame(
            prepared.first.clone(),
            OsStr::new("ffmpeg"),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        update(&cache, 90.0);
        let mut last = None;
        for _ in 0..48 {
            let unit = prepared.worker.next_unit().await.unwrap();
            if unit.keyframe {
                last = Some(unit);
            }
        }
        prepared.worker.stop().await.unwrap();
        let last =
            decode_preview_frame(last.unwrap(), OsStr::new("ffmpeg"), Duration::from_secs(5))
                .await
                .unwrap();
        let lit = |image: &RgbImage| {
            image
                .pixels()
                .filter(|p| p.0.iter().all(|v| *v > 60))
                .count()
        };
        assert!(lit(&first) > 100, "Snow absent from first H264 frame");
        assert!(
            lit(&last) > lit(&first) * 2,
            "Live dynamic high density did not reach encoded frames"
        );
    }
    #[tokio::test]
    async fn direct_media_handoff_warms_reference_frames_without_rewind_and_keeps_rollback_reader()
    {
        let filter = "nullsrc=s=480x480:r=24,format=rgb24,geq=r='32+24*N':g=0:b=0";
        let (root, store, _key, mut source) =
            tokio::task::spawn_blocking(move || source_with_filter_video(filter, "4", "8"))
                .await
                .unwrap();
        let mut units = Vec::new();
        for index in 0..source.frames() {
            let mut unit = source.read_unit(index).unwrap();
            if index != 0 {
                let mut filtered = Vec::new();
                let mut offset = 0;
                while offset < unit.bytes.len() {
                    let start = if unit.bytes[offset..].starts_with(&[0, 0, 0, 1]) {
                        4
                    } else {
                        3
                    };
                    let header = offset + start;
                    let mut end = header + 1;
                    while end < unit.bytes.len()
                        && !unit.bytes[end..].starts_with(&[0, 0, 1])
                        && !unit.bytes[end..].starts_with(&[0, 0, 0, 1])
                    {
                        end += 1;
                    }
                    if ![7, 8].contains(&(unit.bytes[header] & 31)) {
                        filtered.extend_from_slice(&unit.bytes[offset..end]);
                    }
                    offset = end;
                }
                unit.bytes = filtered;
            }
            units.push(unit);
        }
        drop(source);
        assert!(
            validate_h264_access_unit(&units[4], true).is_err(),
            "later IDR must lack repeated SPS/PPS in this fixture"
        );
        let mut writer = store.create_file("Headers only at start".into()).unwrap();
        for unit in &units {
            writer.append(unit).unwrap();
        }
        let info = store.publish_file(writer).unwrap();
        let source = store.acquire_source(&info.key).unwrap();
        let mut direct = MediaReader::spawn(source);
        for _ in 0..5 {
            direct.next_unit().await.unwrap();
        }
        assert_eq!(direct.next_index(), 5);
        let source = direct.take_source().await.unwrap();
        assert!(direct.next_unit().await.is_err());
        assert_eq!(
            source.keyframe_before(5).unwrap(),
            4,
            "exercise reference-frame warmup, not IDR-only seek"
        );
        let backup = source.try_clone().unwrap();
        let runtime = Arc::new(
            ThemeRuntime::new(theme(), ThemeAssets::new(), vec![])
                .unwrap()
                .with_output_rotation(lconnect_image::FrameRotation::try_from(90).unwrap()),
        );
        let mut worker = MediaComposition::spawn_at(source, runtime, 5).unwrap();
        // Exercise independent positional reads concurrently with the decoder's
        // source reader, rather than relying on shared cloned-file seek offsets.
        let mut rollback = MediaReader::spawn_at(backup, 5);
        let mut bytes = Vec::new();
        for index in 0_usize..8 {
            let (encoded, original) = tokio::join!(worker.next_unit(), rollback.next_unit());
            bytes.extend_from_slice(&encoded.unwrap().bytes);
            let original = original.unwrap();
            assert_eq!(original.keyframe, (index + 5).is_multiple_of(4));
            assert_eq!(
                original.bytes,
                units[(index + 5) % 8].bytes,
                "cloned fd readers must retain independent positions"
            );
        }
        worker.stop().await.unwrap();
        rollback.stop().await.unwrap();
        let input = root.path().join("handoff.h264");
        std::fs::write(&input, bytes).unwrap();
        let decoded = tokio::task::spawn_blocking(move || {
            std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-threads", "1", "-i"])
                .arg(input)
                .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        let frames: Vec<_> = decoded.stdout.chunks_exact(FRAME_BYTES).collect();
        assert_eq!(frames.len(), 8);
        for (index, frame) in frames.iter().enumerate() {
            let expected = 32 + 24 * ((index + 5) % 8) as u8;
            let actual = frame[(10 * 480 + 470) * 3];
            assert!(
                actual.abs_diff(expected) < 15,
                "handoff rewound/skipped at {index}: {actual} vs {expected}"
            );
        }
    }

    #[tokio::test]
    async fn live_rotation_preserves_video_phase_and_bounds_stale_frames() {
        let filter = "nullsrc=s=480x480:r=24,format=rgb24,geq=r='if(lt(X,240)*lt(Y,240),32+32*N,if(gte(X,240)*gte(Y,240),240,0))':g='if(gte(X,240),200,0)':b='if(lt(X,240)*gte(Y,240),240,0)'";
        let (root, _store, _key, source) =
            tokio::task::spawn_blocking(move || source_with_filter(filter))
                .await
                .unwrap();
        let runtime = Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![]).unwrap());
        let mut worker = MediaComposition::spawn(source, runtime.clone());
        let mut bytes = Vec::new();
        for index in 0..64 {
            if let Some(degrees) = match index {
                5 => Some(90),
                17 => Some(180),
                29 => Some(270),
                41 => Some(0),
                _ => None,
            } {
                // A stalled consumer used to allow OS/encoder buffers to accumulate
                // arbitrarily many old-angle frames beyond the output mpsc queue.
                tokio::time::sleep(Duration::from_millis(100)).await;
                runtime
                    .set_output_rotation(lconnect_image::FrameRotation::try_from(degrees).unwrap());
            }
            bytes.extend_from_slice(&worker.next_unit().await.unwrap().bytes);
        }
        worker.stop().await.unwrap();
        let input = root.path().join("live.h264");
        std::fs::write(&input, bytes).unwrap();
        let decoded = tokio::task::spawn_blocking(move || {
            std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-threads", "1", "-i"])
                .arg(input)
                .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        let frames: Vec<_> = decoded.stdout.chunks_exact(480 * 480 * 3).collect();
        assert_eq!(frames.len(), 64);
        let corners = [(10, 10), (470, 10), (10, 470), (470, 470)];
        let mut previous_phase = None;
        for (index, frame) in frames.into_iter().enumerate() {
            let (corner, red) = corners
                .iter()
                .enumerate()
                .find_map(|(corner, &(x, y))| {
                    let p = &frame[(y * 480 + x) * 3..][..3];
                    (p[0] > 15 && p[0] < 150 && p[1] < 25 && p[2] < 25).then_some((corner, p[0]))
                })
                .expect("temporal red marker must survive encoding and rotation");
            let phase = [32u8, 64, 96, 128]
                .iter()
                .enumerate()
                .min_by_key(|(_, value)| value.abs_diff(red))
                .unwrap()
                .0;
            if let Some(previous) = previous_phase {
                assert_eq!(
                    phase,
                    (previous + 1) % 4,
                    "video rewound/skipped at frame {index}"
                );
            }
            previous_phase = Some(phase);
            let (changed_at, expected_corner) = match index {
                0..=4 => (0, 0),
                5..=16 => (5, 1),
                17..=28 => (17, 3),
                29..=40 => (29, 2),
                _ => (41, 0),
            };
            if changed_at == 0 || index >= changed_at + PIPELINE_FRAMES + 2 {
                assert_eq!(
                    corner, expected_corner,
                    "unbounded stale rotation at frame {index}"
                );
            }
        }
    }

    #[tokio::test]
    async fn h264_rotation_turns_decoded_video_and_live_sensor_overlay_together() {
        let filter="color=c=red:s=480x480:r=24,drawbox=x=240:y=0:w=240:h=240:c=green:t=fill,drawbox=x=0:y=240:w=240:h=240:c=blue:t=fill,drawbox=x=240:y=240:w=240:h=240:c=yellow:t=fill";
        let (_root, store, key, source) =
            tokio::task::spawn_blocking(move || source_with_filter(filter))
                .await
                .unwrap();
        drop(source);
        for (degrees, order, region) in [
            (0, [0, 1, 2, 3], (20, 190, 30, 150)),
            (90, [2, 0, 3, 1], (330, 450, 20, 190)),
            (180, [3, 2, 1, 0], (290, 460, 330, 450)),
            (270, [1, 3, 0, 2], (30, 150, 290, 460)),
        ] {
            let cache = SensorCache::default();
            update(&cache, 40.0);
            let mut value = serde_json::to_value(theme()).unwrap();
            value["layers"][0]["style"]["x"] = 40.into();
            value["layers"][0]["style"]["y"] = 60.into();
            value["layers"][0]["style"]["size"] = 36.into();
            value["layers"][0]["style"]["align"] = "left".into();
            let runtime = Arc::new(
                ThemeRuntime::new(
                    serde_json::from_value(value).unwrap(),
                    ThemeAssets::new(),
                    vec![cache.clone()],
                )
                .unwrap()
                .with_output_rotation(lconnect_image::FrameRotation::try_from(degrees).unwrap()),
            );
            let source = store.acquire_source(&key).unwrap();
            let mut worker = MediaComposition::spawn(source, runtime);
            let first = worker.next_unit().await.unwrap();
            let mut last = None;
            for frame in 1..40 {
                if frame == 8 {
                    update(&cache, 80.0);
                }
                let unit = worker.next_unit().await.unwrap();
                if unit.keyframe {
                    last = Some(unit);
                }
            }
            worker.stop().await.unwrap();
            let first = decode_preview_frame(first, OsStr::new("ffmpeg"), Duration::from_secs(5))
                .await
                .unwrap();
            let last =
                decode_preview_frame(last.unwrap(), OsStr::new("ffmpeg"), Duration::from_secs(5))
                    .await
                    .unwrap();
            let colors = [[250u8, 0, 0], [0, 125, 0], [0, 0, 250], [250, 250, 0]];
            for ((x, y), index) in [(10, 10), (470, 10), (10, 470), (470, 470)]
                .into_iter()
                .zip(order)
            {
                let actual = first.get_pixel(x, y).0;
                assert!(
                    actual
                        .iter()
                        .zip(colors[index])
                        .all(|(a, b)| a.abs_diff(b) < 25),
                    "rotation={degrees}, corner={x}/{y}: {actual:?}"
                );
            }
            let (x0, x1, y0, y1) = region;
            let white = first
                .enumerate_pixels()
                .filter(|(x, y, p)| {
                    *x >= x0 && *x < x1 && *y >= y0 && *y < y1 && p.0.iter().all(|v| *v > 200)
                })
                .count();
            assert!(
                white > 30,
                "rotated sensor text absent: rotation={degrees}, white={white}"
            );
            let changed = first
                .pixels()
                .zip(last.pixels())
                .filter(|(a, b)| a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 40))
                .count();
            assert!(
                changed > 20,
                "live sensor update absent: rotation={degrees}, changed={changed}"
            );
        }
    }
}
