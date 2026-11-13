//! Bounded GIF/MP4 preparation. Run on a blocking worker, never the UI executor.
//! No daemon or device is touched; publication is a separate, explicit step.
use crate::video_adjustments::VideoAdjustments;
use anyhow::{bail, Context, Result};
use llcontrol_ipc::{
    validate_h264_access_unit, validate_h264_clip, H264AccessUnit, H264Clip,
    MAX_MEDIA_UPLOAD_BYTES, MAX_MEDIA_UPLOAD_FRAMES,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

pub const MAX_STAGED_SOURCE: u64 =
    MAX_MEDIA_UPLOAD_BYTES + MAX_MEDIA_UPLOAD_FRAMES as u64 * 64 + 1024 * 1024;
const MAX_UNIT: u64 = 128 * 1024;
const MAX_REPORT: u64 = 128 * 1024;

/// How the source fits the square LCD.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// Preserve the whole image, with black bars as necessary.
    Contain,
    /// Fill the LCD by cropping the excess at the centre.
    Cover,
    /// GA2LCD AutoFill: resize the full source to the display, without aspect preservation.
    Stretch,
}

#[derive(Clone, Copy)]
struct PictureOptions {
    fit: Fit,
    adjustments: VideoAdjustments,
}
impl From<Fit> for PictureOptions {
    fn from(fit: Fit) -> Self {
        Self {
            fit,
            adjustments: VideoAdjustments::default(),
        }
    }
}

/// Rectangle in the displayed (autorotated) source, normalized to 0..1.
/// Kept per interval because VideoCut records the capture rectangle each time.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoCrop {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl VideoCrop {
    pub fn validate(self) -> Result<()> {
        if [self.x, self.y, self.width, self.height]
            .iter()
            .any(|v| !v.is_finite())
            || self.x < 0.0
            || self.y < 0.0
            || self.width <= 0.0
            || self.height <= 0.0
            || self.x + self.width > 1.0
            || self.y + self.height > 1.0
        {
            bail!("Crop must be a nonempty rectangle within the source");
        }
        Ok(())
    }

    fn filter(self) -> String {
        // RGB avoids chroma-grid rounding of odd crop coordinates. FFmpeg's
        // autorotation runs before this filter, matching displayed coordinates.
        format!("format=rgb24,crop=w='max(1,floor(iw*{}))':h='max(1,floor(ih*{}))':x='floor(iw*{})':y='floor(ih*{})':exact=1,",
            self.width, self.height, self.x, self.y)
    }
}

/// One source interval, in milliseconds on the first video stream timeline.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub start_ms: u64,
    pub end_ms: u64,
    #[serde(default)]
    pub crop: Option<VideoCrop>,
}
/// Ordered recording intervals, matching the VideoCut record/pause workflow.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoCut {
    pub segments: Vec<Segment>,
}
impl VideoCut {
    pub fn validate(&self) -> Result<()> {
        if self.segments.is_empty() || self.segments.len() > 64 {
            bail!("Select 1..64 video segments");
        }
        let mut duration = 0;
        for s in &self.segments {
            if let Some(crop) = s.crop {
                crop.validate()?;
            }
            if s.start_ms >= s.end_ms || s.end_ms > 3_600_000 {
                bail!("Segment must have start < end within one hour");
            }
            duration += s.end_ms - s.start_ms;
        }
        if duration > 180_000 {
            bail!("Selected segments exceed three minutes");
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct TimedSegment {
    start_seconds: f64,
    end_seconds: f64,
    crop: Option<VideoCrop>,
}
impl From<Segment> for TimedSegment {
    fn from(segment: Segment) -> Self {
        Self {
            start_seconds: segment.start_ms as f64 / 1000.0,
            end_seconds: segment.end_ms as f64 / 1000.0,
            crop: segment.crop,
        }
    }
}

/// Honest phase progress; these stages are not an estimated percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Inspecting,
    Encoding,
    Validating,
}

fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        bail!("Media preparation cancelled");
    }
    Ok(())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        bail!("Expected a regular file");
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("Media preparation output exceeds its size limit");
    }
    Ok(bytes)
}

// Keep the child owned until it is reaped, including on cancellation/timeout.
// A worker stuck in kernel I/O must retain its caller's admission permit; callers
// must not release that permit merely because the UI has requested cancellation.
fn run(command: &mut Command, directory: &Path, cancel: &AtomicBool) -> Result<Vec<u8>> {
    run_bounded(
        command,
        directory,
        cancel,
        MAX_REPORT,
        Duration::from_secs(60),
    )
}

fn run_bounded(
    command: &mut Command,
    directory: &Path,
    cancel: &AtomicBool,
    report_limit: u64,
    timeout: Duration,
) -> Result<Vec<u8>> {
    check_cancel(cancel)?;
    let stdout = directory.join("stdout");
    let stderr = directory.join("stderr");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(File::create(&stdout)?)
        .stderr(File::create(&stderr)?)
        .spawn()
        .context("Cannot start media tool; install FFmpeg with ffprobe and libx264")?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
        }
        let cancelled = cancel.load(Ordering::Acquire);
        let timed_out = started.elapsed() >= timeout;
        let oversized = [&stdout, &stderr]
            .iter()
            .any(|p| fs::metadata(p).is_ok_and(|m| m.len() > report_limit));
        if cancelled || timed_out || oversized {
            let _ = child.kill();
            let _ = child.wait();
            if cancelled {
                bail!("Media preparation cancelled");
            }
            if oversized {
                bail!("Media tool output exceeds its size limit");
            }
            bail!("Media tool timed out after {} seconds", timeout.as_secs());
        }
        thread::sleep(Duration::from_millis(20));
    };
    check_cancel(cancel)?;
    let detail = read_bounded(&stderr, MAX_REPORT)?;
    if !status.success() {
        bail!("Media tool failed: {}", String::from_utf8_lossy(&detail));
    }
    read_bounded(&stdout, report_limit)
}

/// Decode the first saved frame into a bounded 144px PNG without device access.
/// Run on a blocking worker and retain its admission until child reap.
pub fn thumbnail(clip: &H264Clip, cancel: &AtomicBool) -> Result<Vec<u8>> {
    check_cancel(cancel)?;
    validate_h264_clip(clip, 1)?;
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("clip.h264");
    let output = directory.path().join("thumbnail.png");
    {
        use std::io::Write;
        let mut file = File::create(&source)?;
        for unit in &clip.units {
            file.write_all(&unit.bytes)?;
        }
    }
    run(
        Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-threads",
                "1",
                "-filter_threads",
                "1",
                "-protocol_whitelist",
                "file",
                "-f",
                "h264",
                "-max_pixels",
                "262144",
                "-i",
            ])
            .arg(&source)
            .args([
                "-frames:v",
                "1",
                "-vf",
                "scale=144:144",
                "-threads",
                "1",
                "-c:v",
                "png",
                "-f",
                "image2",
                "-update",
                "1",
            ])
            .arg(&output),
        directory.path(),
        cancel,
    )?;
    let bytes = read_bounded(&output, MAX_REPORT)?;
    let image = llcontrol_image::assets::decode_asset(&bytes)?;
    if image.width() != 144 || image.height() != 144 {
        bail!("Unexpected thumbnail dimensions");
    }
    check_cancel(cancel)?;
    Ok(bytes)
}

/// Prepared H.264 on disk. Read units on a blocking worker; keep this owner until
/// publication completes or is cancelled. Dropping it removes its staging directory.
pub struct PreparedFile {
    file: File,
    path: PathBuf,
    packets: Vec<UnitRange>,
    encoded_bytes: u64,
    // Declared last so the file is closed before its directory is removed.
    _directory: tempfile::TempDir,
}
/// Disk staging for a finite daemon export. Never yields a PreparedFile before
/// the consumer has received Complete and validated every unit and total.
pub struct ExportWriter {
    prepared: PreparedFile,
    info: llcontrol_ipc::MediaExportInfo,
}
impl ExportWriter {
    pub fn new(info: llcontrol_ipc::MediaExportInfo) -> Result<Self> {
        if info.fps != 24
            || !(1..=MAX_MEDIA_UPLOAD_FRAMES).contains(&info.frames)
            || !(1..=MAX_MEDIA_UPLOAD_BYTES).contains(&info.encoded_bytes)
        {
            bail!("Invalid media export metadata");
        }
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("export.h264");
        let file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self {
            prepared: PreparedFile {
                file,
                path,
                packets: Vec::new(),
                encoded_bytes: 0,
                _directory: directory,
            },
            info,
        })
    }
    pub fn push(mut self, index: usize, unit: H264AccessUnit) -> Result<Self> {
        if index != self.prepared.frames() || index >= self.info.frames {
            bail!("Out-of-order media export frame");
        }
        validate_h264_access_unit(&unit, index == 0)?;
        let total = self.prepared.encoded_bytes + unit.bytes.len() as u64;
        if total > self.info.encoded_bytes {
            bail!("Media export exceeds byte count");
        }
        self.prepared.file.write_all(&unit.bytes)?;
        self.prepared.packets.push(UnitRange {
            position: self.prepared.encoded_bytes,
            size: unit.bytes.len(),
            keyframe: unit.keyframe,
        });
        self.prepared.encoded_bytes = total;
        Ok(self)
    }
    pub fn finish(self) -> Result<PreparedFile> {
        if self.prepared.frames() != self.info.frames
            || self.prepared.encoded_bytes != self.info.encoded_bytes
        {
            bail!("Incomplete media export");
        }
        Ok(self.prepared)
    }
}

/// Seekable preview owner; dropping it removes only the MP4, not prepared media.
pub struct PreviewFile {
    file: File,
    length: u64,
    _directory: Arc<tempfile::TempDir>,
}
/// Keeps an immutable remuxed source alive independently of its preview URL.
pub struct PreviewLease {
    directory: Arc<tempfile::TempDir>,
    pub length: u64,
}
impl PreviewLease {
    pub fn path(&self) -> PathBuf {
        self.directory.path().join("preview.mp4")
    }
}
impl PreviewFile {
    pub fn lease(&self) -> PreviewLease {
        PreviewLease {
            directory: self._directory.clone(),
            length: self.length,
        }
    }

    /// Total bytes for HTTP range responses or bounded IPC readers.
    pub fn length(&self) -> u64 {
        self.length
    }
    /// Read at most 256 KiB; callers never need to materialize the entire movie.
    pub fn read_range(&mut self, offset: u64, length: usize) -> Result<Vec<u8>> {
        if length == 0 || length > 256 * 1024 || offset >= self.length {
            bail!("Invalid preview range");
        }
        let size = length.min(usize::try_from(self.length - offset)?);
        let mut bytes = vec![0; size];
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

struct UnitRange {
    position: u64,
    size: usize,
    keyframe: bool,
}
impl PreparedFile {
    #[cfg(any(test, feature = "test-support"))]
    pub fn staging_path(&self) -> &Path {
        self._directory.path()
    }
    /// Remux these exact encoded frames into a seekable MP4, without re-encoding.
    /// The independent owner keeps preview bytes on disk for bounded range reads.
    pub fn preview(&self, cancel: &AtomicBool) -> Result<PreviewFile> {
        check_cancel(cancel)?;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("preview.mp4");
        // MP4 sample tables add bounded per-frame overhead to the encoded payload.
        let max_bytes = self.encoded_bytes + self.frames() as u64 * 64 + 1024 * 1024;
        run_bounded(
            Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-threads",
                    "1",
                    "-protocol_whitelist",
                    "file",
                    "-fflags",
                    "+genpts",
                    "-r",
                    "24",
                    "-f",
                    "h264",
                    "-i",
                ])
                .arg(&self.path)
                .args([
                    "-map",
                    "0:v:0",
                    "-c:v",
                    "copy",
                    "-an",
                    "-sn",
                    "-dn",
                    "-movflags",
                    "+faststart",
                    "-fs",
                    &(max_bytes + MAX_UNIT + 1).to_string(),
                    "-f",
                    "mp4",
                ])
                .arg(&path),
            directory.path(),
            cancel,
            MAX_REPORT,
            Duration::from_secs(600),
        )?;
        let file = File::open(&path)?;
        let length = file.metadata()?.len();
        if length == 0 || length > max_bytes {
            bail!("Preview exceeds its byte budget");
        }
        // A short successful remux must never be presented as the complete result.
        let report = run(
            Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-protocol_whitelist",
                    "file",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=nb_frames",
                    "-of",
                    "csv=p=0",
                ])
                .arg(&path),
            directory.path(),
            cancel,
        )?;
        let count: usize = std::str::from_utf8(&report)?.trim().parse()?;
        if count != self.frames() {
            bail!("Preview frame count differs from prepared media");
        }
        check_cancel(cancel)?;
        Ok(PreviewFile {
            file,
            length,
            _directory: Arc::new(directory),
        })
    }

    /// Number of 24fps access units.
    pub fn frames(&self) -> usize {
        self.packets.len()
    }
    /// Encoded payload size, excluding source and packet metadata.
    pub fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }
    /// Read and validate one bounded access unit without materializing the clip.
    pub fn read_unit(&mut self, index: usize) -> Result<H264AccessUnit> {
        let packet = self
            .packets
            .get(index)
            .context("Frame index out of range")?;
        self.file.seek(SeekFrom::Start(packet.position))?;
        let mut bytes = vec![0; packet.size];
        self.file.read_exact(&mut bytes)?;
        let unit = H264AccessUnit {
            bytes,
            keyframe: packet.keyframe,
        };
        validate_h264_access_unit(&unit, index == 0)?;
        Ok(unit)
    }
}

/// Legacy diagnostic preparation, retaining its original 10s/240AU/1MiB limits.
pub fn prepare(
    source: &Path,
    fit: Fit,
    cancel: &AtomicBool,
    progress: impl FnMut(Stage),
) -> Result<H264Clip> {
    let mut prepared =
        prepare_bounded(source, fit.into(), cancel, progress, 240, 1024 * 1024, None)?;
    let mut clip = H264Clip {
        fps: 24,
        units: Vec::with_capacity(prepared.frames()),
    };
    for index in 0..prepared.frames() {
        check_cancel(cancel)?;
        clip.units.push(prepared.read_unit(index)?);
    }
    validate_h264_clip(&clip, 1)?;
    Ok(clip)
}

/// Prepare persistent media on disk for bounded chunked publication, up to one
/// hour / 256MiB encoded. Run on a blocking worker retaining admission until reap.
/// These are Linux resource budgets, not inferred Windows duration limits.
pub fn prepare_file(
    source: &Path,
    fit: Fit,
    cancel: &AtomicBool,
    progress: impl FnMut(Stage),
) -> Result<PreparedFile> {
    prepare_file_budgeted(
        source,
        fit,
        MAX_MEDIA_UPLOAD_FRAMES,
        MAX_MEDIA_UPLOAD_BYTES,
        cancel,
        progress,
    )
}

/// Use the same validated pipeline with a remaining batch frame/byte budget.
/// Defaults remain identical for desktop callers of prepare_file.
pub fn prepare_file_budgeted(
    source: &Path,
    fit: Fit,
    max_frames: usize,
    max_bytes: u64,
    cancel: &AtomicBool,
    progress: impl FnMut(Stage),
) -> Result<PreparedFile> {
    if !(1..=MAX_MEDIA_UPLOAD_FRAMES).contains(&max_frames)
        || !(1..=MAX_MEDIA_UPLOAD_BYTES).contains(&max_bytes)
    {
        bail!("Invalid remaining media preparation budget");
    }
    prepare_bounded(
        source,
        fit.into(),
        cancel,
        progress,
        max_frames,
        max_bytes,
        None,
    )
}

/// Prepare ordered segments sequentially, keeping only one decoder admitted.
/// Each segment starts with an IDR; raw H.264 carries no audio/timestamp gaps.
pub fn prepare_file_cut(
    source: &Path,
    fit: Fit,
    cut: Option<VideoCut>,
    cancel: &AtomicBool,
    progress: impl FnMut(Stage),
) -> Result<PreparedFile> {
    prepare_file_edited(
        source,
        fit,
        cut,
        VideoAdjustments::default(),
        cancel,
        progress,
    )
}

/// Apply GA2LCD channel effects before spatial crop/resize, for the whole source
/// or every selected interval. Zero effects preserve the previous codec path.
pub fn prepare_file_edited(
    source: &Path,
    fit: Fit,
    cut: Option<VideoCut>,
    adjustments: VideoAdjustments,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Stage),
) -> Result<PreparedFile> {
    let picture = PictureOptions { fit, adjustments };
    let Some(cut) = cut else {
        return prepare_bounded(
            source,
            picture,
            cancel,
            progress,
            MAX_MEDIA_UPLOAD_FRAMES,
            MAX_MEDIA_UPLOAD_BYTES,
            None,
        );
    };
    cut.validate()?;
    check_cancel(cancel)?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("joined.h264");
    let mut output = File::create(&path)?;
    let mut packets = Vec::new();
    let mut encoded_bytes = 0;
    for segment in cut.segments {
        check_cancel(cancel)?;
        let mut part = prepare_bounded(
            source,
            picture,
            cancel,
            &mut progress,
            MAX_MEDIA_UPLOAD_FRAMES,
            MAX_MEDIA_UPLOAD_BYTES - encoded_bytes,
            Some(segment.into()),
        )?;
        for index in 0..part.frames() {
            check_cancel(cancel)?;
            let unit = part.read_unit(index)?;
            let size = unit.bytes.len();
            if packets.len() >= 24 * 180 + 64
                || encoded_bytes + size as u64 > MAX_MEDIA_UPLOAD_BYTES
            {
                bail!("Selected media exceeds its frame/byte budget");
            }
            use std::io::Write;
            output.write_all(&unit.bytes)?;
            packets.push(UnitRange {
                position: encoded_bytes,
                size,
                keyframe: unit.keyframe,
            });
            encoded_bytes += size as u64;
        }
    }
    drop(output);
    check_cancel(cancel)?;
    Ok(PreparedFile {
        file: File::open(&path)?,
        path,
        packets,
        encoded_bytes,
        _directory: directory,
    })
}

/// Prepare the separate VideoEdit single-range workflow, without VideoCut's
/// recording cap. Source timing is probed here, never trusted from a UI payload.
pub fn prepare_video_edit(
    source: &Path,
    fit: Fit,
    range: crate::video_range::VideoEditRange,
    adjustments: VideoAdjustments,
    cancel: &AtomicBool,
    progress: impl FnMut(Stage),
) -> Result<PreparedFile> {
    range.validate()?;
    let metadata = inspect_file(source, cancel)?;
    range.validate_source(&metadata)?;
    prepare_bounded(
        source,
        PictureOptions { fit, adjustments },
        cancel,
        progress,
        MAX_MEDIA_UPLOAD_FRAMES,
        MAX_MEDIA_UPLOAD_BYTES,
        Some(TimedSegment {
            start_seconds: range.start_seconds,
            end_seconds: range.end_seconds,
            crop: None,
        }),
    )
}

/// Exact rational rate reported by the source demuxer, never the output24fps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFrameRate {
    pub numerator: u32,
    pub denominator: u32,
}
impl SourceFrameRate {
    fn parse(value: Option<&str>) -> Option<Self> {
        let (numerator, denominator) = value?.split_once('/')?;
        let numerator = numerator.parse().ok()?;
        let denominator = denominator.parse().ok()?;
        (numerator > 0 && denominator > 0).then_some(Self {
            numerator,
            denominator,
        })
    }
    /// Duration of one nominal/average source frame, not a VFR frame index.
    pub fn frame_seconds(self) -> f64 {
        f64::from(self.denominator) / f64::from(self.numerator)
    }
}

/// Native source information for editor limits. Rates remain separate: neither
/// alone proves constant cadence or equivalence to the Windows player SDK.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMetadata {
    pub width: u32,
    pub height: u32,
    pub duration_seconds: Option<f64>,
    pub average_frame_rate: Option<SourceFrameRate>,
    pub nominal_frame_rate: Option<SourceFrameRate>,
}

/// Inspect an owned staged GIF/MP4 on a blocking worker. Caller must retain the
/// input lease and admission until this cancellable child has been reaped.
pub fn inspect_file(source: &Path, cancel: &AtomicBool) -> Result<SourceMetadata> {
    check_cancel(cancel)?;
    let source = source.canonicalize().context("Cannot open media file")?;
    let metadata = source.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_STAGED_SOURCE {
        bail!("Input must be a nonempty regular file within the staged-source byte limit");
    }
    let demuxer = match source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "gif" => "gif",
        "mp4" => "mov",
        _ => bail!("Input must be GIF or MP4"),
    };
    let directory = tempfile::tempdir()?;
    probe_source(&source, demuxer, directory.path(), cancel)
}

fn probe_source(
    source: &Path,
    demuxer: &str,
    directory: &Path,
    cancel: &AtomicBool,
) -> Result<SourceMetadata> {
    let probe = run(
        Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-protocol_whitelist",
                "file",
                "-f",
                demuxer,
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,duration,avg_frame_rate,r_frame_rate:format=duration",
                "-of",
                "json",
            ])
            .arg(source),
        directory,
        cancel,
    )?;
    let probe: InputProbe = serde_json::from_slice(&probe)?;
    if probe.streams.len() != 1
        || probe
            .streams
            .iter()
            .any(|s| !(1..=4096).contains(&s.width) || !(1..=4096).contains(&s.height))
    {
        bail!("Input must have one readable video stream within 1..4096 pixels");
    }
    let stream = &probe.streams[0];
    let duration = stream
        .duration
        .as_deref()
        .filter(|d| *d != "N/A")
        .or_else(|| {
            probe
                .format
                .as_ref()
                .and_then(|f| f.duration.as_deref())
                .filter(|d| *d != "N/A")
        });
    let duration_seconds = duration
        .map(|value| -> Result<f64> {
            let value: f64 = value.parse().context("Invalid media duration")?;
            if !value.is_finite() || value <= 0.0 {
                bail!("Invalid media duration");
            }
            Ok(value)
        })
        .transpose()?;
    Ok(SourceMetadata {
        width: stream.width,
        height: stream.height,
        duration_seconds,
        average_frame_rate: SourceFrameRate::parse(stream.avg_frame_rate.as_deref()),
        nominal_frame_rate: SourceFrameRate::parse(stream.r_frame_rate.as_deref()),
    })
}

fn prepare_bounded(
    source: &Path,
    picture: PictureOptions,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Stage),
    max_frames: usize,
    max_bytes: u64,
    segment: Option<TimedSegment>,
) -> Result<PreparedFile> {
    check_cancel(cancel)?;
    let source = source.canonicalize().context("Cannot open media file")?;
    let metadata = source.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_STAGED_SOURCE {
        bail!("Input must be a nonempty regular file within the staged-source byte limit");
    }
    let extension = source.extension().and_then(|s| s.to_str()).unwrap_or("");
    let demuxer = match extension.to_ascii_lowercase().as_str() {
        "gif" => "gif",
        "mp4" => "mov",
        _ => bail!("Input must be GIF or MP4"),
    };
    let directory = tempfile::tempdir()?;
    progress(Stage::Inspecting);
    let metadata = probe_source(&source, demuxer, directory.path(), cancel)?;
    let duration = metadata.duration_seconds;
    if segment.is_some() && duration.is_none() {
        bail!("Cannot cut video without a known duration");
    }
    if let Some(duration) = duration {
        if let Some(segment) = segment {
            // MP4 movie/browser duration can round the video stream duration up
            // to the next millisecond. Permit only this DTO precision boundary;
            // FFmpeg still stops at actual stream EOF.
            if segment.end_seconds * 1000.0 > duration * 1000.0 + 1.0 {
                bail!("Segment ends beyond the source duration");
            }
        }
        if !duration.is_finite() || duration <= 0.0 || duration > max_frames as f64 / 24.0 {
            bail!(
                "Media exceeds the {} second preparation budget",
                max_frames / 24
            );
        }
    }
    check_cancel(cancel)?;
    progress(Stage::Encoding);
    let encoded_path = directory.path().join("clip.h264");
    let mut command = Command::new("ffmpeg");
    command.args([
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-threads",
        "1",
        "-filter_threads",
        "1",
        "-protocol_whitelist",
        "file",
        "-f",
        demuxer,
    ]);
    if demuxer == "gif" {
        command.args(["-ignore_loop", "1"]);
    }
    let transform = match picture.fit {
        Fit::Contain => "scale=480:480:force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1,pad=480:480:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,fps=24",
        Fit::Cover => "scale=480:480:force_original_aspect_ratio=increase:force_divisible_by=2:reset_sar=1,crop=480:480,setsar=1,fps=24",
        Fit::Stretch => "scale=480:480,setsar=1,fps=24",
    };
    let effects = if picture.adjustments.contrast == 0 && picture.adjustments.brightness == 0 {
        String::new()
    } else {
        format!("{},", picture.adjustments.filter())
    };
    let transform = format!(
        "{effects}{}{transform}",
        segment
            .and_then(|s| s.crop)
            .map(VideoCrop::filter)
            .unwrap_or_default()
    );
    command.arg("-i").arg(&source);
    if let Some(segment) = segment {
        command.args([
            "-ss",
            &format!("{:.6}", segment.start_seconds),
            "-t",
            &format!("{:.6}", segment.end_seconds - segment.start_seconds),
        ]);
    }
    command
        .args([
            "-map",
            "0:v:0",
            "-vf",
            &transform,
            "-frames:v",
            &(max_frames + 1).to_string(),
            "-an",
            "-sn",
            "-dn",
            "-c:v",
            "libx264",
            "-profile:v",
            "baseline",
            "-level:v",
            "3.0",
            "-preset",
            "ultrafast",
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
            "-fs",
            &(max_bytes + MAX_UNIT + 1).to_string(),
            "-f",
            "h264",
        ])
        .arg(&encoded_path);
    run_bounded(
        &mut command,
        directory.path(),
        cancel,
        MAX_REPORT,
        Duration::from_secs(3600),
    )?;
    let file = File::open(&encoded_path)?;
    let encoded_bytes = file.metadata()?.len();
    if encoded_bytes == 0 || encoded_bytes > max_bytes {
        bail!("Encoded media exceeds its byte budget");
    }
    progress(Stage::Validating);
    let packets = run_bounded(
        Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-protocol_whitelist",
                "file",
                "-f",
                "h264",
                "-select_streams",
                "v:0",
                "-show_entries",
                "packet=pos,size,flags",
                "-of",
                "json",
            ])
            .arg(&encoded_path),
        directory.path(),
        cancel,
        16 * 1024 * 1024,
        Duration::from_secs(3600),
    )?;
    let packets: PacketProbe = serde_json::from_slice(&packets)?;
    if packets.packets.is_empty() || packets.packets.len() > max_frames {
        bail!("Encoded media exceeds its frame budget");
    }
    let mut ranges = Vec::with_capacity(packets.packets.len());
    let mut expected = 0;
    for packet in packets.packets {
        check_cancel(cancel)?;
        let position: u64 = packet.pos.parse()?;
        let size: u64 = packet.size.parse()?;
        let end = position
            .checked_add(size)
            .context("Packet range overflow")?;
        if position != expected || size == 0 || size > MAX_UNIT || end > encoded_bytes {
            bail!("Invalid or noncontiguous encoded packet");
        }
        ranges.push(UnitRange {
            position,
            size: usize::try_from(size)?,
            keyframe: packet.flags.contains('K'),
        });
        expected = end;
    }
    if expected != encoded_bytes {
        bail!("Incomplete encoded packet coverage");
    }
    let mut prepared = PreparedFile {
        file,
        path: encoded_path,
        packets: ranges,
        encoded_bytes,
        _directory: directory,
    };
    for index in 0..prepared.frames() {
        check_cancel(cancel)?;
        prepared.read_unit(index)?;
    }
    check_cancel(cancel)?;
    Ok(prepared)
}

#[derive(Deserialize)]
struct InputProbe {
    streams: Vec<Dimensions>,
    format: Option<InputFormat>,
}
#[derive(Deserialize)]
struct Dimensions {
    duration: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    width: u32,
    height: u32,
}
#[derive(Deserialize)]
struct InputFormat {
    duration: Option<String>,
}
#[derive(Deserialize)]
struct PacketProbe {
    packets: Vec<Packet>,
}
#[derive(Deserialize)]
struct Packet {
    pos: String,
    size: String,
    flags: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn persistent_vp9_import_keeps_all_586_frames_on_disk_and_cleans_up() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("long.mp4");
        let output = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=480x480:r=24",
                "-frames:v",
                "586",
                "-c:v",
                "libvpx-vp9",
                "-deadline",
                "realtime",
                "-cpu-used",
                "8",
                "-threads",
                "1",
            ])
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let cancel = AtomicBool::new(false);
        assert!(prepare(&source, Fit::Contain, &cancel, |_| {}).is_err());
        let mut prepared = prepare_file(&source, Fit::Contain, &cancel, |_| {}).unwrap();
        assert_eq!(prepared.frames(), 586);
        assert!(prepared.encoded_bytes() > 1024 * 1024);
        let staging = prepared._directory.path().to_owned();
        let restored = root.path().join("restored.h264");
        {
            use std::io::Write;
            let mut file = File::create(&restored).unwrap();
            for index in 0..prepared.frames() {
                let unit = prepared.read_unit(index).unwrap();
                file.write_all(&unit.bytes).unwrap();
            }
        }
        assert_eq!(
            fs::metadata(&restored).unwrap().len(),
            prepared.encoded_bytes()
        );
        assert!(prepared.read_unit(586).is_err());
        let output = Command::new("ffmpeg")
            .args(["-nostdin", "-v", "error", "-i"])
            .arg(&restored)
            .args(["-threads", "1", "-f", "framemd5", "-"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let frames = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            frames
                .lines()
                .filter(|line| !line.starts_with('#') && !line.is_empty())
                .count(),
            586
        );
        drop(prepared);
        assert!(!staging.exists());
        assert!(source.exists());
    }

    #[test]
    fn file_preparation_rejects_byte_budget_and_cancellation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("input.mp4");
        fixture(&source, "testsrc2=s=480x480:r=24", &[]);
        let cancel = AtomicBool::new(false);
        let error = prepare_bounded(
            &source,
            Fit::Contain.into(),
            &cancel,
            |_| {},
            240,
            1024,
            None,
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("byte budget"), "{error}");
        let mut stages = Vec::new();
        let result = prepare_file(&source, Fit::Contain, &cancel, |stage| {
            stages.push(stage);
            if stage == Stage::Validating {
                cancel.store(true, Ordering::Release);
            }
        });
        assert!(result.is_err());
        assert_eq!(stages.last(), Some(&Stage::Validating));
    }

    fn fixture(path: &Path, filter: &str, extra: &[&str]) {
        let result = Command::new("ffmpeg")
            .args([
                "-nostdin", "-v", "error", "-f", "lavfi", "-i", filter, "-t", "0.5", "-threads",
                "1",
            ])
            .args(extra)
            .arg(path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    #[test]
    fn autofill_retains_both_edges_and_fills_height_for_mp4_and_gif() {
        let root = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(false);
        for extension in ["mp4", "gif"] {
            let input = root.path().join(format!("edges.{extension}"));
            fixture(&input,"color=c=green:s=320x160:r=24,drawbox=x=0:y=0:w=80:h=160:color=red:t=fill,drawbox=x=240:y=0:w=80:h=160:color=blue:t=fill",&[]);
            for fit in [Fit::Stretch, Fit::Cover, Fit::Contain] {
                let clip = prepare(&input, fit, &cancel, |_| {}).unwrap();
                let png = thumbnail(&clip, &cancel).unwrap();
                let image = llcontrol_image::assets::decode_asset(&png)
                    .unwrap()
                    .to_rgb8();
                let left = image.get_pixel(14, 14).0;
                let right = image.get_pixel(130, 14).0;
                match fit {
                    Fit::Stretch => {
                        assert!(left[0] > 200 && left[1] < 30);
                        assert!(right[2] > 200 && right[0] < 30);
                    }
                    Fit::Cover => {
                        assert!(left[1] > 80 && left[0] < 30);
                        assert!(right[1] > 80 && right[2] < 30);
                    }
                    Fit::Contain => {
                        assert!(left.iter().chain(right.iter()).all(|c| *c < 8));
                    }
                }
            }
        }
    }

    #[test]
    fn real_mp4_and_looping_gif_decode_at_lcd_size_and_fit() {
        let root = tempfile::tempdir().unwrap();
        let cancelled = AtomicBool::new(false);
        for extension in ["mp4", "gif"] {
            let input = root.path().join(format!("input.{extension}"));
            fixture(&input, "color=c=red:s=320x160:r=24", &[]);
            let original = fs::read(&input).unwrap();
            for fit in [Fit::Contain, Fit::Cover, Fit::Stretch] {
                let mut stages = Vec::new();
                let clip = prepare(&input, fit, &cancelled, |s| stages.push(s)).unwrap();
                assert_eq!(
                    stages,
                    [Stage::Inspecting, Stage::Encoding, Stage::Validating]
                );
                assert!((11..=13).contains(&clip.units.len()));
                let png = thumbnail(&clip, &cancelled).unwrap();
                let preview = llcontrol_image::assets::decode_asset(&png)
                    .unwrap()
                    .to_rgb8();
                assert_eq!(preview.dimensions(), (144, 144));
                let top = preview.get_pixel(72, 0).0;
                match fit {
                    Fit::Contain => assert!(top.iter().all(|c| *c < 8)),
                    Fit::Cover | Fit::Stretch => {
                        assert!(top[0] > 230 && top[1] < 15 && top[2] < 15)
                    }
                }
                assert!(thumbnail(&clip, &AtomicBool::new(true)).is_err());
                let encoded = root.path().join("verify.h264");
                fs::write(
                    &encoded,
                    clip.units
                        .iter()
                        .flat_map(|u| u.bytes.iter().copied())
                        .collect::<Vec<_>>(),
                )
                .unwrap();
                let frame = Command::new("ffmpeg")
                    .args(["-nostdin", "-v", "error", "-i"])
                    .arg(&encoded)
                    .args([
                        "-frames:v",
                        "1",
                        "-pix_fmt",
                        "rgb24",
                        "-threads",
                        "1",
                        "-f",
                        "rawvideo",
                        "pipe:1",
                    ])
                    .output()
                    .unwrap();
                assert!(frame.status.success());
                assert_eq!(frame.stdout.len(), 480 * 480 * 3);
                let top = &frame.stdout[3 * 240..3 * 240 + 3];
                match fit {
                    Fit::Contain => assert!(top.iter().all(|c| *c < 8)),
                    Fit::Cover | Fit::Stretch => {
                        assert!(top[0] > 230 && top[1] < 15 && top[2] < 15)
                    }
                }
            }
            assert_eq!(fs::read(input).unwrap(), original);
        }
    }

    #[test]
    fn duration_and_dimensions_are_rejected_before_encoding() {
        let root = tempfile::tempdir().unwrap();
        for (name, filter, duration) in [
            ("long.mp4", "color=c=red:s=64x64:r=24", "11"),
            ("wide.mp4", "color=c=red:s=4100x2:r=24", "0.5"),
        ] {
            let input = root.path().join(name);
            let output = Command::new("ffmpeg")
                .args([
                    "-nostdin", "-v", "error", "-f", "lavfi", "-i", filter, "-t", duration,
                    "-threads", "1",
                ])
                .arg(&input)
                .output()
                .unwrap();
            assert!(output.status.success());
            let mut stages = Vec::new();
            assert!(
                prepare(&input, Fit::Contain, &AtomicBool::new(false), |s| stages
                    .push(s))
                .is_err()
            );
            assert_eq!(stages, [Stage::Inspecting]);
        }
    }

    #[test]
    fn cancellation_at_phase_boundary_never_produces_a_clip() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("input.mp4");
        fixture(&input, "color=c=blue:s=64x64:r=24", &[]);
        let cancelled = AtomicBool::new(false);
        let result = prepare(&input, Fit::Contain, &cancelled, |stage| {
            if stage == Stage::Encoding {
                cancelled.store(true, Ordering::Release);
            }
        });
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    #[test]
    fn cancellation_reaps_running_tool_before_return() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("started");
        let cancelled = Arc::new(AtomicBool::new(false));
        let trigger = cancelled.clone();
        let watch = marker.clone();
        let thread = thread::spawn(move || {
            let start = Instant::now();
            while !watch.exists() && start.elapsed() < Duration::from_secs(3) {
                thread::sleep(Duration::from_millis(5));
            }
            assert!(watch.exists());
            trigger.store(true, Ordering::Release);
        });
        let started = Instant::now();
        let result = run(
            Command::new("python3")
                .args([
                    "-c",
                    "import os,pathlib,sys,time; p=pathlib.Path(sys.argv[1]); t=p.with_suffix('.tmp'); t.write_text(str(os.getpid())); t.replace(p); time.sleep(30)",
                ])
                .arg(&marker),
            root.path(),
            &cancelled,
        );
        thread.join().unwrap();
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert!(started.elapsed() < Duration::from_secs(5));
        let pid: u32 = fs::read_to_string(marker).unwrap().parse().unwrap();
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "cancelled child must be reaped, including zombie state"
        );
    }

    #[test]
    fn invalid_source_and_pre_cancelled_requests_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("broken.mp4");
        fs::write(&input, b"not a video").unwrap();
        assert!(prepare(&input, Fit::Contain, &AtomicBool::new(false), |_| {}).is_err());
        let mut called = false;
        assert!(
            prepare(&input, Fit::Contain, &AtomicBool::new(true), |_| called =
                true)
            .unwrap_err()
            .to_string()
            .contains("cancelled")
        );
        assert!(!called);
    }
    #[test]
    fn edited_video_applies_effects_to_whole_source_and_each_cut_before_padding() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.mp4");
        let status = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=0x404040:s=32x16:r=24:d=0.5",
                "-c:v",
                "libx264",
                "-threads",
                "1",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        let decode = |path: &Path, frames: usize| {
            let output = Command::new("ffmpeg")
                .args(["-v", "error", "-threads", "1", "-i"])
                .arg(path)
                .args([
                    "-frames:v",
                    &frames.to_string(),
                    "-pix_fmt",
                    "rgb24",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        };
        let original = decode(&source, 1);
        // Independent installed-SDK numerical oracle, not the implementation LUT.
        let oracle = include_bytes!("../tests/fixtures/video-effect-channels.bin");
        let expected: Vec<u8> = original[..3]
            .iter()
            .map(|&value| {
                let contrasted = oracle[32 * 256 + usize::from(value)];
                oracle[65536 + 128 * 256 + usize::from(contrasted)]
            })
            .collect();
        let cancel = AtomicBool::new(false);
        let cut = || VideoCut {
            segments: vec![
                Segment {
                    start_ms: 250,
                    end_ms: 500,
                    crop: Some(VideoCrop {
                        x: 0.25,
                        y: 0.0,
                        width: 0.5,
                        height: 1.0,
                    }),
                },
                Segment {
                    start_ms: 0,
                    end_ms: 250,
                    crop: None,
                },
            ],
        };
        for selection in [None, Some(cut())] {
            let cropped_first = selection.is_some();
            let prepared = prepare_file_edited(
                &source,
                Fit::Contain,
                selection,
                VideoAdjustments {
                    contrast: 32,
                    brightness: 128,
                },
                &cancel,
                |_| {},
            )
            .unwrap();
            assert_eq!(prepared.frames(), 12);
            let pixels = decode(&prepared.path, 12);
            assert_eq!(pixels.len(), 12 * 480 * 480 * 3);
            for (index, frame) in pixels.chunks_exact(480 * 480 * 3).enumerate() {
                let center = (240 * 480 + 240) * 3;
                for channel in 0..3 {
                    assert!(
                        (i16::from(frame[center + channel]) - i16::from(expected[channel])).abs()
                            <= 4,
                        "frame {index}, channel {channel}: {} vs {}",
                        frame[center + channel],
                        expected[channel]
                    );
                }
                if !cropped_first || index >= 6 {
                    assert!(
                        frame[..3].iter().all(|&value| value <= 4),
                        "padding must remain black"
                    );
                }
            }
        }
        for selection in [None, Some(cut())] {
            let mut legacy =
                prepare_file_cut(&source, Fit::Contain, selection.clone(), &cancel, |_| {})
                    .unwrap();
            let mut neutral = prepare_file_edited(
                &source,
                Fit::Contain,
                selection,
                VideoAdjustments::default(),
                &cancel,
                |_| {},
            )
            .unwrap();
            assert_eq!(legacy.frames(), neutral.frames());
            for index in 0..legacy.frames() {
                assert_eq!(
                    legacy.read_unit(index).unwrap().bytes,
                    neutral.read_unit(index).unwrap().bytes
                );
            }
        }
    }

    #[test]
    fn video_cut_reorders_repeats_and_decodes_only_selected_intervals() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.mp4");
        let status=Command::new("ffmpeg").args(["-v","error","-f","lavfi","-i",
            "color=red:s=32x32:r=24:d=1[r];color=blue:s=32x32:r=24:d=1[b];color=lime:s=32x32:r=24:d=1[g];[r][b][g]concat=n=3:v=1:a=0",
            "-c:v","libx264","-threads","1","-g","72"]).arg(&source).status().unwrap();
        assert!(status.success());
        let cut = VideoCut {
            segments: vec![
                Segment {
                    start_ms: 2250,
                    end_ms: 2750,
                    crop: None,
                },
                Segment {
                    start_ms: 250,
                    end_ms: 750,
                    crop: None,
                },
                Segment {
                    start_ms: 2250,
                    end_ms: 2750,
                    crop: None,
                },
            ],
        };
        let cancel = AtomicBool::new(false);
        let mut prepared =
            prepare_file_cut(&source, Fit::Contain, Some(cut), &cancel, |_| {}).unwrap();
        assert_eq!(prepared.frames(), 36);
        let joined = root.path().join("result.h264");
        let mut bytes = Vec::new();
        for i in 0..prepared.frames() {
            let unit = prepared.read_unit(i).unwrap();
            if i % 12 == 0 {
                assert!(unit.keyframe);
            }
            bytes.extend(unit.bytes);
        }
        fs::write(&joined, bytes).unwrap();
        let decoded = Command::new("ffmpeg")
            .args(["-v", "error", "-threads", "1", "-f", "h264", "-i"])
            .arg(joined)
            .args([
                "-vf",
                "scale=1:1",
                "-threads",
                "1",
                "-pix_fmt",
                "rgb24",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        assert_eq!(decoded.stdout.len(), 36 * 3);
        for (i, rgb) in decoded.stdout.chunks_exact(3).enumerate() {
            let channel = if (12..24).contains(&i) { 0 } else { 1 };
            assert!(rgb[channel] > 200, "frame {i}: {rgb:?}");
            assert!(rgb[2] < 30);
        }
        let stage = prepared.staging_path().to_owned();
        drop(prepared);
        assert!(!stage.exists());
        assert!(source.exists());
    }

    #[test]
    fn unknown_or_invalid_source_rates_remain_unavailable() {
        for rate in [
            None,
            Some("0/0"),
            Some("0/1"),
            Some("30/0"),
            Some("N/A"),
            Some("-30/1"),
            Some("29.97"),
            Some("30000/1001/2"),
            Some("4294967296/1"),
        ] {
            assert_eq!(SourceFrameRate::parse(rate), None, "{rate:?}");
        }
        assert_eq!(
            SourceFrameRate::parse(Some("24/1")),
            Some(SourceFrameRate {
                numerator: 24,
                denominator: 1
            })
        );
    }

    #[test]
    fn fractional_cut_uses_source_relative_time_with_nonzero_container_start() {
        let root = tempfile::tempdir().unwrap();
        for offset in [0, 5] {
            let source = root.path().join(format!("offset-{offset}.mp4"));
            let status = Command::new("ffmpeg").args([
                "-v", "error", "-f", "lavfi", "-i",
                "color=red:s=32x32:r=30000/1001:d=1[r];color=blue:s=32x32:r=30000/1001:d=1[b];[r][b]concat=n=2:v=1:a=0",
                "-c:v", "libx264", "-threads", "1", "-pix_fmt", "yuv420p",
                "-output_ts_offset", &offset.to_string(),
            ]).arg(&source).status().unwrap();
            assert!(status.success());
            let probe = Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=start_time,r_frame_rate",
                    "-of",
                    "json",
                ])
                .arg(&source)
                .output()
                .unwrap();
            assert!(probe.status.success());
            let info: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
            assert_eq!(info["streams"][0]["r_frame_rate"], "30000/1001");
            let start: f64 = info["streams"][0]["start_time"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            assert!((start - f64::from(offset)).abs() < 0.001);
            let metadata = inspect_file(&source, &AtomicBool::new(false)).unwrap();
            assert_eq!((metadata.width, metadata.height), (32, 32));
            assert!(
                (metadata.duration_seconds.unwrap() - 2.002).abs() < 0.001,
                "stream duration must not include the container timestamp offset"
            );
            let rate = SourceFrameRate {
                numerator: 30000,
                denominator: 1001,
            };
            assert_eq!(metadata.average_frame_rate, Some(rate));
            assert_eq!(metadata.nominal_frame_rate, Some(rate));
            assert!((rate.frame_seconds() - 1001.0 / 30000.0).abs() < f64::EPSILON);
            let json = serde_json::to_value(metadata).unwrap();
            assert_eq!(json["averageFrameRate"]["numerator"], 30000);
            assert_eq!(json["averageFrameRate"]["denominator"], 1001);
            assert!(inspect_file(&source, &AtomicBool::new(true))
                .unwrap_err()
                .to_string()
                .contains("cancelled"));
            let prepared = prepare_file_cut(
                &source,
                Fit::Contain,
                Some(VideoCut {
                    segments: vec![Segment {
                        start_ms: 1101,
                        end_ms: 1600,
                        crop: None,
                    }],
                }),
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
            assert_eq!(prepared.frames(), 12);
            let edited = prepare_video_edit(
                &source,
                Fit::Contain,
                crate::video_range::VideoEditRange {
                    start_seconds: 1.101001,
                    end_seconds: 1.600001,
                },
                VideoAdjustments::default(),
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
            assert_eq!(edited.frames(), 12);
            assert!(prepare_video_edit(
                &source,
                Fit::Contain,
                crate::video_range::VideoEditRange {
                    start_seconds: 0.0,
                    end_seconds: 2.002
                },
                VideoAdjustments::default(),
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err());
            let decoded = Command::new("ffmpeg")
                .args(["-v", "error", "-threads", "1", "-i"])
                .arg(&edited.path)
                .args([
                    "-vf",
                    "scale=1:1",
                    "-threads",
                    "1",
                    "-pix_fmt",
                    "rgb24",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ])
                .output()
                .unwrap();
            assert!(decoded.status.success());
            assert_eq!(decoded.stdout.len(), 36);
            for rgb in decoded.stdout.chunks_exact(3) {
                assert!(
                    rgb[2] > 200 && rgb[0] < 30 && rgb[1] < 30,
                    "offset {offset}: selected source-relative blue interval, got {rgb:?}"
                );
            }
        }
    }

    #[test]
    fn video_cut_crops_each_interval_before_fit_and_rejects_invalid_rectangles() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("halves.mp4");
        fixture(
            &source,
            "color=red:s=64x32:r=24,drawbox=x=32:y=0:w=32:h=32:color=blue:t=fill",
            &[],
        );
        let crop = |x| VideoCrop {
            x,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        let cancel = AtomicBool::new(false);
        let cut = VideoCut {
            segments: vec![
                Segment {
                    start_ms: 0,
                    end_ms: 250,
                    crop: Some(crop(0.5)),
                },
                Segment {
                    start_ms: 0,
                    end_ms: 250,
                    crop: Some(crop(0.0)),
                },
            ],
        };
        let mut prepared =
            prepare_file_cut(&source, Fit::Contain, Some(cut), &cancel, |_| {}).unwrap();
        assert_eq!(prepared.frames(), 12);
        let output = root.path().join("cropped.h264");
        let mut bytes = Vec::new();
        for index in 0..prepared.frames() {
            bytes.extend(prepared.read_unit(index).unwrap().bytes);
        }
        fs::write(&output, bytes).unwrap();
        let decoded = Command::new("ffmpeg")
            .args(["-v", "error", "-threads", "1", "-i"])
            .arg(&output)
            .args([
                "-vf",
                "scale=1:1",
                "-pix_fmt",
                "rgb24",
                "-f",
                "rawvideo",
                "-",
            ])
            .output()
            .unwrap();
        assert!(decoded.status.success());
        assert_eq!(decoded.stdout.len(), 36);
        for (index, rgb) in decoded.stdout.chunks_exact(3).enumerate() {
            let wanted = if index < 6 { 2 } else { 0 };
            assert!(rgb[wanted] > 200, "frame {index}: {rgb:?}");
            assert!(rgb[1] < 30, "frame {index}: {rgb:?}");
        }
        let mut preview = prepared.preview(&cancel).unwrap();
        let preview_path = preview._directory.path().join("preview.mp4");
        let hashes = |path: &Path| {
            let output = Command::new("ffmpeg")
                .args(["-v", "error", "-threads", "1", "-i"])
                .arg(path)
                .args(["-f", "framemd5", "-"])
                .output()
                .unwrap();
            assert!(output.status.success());
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .filter(|line| !line.starts_with('#') && !line.is_empty())
                .map(|line| line.rsplit(',').next().unwrap().trim().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(hashes(&output), hashes(&preview_path));
        let mut streamed = Vec::new();
        while (streamed.len() as u64) < preview.length() {
            streamed.extend(preview.read_range(streamed.len() as u64, 113).unwrap());
        }
        assert_eq!(streamed, fs::read(&preview_path).unwrap());
        assert_eq!(
            preview.read_range(preview.length() - 2, 1024).unwrap(),
            streamed[streamed.len() - 2..]
        );
        assert!(preview.read_range(0, 0).is_err());
        assert!(preview.read_range(0, 256 * 1024 + 1).is_err());
        assert!(preview.read_range(u64::MAX, 1).is_err());
        assert!(preview.read_range(preview.length(), 1).is_err());
        let directory = preview._directory.path().to_owned();
        drop(preview);
        assert!(!directory.exists());
        assert!(prepared.path.exists());
        assert_eq!(prepared.frames(), 12);
        let cancelled = AtomicBool::new(true);
        assert!(prepared
            .preview(&cancelled)
            .err()
            .unwrap()
            .to_string()
            .contains("cancelled"));

        for bad in [
            VideoCrop {
                x: -0.1,
                ..crop(0.0)
            },
            VideoCrop {
                width: 0.0,
                ..crop(0.0)
            },
            VideoCrop {
                width: f64::NAN,
                ..crop(0.0)
            },
            crop(0.6),
        ] {
            let cut = VideoCut {
                segments: vec![Segment {
                    start_ms: 0,
                    end_ms: 250,
                    crop: Some(bad),
                }],
            };
            let mut inspected = false;
            let error = prepare_file_cut(&source, Fit::Contain, Some(cut), &cancel, |_| {
                inspected = true
            })
            .err()
            .unwrap();
            assert!(error.to_string().contains("Crop"));
            assert!(!inspected);
        }
        let legacy: VideoCut =
            serde_json::from_str(r#"{"segments":[{"start_ms":0,"end_ms":250}]}"#).unwrap();
        assert!(legacy.segments[0].crop.is_none());
    }

    #[test]
    fn video_cut_accepts_browser_millisecond_eof_rounding_but_not_real_overrun() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("fractional.mp4");
        let status = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=red:s=32x32:r=24",
                "-frames:v",
                "25",
                "-c:v",
                "libx264",
                "-threads",
                "1",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        let cancel = AtomicBool::new(false);
        let selection = |end_ms| {
            Some(VideoCut {
                segments: vec![Segment {
                    start_ms: 0,
                    end_ms,
                    crop: None,
                }],
            })
        };
        // 25/24 = 1.041666... video seconds; the MP4 movie exposes 1.042.
        let prepared =
            prepare_file_cut(&source, Fit::Contain, selection(1042), &cancel, |_| {}).unwrap();
        assert_eq!(prepared.frames(), 25);
        let mut stages = Vec::new();
        let error = prepare_file_cut(&source, Fit::Contain, selection(1044), &cancel, |s| {
            stages.push(s)
        })
        .err()
        .unwrap();
        assert!(error.to_string().contains("beyond"));
        assert_eq!(stages, [Stage::Inspecting]);
    }

    #[test]
    fn video_cut_rejects_bad_ranges_and_source_overrun_and_cancels() {
        for segments in [
            vec![],
            vec![Segment {
                start_ms: 4,
                end_ms: 4,
                crop: None,
            }],
            vec![Segment {
                start_ms: 0,
                end_ms: 180001,
                crop: None,
            }],
            vec![Segment {
                start_ms: u64::MAX,
                end_ms: 1,
                crop: None,
            }],
        ] {
            assert!(VideoCut { segments }.validate().is_err());
        }
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("input.mp4");
        fixture(&source, "color=red:s=32x32:r=24", &[]);
        let cut = VideoCut {
            segments: vec![Segment {
                start_ms: 0,
                end_ms: 1000,
                crop: None,
            }],
        };
        let cancel = AtomicBool::new(false);
        let mut stages = Vec::new();
        let error = prepare_file_cut(&source, Fit::Contain, Some(cut), &cancel, |s| {
            stages.push(s)
        })
        .err()
        .unwrap();
        assert!(error.to_string().contains("beyond"), "{error}");
        assert_eq!(stages, [Stage::Inspecting]);
        let cut = VideoCut {
            segments: vec![
                Segment {
                    start_ms: 0,
                    end_ms: 250,
                    crop: None,
                };
                2
            ],
        };
        let result = prepare_file_cut(&source, Fit::Contain, Some(cut), &cancel, |s| {
            if s == Stage::Validating {
                cancel.store(true, Ordering::Release)
            }
        });
        assert!(result.is_err());
        assert!(source.exists());
    }
}
