//! Blocking recording engine. Call from an owned worker, never the UI executor.
//! Portal/session and GUI ownership are supplied by the caller; no default source.
use anyhow::{bail, ensure, Context, Result};
use gstreamer::{self as gst, prelude::*};
use std::{
    os::fd::{AsRawFd, OwnedFd},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tempfile::TempDir;

const FPS: u64 = 24;
const MAX_FRAMES: u64 = 180 * FPS;
const MAX_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Phase {
    Setup,
    Recording,
    Paused,
    LimitReached,
    Finished,
    Cancelled,
    Failed(String),
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Status {
    pub phase: Phase,
    pub frames: u64,
    pub active_millis: u64,
    pub region: Option<crate::media_prepare::VideoCrop>,
}
impl Status {
    pub fn recorded_millis(&self) -> u64 {
        self.active_millis
    }
}

/// Owns the finalized source until the caller finishes preparing/reviewing it.
#[derive(Debug)]
pub struct RecordingFile {
    directory: TempDir,
    length: u64,
    pub frames: u64,
}
impl RecordingFile {
    pub(crate) fn from_directory(directory: TempDir, frames: u64, length: u64) -> Self {
        Self {
            directory,
            frames,
            length,
        }
    }
    pub fn length(&self) -> u64 {
        self.length
    }
    pub fn path(&self) -> std::path::PathBuf {
        self.directory.path().join("recording.mp4")
    }
}

enum Source {
    Portal {
        fd: OwnedFd,
        node: u32,
        serial: Option<u64>,
        closed: tokio::sync::watch::Receiver<bool>,
    },
    #[cfg(any(test, feature = "native-smoke"))]
    Synthetic {
        closed: Option<tokio::sync::watch::Receiver<bool>>,
        sparse: bool,
        stop_after: Option<u64>,
        resume_after: Option<u64>,
        end_after: Option<i32>,
    },
}
enum Command {
    Region(
        crate::media_prepare::VideoCrop,
        mpsc::SyncSender<Result<()>>,
    ),
    Pause(mpsc::SyncSender<Result<()>>),
    Resume(mpsc::SyncSender<Result<()>>),
    Finish,
}
struct Progress {
    source_ended: AtomicBool,
    accepting: Mutex<bool>,
    started: AtomicBool,
    latest: Mutex<Option<gst::Sample>>,
    region: Mutex<Option<crate::media_prepare::VideoCrop>>,
    frames: AtomicU64,
    active_millis: AtomicU64,
    bytes: AtomicU64,
    oversized: AtomicBool,
    finishing: AtomicBool,
    cancel: AtomicBool,
    phase: Mutex<Phase>,
}
impl Progress {
    fn set_phase(&self, phase: Phase) {
        *self.phase.lock().unwrap() = phase;
    }
}

/// Drop cancels and joins the engine. Explicit finish returns an owned MP4.
/// Keep the portal Session alive until this handle has finished or been dropped.
pub struct Recorder {
    commands: mpsc::SyncSender<Command>,
    progress: Arc<Progress>,
    task: Option<JoinHandle<Result<Option<RecordingFile>>>>,
}
impl Recorder {
    pub fn start(session: &crate::screencast::Session) -> Result<Self> {
        Self::open(session, false)
    }
    /// Open the source for region preview without recording frames or time.
    pub fn prepare(session: &crate::screencast::Session) -> Result<Self> {
        Self::open(session, true)
    }
    fn open(session: &crate::screencast::Session, setup: bool) -> Result<Self> {
        ensure!(!*session.closed().borrow(), "ScreenCast session is closed");
        ensure!(session.node_id != 0, "Missing ScreenCast node");
        Self::spawn_mode(
            Source::Portal {
                fd: session
                    .remote_fd()
                    .context("Missing PipeWire descriptor")?
                    .try_clone_to_owned()?,
                node: session.node_id,
                serial: session.serial,
                closed: session.closed(),
            },
            MAX_FRAMES,
            MAX_BYTES,
            setup,
        )
    }
    #[cfg(test)]
    pub(crate) fn synthetic() -> Result<Self> {
        Self::spawn(
            Source::Synthetic {
                closed: None,
                sparse: false,
                stop_after: None,
                resume_after: None,
                end_after: None,
            },
            MAX_FRAMES,
            MAX_BYTES,
        )
    }
    #[cfg(test)]
    fn spawn(source: Source, max_frames: u64, max_bytes: u64) -> Result<Self> {
        Self::spawn_mode(source, max_frames, max_bytes, false)
    }
    /// Called only by the isolated worker after inheriting the selected portal FD.
    pub(crate) fn prepare_worker(
        fd: Option<OwnedFd>,
        node: u32,
        serial: Option<u64>,
        directory: std::path::PathBuf,
        synthetic: bool,
        setup: bool,
    ) -> Result<Self> {
        let (_keep_open, closed) = tokio::sync::watch::channel(false);
        let source = if synthetic {
            #[cfg(any(test, feature = "native-smoke"))]
            {
                Source::Synthetic {
                    closed: None,
                    sparse: false,
                    stop_after: None,
                    resume_after: None,
                    end_after: None,
                }
            }
            #[cfg(not(any(test, feature = "native-smoke")))]
            {
                bail!("Synthetic recording is not available");
            }
        } else {
            ensure!(node != 0, "Missing ScreenCast node");
            Source::Portal {
                fd: fd.context("Missing inherited PipeWire descriptor")?,
                node,
                serial,
                closed,
            }
        };
        Self::spawn_mode_in(source, MAX_FRAMES, MAX_BYTES, setup, Some(directory))
    }
    fn spawn_mode(source: Source, max_frames: u64, max_bytes: u64, setup: bool) -> Result<Self> {
        Self::spawn_mode_in(source, max_frames, max_bytes, setup, None)
    }
    fn spawn_mode_in(
        source: Source,
        max_frames: u64,
        max_bytes: u64,
        setup: bool,
        directory: Option<std::path::PathBuf>,
    ) -> Result<Self> {
        let progress = Arc::new(Progress {
            source_ended: AtomicBool::new(false),
            accepting: Mutex::new(!setup),
            started: AtomicBool::new(!setup),
            latest: Mutex::new(None),
            region: Mutex::new(None),
            frames: AtomicU64::new(0),
            active_millis: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            oversized: AtomicBool::new(false),
            finishing: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            phase: Mutex::new(if setup {
                Phase::Setup
            } else {
                Phase::Recording
            }),
        });
        let (commands, receiver) = mpsc::sync_channel(4);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let observed = progress.clone();
        let task = thread::Builder::new()
            .name("screen-record".into())
            .spawn(move || {
                let result = record(
                    source, &observed, receiver, ready_tx, max_frames, max_bytes, directory,
                );
                if let Err(error) = &result {
                    observed.set_phase(Phase::Failed(format!("{error:#}")));
                }
                result
            })?;
        let mut recorder = Self {
            commands,
            progress,
            task: Some(task),
        };
        if ready_rx.recv().unwrap_or(false) {
            return Ok(recorder);
        }
        let result = recorder.join()?;
        drop(result);
        bail!("Recording did not start")
    }
    pub fn status(&self) -> Status {
        Status {
            region: *self.progress.region.lock().unwrap(),
            phase: self.progress.phase.lock().unwrap().clone(),
            frames: self.progress.frames.load(Ordering::Acquire),
            active_millis: self.progress.active_millis.load(Ordering::Acquire),
        }
    }
    pub fn pause(&self) -> Result<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Pause(tx))
            .context("Recorder stopped")?;
        rx.recv()
            .context("Recorder stopped before pause acknowledgement")?
    }
    pub fn resume(&self) -> Result<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Resume(tx))
            .context("Recorder stopped")?;
        rx.recv()
            .context("Recorder stopped before resume acknowledgement")?
    }
    /// Encode the last full-source frame, without resuming capture or using crop.
    pub fn snapshot(&self) -> Result<Vec<u8>> {
        ensure!(
            matches!(self.status().phase, Phase::Setup | Phase::Paused),
            "Pause recording before previewing source"
        );
        let sample = self
            .progress
            .latest
            .lock()
            .unwrap()
            .clone()
            .context("No source frame yet")?;
        encode_snapshot(&sample)
    }
    /// Change the next interval's source rectangle while paused.
    pub fn set_region(&self, region: crate::media_prepare::VideoCrop) -> Result<()> {
        region.validate()?;
        let (tx, rx) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Region(region, tx))
            .context("Recorder stopped")?;
        rx.recv()
            .context("Recorder stopped before region acknowledgement")?
    }
    pub fn finish(mut self) -> Result<RecordingFile> {
        // Even if the worker already failed, join to return the actual error.
        let _ = self.commands.send(Command::Finish);
        self.join()?.context("Recording was cancelled")
    }
    fn join(&mut self) -> Result<Option<RecordingFile>> {
        self.task
            .take()
            .context("Recorder already joined")?
            .join()
            .map_err(|_| anyhow::anyhow!("Recorder worker panicked"))?
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        self.progress.cancel.store(true, Ordering::Release);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

struct Pipeline(gst::Pipeline);
impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}
fn element(name: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(name)
        .build()
        .with_context(|| format!("Missing GStreamer element: {name}"))
}
fn transition(pipeline: &gst::Pipeline, state: gst::State) -> Result<()> {
    pipeline.set_state(state)?;
    let (result, current, _) = pipeline.state(gst::ClockTime::from_seconds(5));
    result?;
    ensure!(current == state, "Recording state transition timed out");
    Ok(())
}
fn build(
    source: &Source,
    path: &Path,
    progress: &Arc<Progress>,
    max_frames: u64,
    max_bytes: u64,
) -> Result<Pipeline> {
    gst::init()?;
    let source = match source {
        Source::Portal {
            fd, node, serial, ..
        } => {
            let source = element("pipewiresrc")?;
            source.set_property("fd", fd.as_raw_fd());
            ensure!(
                source.find_property("keepalive-time").is_some(),
                "PipeWire plugin lacks unchanged-frame keepalive"
            );
            source.set_property("keepalive-time", 40i32);
            source.set_property("do-timestamp", true);
            source.set_property("resend-last", true);
            if let Some(serial) = serial {
                ensure!(
                    source.find_property("target-object").is_some(),
                    "PipeWire plugin lacks stable stream targeting"
                );
                source.set_property("target-object", serial.to_string());
            } else {
                source.set_property("path", node.to_string());
            }
            source
        }
        #[cfg(any(test, feature = "native-smoke"))]
        Source::Synthetic {
            sparse,
            stop_after,
            resume_after,
            end_after,
            ..
        } => {
            let source = element("videotestsrc")?;
            source.set_property("is-live", true);
            if let Some(count) = end_after {
                source.set_property("num-buffers", *count);
            }
            if *sparse || stop_after.is_some() {
                // Model sparse damage updates or stopped buffer delivery while
                // keeping state changes and EOS responsive. This is not a
                // simulation of a deadlocked source/plugin.
                let count = AtomicU64::new(0);
                let sparse = *sparse;
                let stop_after = *stop_after;
                let resume_after = *resume_after;
                source
                    .static_pad("src")
                    .context("Missing synthetic pad")?
                    .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                        let index = count.fetch_add(1, Ordering::Relaxed);
                        if stop_after.is_some_and(|limit| {
                            index >= limit && resume_after.is_none_or(|resume| index < resume)
                        }) || (sparse && index.checked_rem(6) != Some(0))
                        {
                            gst::PadProbeReturn::Drop
                        } else {
                            gst::PadProbeReturn::Ok
                        }
                    });
            }
            source
        }
    };
    let input = element("capsfilter")?;
    input.set_property(
        "caps",
        gst::Caps::builder("video/x-raw")
            .field("width", gst::IntRange::<i32>::new(2, 4096))
            .field("height", gst::IntRange::<i32>::new(2, 4096))
            .build(),
    );
    let queue = element("queue")?;
    queue.set_property("max-size-buffers", 2u32);
    queue.set_property("max-size-bytes", 0u32);
    queue.set_property("max-size-time", 0u64);
    let convert = element("videoconvert")?;
    let rgb = element("capsfilter")?;
    rgb.set_property(
        "caps",
        gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .build(),
    );
    let source_progress = progress.clone();
    rgb.static_pad("src")
        .context("Missing source events pad")?
        .add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
            if info
                .event()
                .is_some_and(|event| matches!(event.view(), gst::EventView::Eos(_)))
            {
                source_progress.source_ended.store(true, Ordering::Release);
            }
            gst::PadProbeReturn::Ok
        });
    // Clock the latest source frame independently of damage updates. A
    // responsive source stall must preserve the final visible frame and time.
    let rate = element("imagefreeze")?;
    rate.set_property("is-live", true);
    rate.set_property("allow-replace", true);
    let observed = progress.clone();
    let setup = !progress.started.load(Ordering::Acquire);
    rate.static_pad("src")
        .context("Missing full-source pad")?
        .add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
            if let (Some(buffer), Some(caps)) = (info.buffer(), pad.current_caps()) {
                // Retain one bounded negotiated RGBA frame; never copy every frame.
                if buffer.size() <= 4096 * 4096 * 4 + 4096 * 64 {
                    *observed.latest.lock().unwrap() =
                        Some(gst::Sample::builder().buffer(buffer).caps(&caps).build());
                }
            }
            // Snapshot is upstream of this gate. Setup buffers never reach
            // the encoder; its output probe assigns the saved CFR timeline.
            if !observed.started.load(Ordering::Acquire) {
                return gst::PadProbeReturn::Drop;
            }
            gst::PadProbeReturn::Ok
        });
    let crop = element("videocrop")?;
    crop.set_property("name", "capture-region");
    let scale = element("videoscale")?;
    scale.set_property("add-borders", false);
    let encode_convert = element("videoconvert")?;
    let output = element("capsfilter")?;
    output.set_property("name", "capture-output");
    output.set_property(
        "caps",
        gst::Caps::builder("video/x-raw")
            .field("format", "I420")
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .field("framerate", gst::Fraction::new(FPS as i32, 1))
            .field("width", gst::IntRange::<i32>::with_step(2, 4096, 2))
            .field("height", gst::IntRange::<i32>::with_step(2, 4096, 2))
            .build(),
    );
    let encoder = element("x264enc")?;
    encoder.set_property_from_str("tune", "zerolatency");
    encoder.set_property_from_str("speed-preset", "ultrafast");
    encoder.set_property("threads", 2u32);
    encoder.set_property("bitrate", 8000u32);
    encoder.set_property("bframes", 0u32);
    encoder.set_property("key-int-max", FPS as u32);
    let parser = element("h264parse")?;
    let mux = element("mp4mux")?;
    let sink = element("filesink")?;
    sink.set_property("location", path.to_str().context("Invalid recording path")?);
    // Setup intentionally produces no encoder preroll until explicit Resume.
    if setup {
        sink.set_property("async", false);
    }
    let p = progress.clone();
    output
        .static_pad("src")
        .context("Missing raw output")?
        .add_probe(gst::PadProbeType::BUFFER, move |_, info| {
            let accepting = p.accepting.lock().unwrap();
            if !*accepting
                || p.finishing.load(Ordering::Acquire)
                || p.frames.load(Ordering::Acquire) >= max_frames
            {
                return gst::PadProbeReturn::Drop;
            }
            let Some(gst::PadProbeData::Buffer(buffer)) = info.data.as_mut() else {
                return gst::PadProbeReturn::Drop;
            };
            let index = p.frames.fetch_add(1, Ordering::AcqRel);
            // Saved time is the admitted CFR timeline, independent of source
            // startup and discarded buffers around pause/resume transitions.
            let pts = gst::ClockTime::from_nseconds(index * 1_000_000_000 / FPS);
            let next = gst::ClockTime::from_nseconds((index + 1) * 1_000_000_000 / FPS);
            let buffer = buffer.make_mut();
            buffer.set_pts(pts);
            buffer.set_dts(None);
            buffer.set_duration(next - pts);
            gst::PadProbeReturn::Ok
        })
        .context("Cannot observe recording frames")?;
    let p = progress.clone();
    mux.static_pad("src")
        .context("Missing mux output")?
        .add_probe(
            gst::PadProbeType::BUFFER | gst::PadProbeType::BUFFER_LIST,
            move |_, info| {
                let size = info.buffer().map_or_else(
                    || {
                        info.buffer_list()
                            .map_or(0, |list| list.iter().map(|b| b.size() as u64).sum())
                    },
                    |b| b.size() as u64,
                );
                if p.bytes
                    .fetch_add(size, Ordering::AcqRel)
                    .saturating_add(size)
                    > max_bytes
                {
                    p.oversized.store(true, Ordering::Release);
                    return gst::PadProbeReturn::Drop;
                }
                gst::PadProbeReturn::Ok
            },
        )
        .context("Cannot bound recording output")?;
    let pipeline = Pipeline(gst::Pipeline::new());
    let elements = [
        &source,
        &input,
        &queue,
        &convert,
        &rgb,
        &rate,
        &crop,
        &scale,
        &encode_convert,
        &output,
        &encoder,
        &parser,
        &mux,
        &sink,
    ];
    pipeline.0.add_many(elements)?;
    gst::Element::link_many(elements)?;
    Ok(pipeline)
}
fn encode_snapshot(sample: &gst::Sample) -> Result<Vec<u8>> {
    let caps = sample.caps().context("Missing source caps")?;
    let structure = caps.structure(0).context("Missing source structure")?;
    let width: i32 = structure.get("width")?;
    let height: i32 = structure.get("height")?;
    ensure!(
        (2..=4096).contains(&width) && (2..=4096).contains(&height),
        "Invalid preview dimensions"
    );
    let divisor = width.max(height).max(640);
    let input = element("appsrc")?;
    input.set_property("caps", caps.to_owned());
    input.set_property("format", gst::Format::Time);
    let scale = element("videoscale")?;
    let convert = element("videoconvert")?;
    let output = element("capsfilter")?;
    output.set_property(
        "caps",
        gst::Caps::builder("video/x-raw")
            .field("width", (width * 640 / divisor).max(1))
            .field("height", (height * 640 / divisor).max(1))
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .build(),
    );
    let encoder = element("jpegenc")?;
    encoder.set_property("quality", 85i32);
    let sink = element("fakesink")?;
    sink.set_property("sync", false);
    let pipeline = Pipeline(gst::Pipeline::new());
    let elements = [&input, &scale, &convert, &output, &encoder, &sink];
    pipeline.0.add_many(elements)?;
    gst::Element::link_many(elements)?;
    let bus = pipeline.0.bus().context("Missing preview bus")?;
    pipeline.0.set_state(gst::State::Playing)?;
    let buffer = sample.buffer().context("Missing source buffer")?.to_owned();
    ensure!(
        input.emit_by_name::<gst::FlowReturn>("push-buffer", &[&buffer]) == gst::FlowReturn::Ok,
        "Preview input rejected"
    );
    ensure!(
        input.emit_by_name::<gst::FlowReturn>("end-of-stream", &[]) == gst::FlowReturn::Ok,
        "Preview EOS rejected"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    while !check_bus(&bus)? {
        ensure!(Instant::now() < deadline, "Source preview timed out");
        thread::sleep(Duration::from_millis(10));
    }
    let result = sink
        .property::<Option<gst::Sample>>("last-sample")
        .context("No encoded source preview")?;
    let bytes = result
        .buffer()
        .context("Missing preview bytes")?
        .map_readable()?;
    ensure!(bytes.len() <= 2 * 1024 * 1024, "Source preview too large");
    Ok(bytes.as_slice().to_vec())
}
fn set_region(
    pipeline: &gst::Pipeline,
    region: crate::media_prepare::VideoCrop,
    initial: Option<(i32, i32)>,
) -> Result<()> {
    region.validate()?;
    let crop = pipeline
        .by_name("capture-region")
        .context("Missing capture crop")?;
    let output = pipeline
        .by_name("capture-output")
        .context("Missing capture output")?;
    let dimensions = |element: &gst::Element, pad: &str| -> Result<(i32, i32)> {
        let caps = element
            .static_pad(pad)
            .and_then(|p| p.current_caps())
            .context("Capture dimensions not negotiated yet")?;
        let structure = caps.structure(0).context("Missing capture caps")?;
        Ok((structure.get("width")?, structure.get("height")?))
    };
    let (width, height) = initial.map_or_else(|| dimensions(&crop, "sink"), Ok)?;
    let (out_width, out_height) = initial.map_or_else(
        || dimensions(&output, "src"),
        |(w, h)| Ok((w - w % 2, h - h % 2)),
    )?;
    let left = (region.x * f64::from(width)).floor() as i32;
    let top = (region.y * f64::from(height)).floor() as i32;
    let selected_width = (region.width * f64::from(width)).floor().max(1.0) as i32;
    let selected_height = (region.height * f64::from(height)).floor().max(1.0) as i32;
    ensure!(
        left + selected_width <= width && top + selected_height <= height,
        "Capture region exceeds source"
    );
    // Pin encoder geometry before changing crop; MP4 must not change dimensions.
    output.set_property(
        "caps",
        gst::Caps::builder("video/x-raw")
            .field("format", "I420")
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .field("framerate", gst::Fraction::new(FPS as i32, 1))
            .field("width", out_width)
            .field("height", out_height)
            .build(),
    );
    crop.set_property("left", left);
    crop.set_property("right", width - left - selected_width);
    crop.set_property("top", top);
    crop.set_property("bottom", height - top - selected_height);
    Ok(())
}
fn check_bus(bus: &gst::Bus) -> Result<bool> {
    while let Some(message) = bus.pop() {
        match message.view() {
            gst::MessageView::Error(error) => bail!(
                "Recording pipeline: {} ({:?})",
                error.error(),
                error.debug()
            ),
            gst::MessageView::Eos(..) => return Ok(true),
            _ => (),
        }
    }
    Ok(false)
}
fn record(
    source: Source,
    progress: &Arc<Progress>,
    commands: mpsc::Receiver<Command>,
    ready: mpsc::SyncSender<bool>,
    max_frames: u64,
    max_bytes: u64,
    directory: Option<std::path::PathBuf>,
) -> Result<Option<RecordingFile>> {
    let directory = match directory {
        Some(path) => tempfile::tempdir_in(path)?,
        None => tempfile::tempdir()?,
    };
    let path = directory.path().join("recording.mp4");
    let pipeline = build(&source, &path, progress, max_frames, max_bytes)?;
    let bus = pipeline.0.bus().context("Missing recording bus")?;
    transition(&pipeline.0, gst::State::Playing)?;
    let _ = ready.send(true);
    let mut phase = progress.phase.lock().unwrap().clone();
    let mut active = Duration::ZERO;
    let mut last_tick = Instant::now();
    let duration_limit = Duration::from_secs_f64(max_frames as f64 / FPS as f64);
    loop {
        let now = Instant::now();
        if phase == Phase::Recording {
            active += now.duration_since(last_tick);
        }
        last_tick = now;
        progress.active_millis.store(
            active.min(duration_limit).as_millis() as u64,
            Ordering::Release,
        );
        if progress.cancel.load(Ordering::Acquire) {
            progress.set_phase(Phase::Cancelled);
            return Ok(None);
        }
        let source_closed = match &source {
            Source::Portal { closed, .. } => *closed.borrow(),
            #[cfg(any(test, feature = "native-smoke"))]
            Source::Synthetic { closed, .. } => closed.as_ref().is_some_and(|s| *s.borrow()),
        };
        ensure!(!source_closed, "ScreenCast session closed while recording");
        ensure!(
            !progress.source_ended.load(Ordering::Acquire),
            "Capture source ended unexpectedly"
        );
        ensure!(
            !progress.oversized.load(Ordering::Acquire),
            "Recording exceeds output size limit"
        );
        ensure!(!check_bus(&bus)?, "Capture source ended unexpectedly");
        if phase == Phase::Recording
            && (progress.frames.load(Ordering::Acquire) >= max_frames || active >= duration_limit)
        {
            *progress.accepting.lock().unwrap() = false;
            transition(&pipeline.0, gst::State::Paused)?;
            active = duration_limit;
            progress
                .active_millis
                .store(duration_limit.as_millis() as u64, Ordering::Release);
            phase = Phase::LimitReached;
            progress.set_phase(phase.clone());
        }
        match commands.recv_timeout(Duration::from_millis(20)) {
            Ok(Command::Region(region, reply)) => {
                let result = if phase == Phase::Setup {
                    region.validate()
                } else if phase == Phase::Paused {
                    set_region(&pipeline.0, region, None)
                } else {
                    Err(anyhow::anyhow!(
                        "Pause recording before changing capture region"
                    ))
                };
                if result.is_ok() {
                    *progress.region.lock().unwrap() = Some(region);
                }
                let _ = reply.send(result);
            }
            Ok(Command::Pause(reply)) => {
                if phase == Phase::Setup {
                    let _ = reply.send(Err(anyhow::anyhow!("Recording has not started")));
                    continue;
                }
                *progress.accepting.lock().unwrap() = false;
                transition(&pipeline.0, gst::State::Paused)?;
                if phase == Phase::Recording {
                    active += last_tick.elapsed();
                    progress.active_millis.store(
                        active.min(duration_limit).as_millis() as u64,
                        Ordering::Release,
                    );
                }
                if phase != Phase::LimitReached {
                    phase = Phase::Paused;
                }
                progress.set_phase(phase.clone());
                let _ = reply.send(Ok(()));
            }
            Ok(Command::Resume(reply)) => {
                if progress.frames.load(Ordering::Acquire) >= max_frames || active >= duration_limit
                {
                    let _ = reply.send(Err(anyhow::anyhow!("Recording time limit reached")));
                } else {
                    if phase == Phase::Setup {
                        let configured = (|| -> Result<()> {
                            let sample = progress
                                .latest
                                .lock()
                                .unwrap()
                                .clone()
                                .context("No source frame yet")?;
                            let caps = sample.caps().context("Missing source caps")?;
                            let structure =
                                caps.structure(0).context("Missing source dimensions")?;
                            let region = progress.region.lock().unwrap().unwrap_or(
                                crate::media_prepare::VideoCrop {
                                    x: 0.0,
                                    y: 0.0,
                                    width: 1.0,
                                    height: 1.0,
                                },
                            );
                            set_region(
                                &pipeline.0,
                                region,
                                Some((structure.get("width")?, structure.get("height")?)),
                            )
                        })();
                        if let Err(error) = configured {
                            let _ = reply.send(Err(error));
                            continue;
                        }
                        progress.started.store(true, Ordering::Release);
                    }
                    if phase != Phase::Recording {
                        *progress.accepting.lock().unwrap() = true;
                        transition(&pipeline.0, gst::State::Playing)?;
                        last_tick = Instant::now();
                        phase = Phase::Recording;
                        progress.set_phase(phase.clone());
                    }
                    let _ = reply.send(Ok(()));
                }
            }
            Ok(Command::Finish) => {
                if phase == Phase::Recording {
                    active += last_tick.elapsed();
                    progress.active_millis.store(
                        active.min(duration_limit).as_millis() as u64,
                        Ordering::Release,
                    );
                }
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
        }
    }
    progress.finishing.store(true, Ordering::Release);
    // Resume a paused live pipeline to let EOS drain without admitting new frames.
    pipeline.0.set_state(gst::State::Playing)?;
    ensure!(
        pipeline
            .0
            .by_name("capture-region")
            .context("Missing capture crop")?
            .send_event(gst::event::Eos::new()),
        "Recorder rejected EOS"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if progress.cancel.load(Ordering::Acquire) {
            progress.set_phase(Phase::Cancelled);
            return Ok(None);
        }
        ensure!(
            !progress.oversized.load(Ordering::Acquire),
            "Recording exceeds output size limit"
        );
        if check_bus(&bus)? {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "Recording finalization timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
    transition(&pipeline.0, gst::State::Null)?;
    ensure!(
        !progress.oversized.load(Ordering::Acquire),
        "Recording exceeds output size limit"
    );
    let frames = progress.frames.load(Ordering::Acquire);
    ensure!(frames > 0, "Recording contains no frames");
    let length = std::fs::metadata(&path)?.len();
    ensure!(length <= max_bytes, "Recording exceeds file size limit");
    progress.set_phase(Phase::Finished);
    Ok(Some(RecordingFile {
        directory,
        length,
        frames,
    }))
}

#[cfg(test)]
mod tests;
