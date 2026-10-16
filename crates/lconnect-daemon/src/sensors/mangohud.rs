//! Explicit per-frame MangoHud CSV source. No injection, process discovery,
//! directory guessing or interpretation as compositor/refresh FPS.
use super::reading;
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};
const HEADER:&str="fps,frametime,cpu_load,cpu_power,gpu_load,cpu_temp,gpu_temp,gpu_core_clock,gpu_mem_clock,gpu_vram_used,gpu_power,ram_used,swap_used,process_rss,cpu_mhz,elapsed";
const HEAD: u64 = 8192;
const BATCH: u64 = 256 * 1024;
struct Session {
    file: File,
    dev: u64,
    ino: u64,
    offset: u64,
    last_elapsed: Option<u64>,
}
pub(super) struct MangoHud {
    path: PathBuf,
    session: Option<Session>,
}
impl MangoHud {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            session: None,
        }
    }
    pub(super) fn reset(&mut self) {
        self.session = None;
    }
    pub(super) fn collect(&mut self, now: u64) -> SensorReading {
        let (id, value) = match self.sample() {
            Ok(result) => result,
            Err(status) => {
                self.session = None;
                ("mangohud:discovery".into(), Err(status))
            }
        };
        let name = self
            .path
            .file_name()
            .and_then(|p| p.to_str())
            .filter(|p| p.len() <= 128 && !p.chars().any(char::is_control))
            .unwrap_or("configured log");
        reading(
            id,
            format!("MangoHud logged frame mean: {name}"),
            SensorUnit::FramesPerSecond,
            value,
            now,
        )
    }
    fn sample(&mut self) -> Result<(String, Result<f64, SensorStatus>), SensorStatus> {
        if !self.path.is_absolute() {
            return Err(SensorStatus::Invalid);
        }
        // O_NONBLOCK prevents a configured FIFO from blocking before fstat can
        // reject it. This collector only accepts regular no-follow log files.
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(
                rustix::fs::OFlags::NOFOLLOW.bits() as i32
                    | rustix::fs::OFlags::NONBLOCK.bits() as i32,
            )
            .open(&self.path)
            .map_err(|_| SensorStatus::Unavailable)?;
        let meta = file.metadata().map_err(|_| SensorStatus::Unavailable)?;
        if !meta.is_file() {
            return Err(SensorStatus::Invalid);
        }
        let path = self
            .path
            .canonicalize()
            .map_err(|_| SensorStatus::Unavailable)?;
        let bytes = path.as_os_str().as_encoded_bytes();
        if bytes.len() > 384 {
            return Err(SensorStatus::Invalid);
        }
        let encoded: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let id = format!(
            "mangohud:{encoded}:{:x}:{:x}:logged_frame_mean",
            meta.dev(),
            meta.ino()
        );
        let replacement = self
            .session
            .as_ref()
            .is_none_or(|s| s.dev != meta.dev() || s.ino != meta.ino() || meta.len() < s.offset);
        if replacement {
            let mut head = String::new();
            (&mut file)
                .take(HEAD)
                .read_to_string(&mut head)
                .map_err(|_| SensorStatus::Invalid)?;
            if !head
                .lines()
                .take(10)
                .any(|s| s.trim_end_matches('\r') == HEADER)
            {
                return Err(SensorStatus::Invalid);
            }
            // Never turn an existing benchmark into a current reading. Only
            // complete frames appended after attachment can become usable.
            self.session = Some(Session {
                file,
                dev: meta.dev(),
                ino: meta.ino(),
                offset: meta.len(),
                last_elapsed: None,
            });
            return Ok((id, Err(SensorStatus::Unavailable)));
        }
        let s = self.session.as_mut().ok_or(SensorStatus::Unavailable)?;
        let length = meta.len() - s.offset;
        if length == 0 {
            return Ok((id, Err(SensorStatus::Unavailable)));
        }
        if length > BATCH {
            s.offset = meta.len();
            s.last_elapsed = None;
            return Ok((id, Err(SensorStatus::Invalid)));
        }
        s.file
            .seek(SeekFrom::Start(s.offset))
            .map_err(|_| SensorStatus::Unavailable)?;
        let mut bytes = vec![0; length as usize];
        s.file
            .read_exact(&mut bytes)
            .map_err(|_| SensorStatus::Unavailable)?;
        let Some(end) = bytes.iter().rposition(|b| *b == b'\n') else {
            return if length > 1024 {
                Err(SensorStatus::Invalid)
            } else {
                Ok((id, Err(SensorStatus::Unavailable)))
            };
        };
        s.offset += end as u64 + 1;
        let text = std::str::from_utf8(&bytes[..=end]).map_err(|_| SensorStatus::Invalid)?;
        let mut duration = 0.0;
        let mut count = 0u32;
        let mut last = s.last_elapsed;
        for line in text.lines() {
            let (frame, elapsed) = parse_frame(line)?;
            if last.is_some_and(|p| elapsed <= p) {
                return Err(SensorStatus::Invalid);
            }
            last = Some(elapsed);
            duration += frame;
            count += 1;
        }
        s.last_elapsed = last;
        if count == 0 || !duration.is_finite() {
            return Err(SensorStatus::Invalid);
        }
        Ok((id, Ok(1000.0 * f64::from(count) / duration)))
    }
}
fn parse_frame(line: &str) -> Result<(f64, u64), SensorStatus> {
    if line.len() > 1024 {
        return Err(SensorStatus::Invalid);
    }
    let fields: Vec<_> = line.trim_end_matches('\r').split(',').collect();
    if fields.len() != 16 {
        return Err(SensorStatus::Invalid);
    }
    let fps = fields[0]
        .parse::<f64>()
        .map_err(|_| SensorStatus::Invalid)?;
    let ms = fields[1]
        .parse::<f64>()
        .map_err(|_| SensorStatus::Invalid)?;
    let elapsed = fields[15]
        .parse::<u64>()
        .map_err(|_| SensorStatus::Invalid)?;
    if !fps.is_finite()
        || !ms.is_finite()
        || ms <= 0.0
        || fps <= 0.0
        || fps > 1_000_000.0
        || ms > 60_000.0
        || (fps * ms / 1000.0 - 1.0).abs() > 0.01
    {
        return Err(SensorStatus::Invalid);
    }
    Ok((ms, elapsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn fixture() -> (tempfile::TempDir, MangoHud) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("game.csv");
        std::fs::write(&path,format!("os,cpu,gpu,ram,kernel,driver,cpuscheduler\nLinux,CPU,GPU,32G,fixture,driver,none\n{HEADER}\n")).unwrap();
        (root, MangoHud::new(path))
    }
    fn frame(ms: f64, elapsed: u64) -> String {
        format!("{},{ms},0,0,0,0,0,0,0,0,0,0,0,0,0,{elapsed}\n", 1000.0 / ms)
    }
    fn append(p: &PathBuf, s: &str) {
        let mut f = OpenOptions::new().append(true).open(p).unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }
    #[test]
    fn old_benchmark_is_not_live_and_new_frames_use_duration_mean_not_arithmetic_fps() {
        let (_root, mut source) = fixture();
        append(&source.path, &frame(10.0, 1));
        assert_eq!(source.collect(10).status, SensorStatus::Unavailable);
        append(&source.path, &(frame(10.0, 2) + &frame(30.0, 3)));
        let row = source.collect(20);
        assert_eq!(row.unit, SensorUnit::FramesPerSecond);
        assert_eq!(row.value, Some(50.0));
        assert!(row.id.starts_with("mangohud:"));
        assert_eq!(
            source.collect(30).value,
            None,
            "unchanged bytes are not a fresh frame"
        );
        source.reset();
        assert_eq!(source.collect(40).value, None);
    }
    #[test]
    fn partial_invalid_and_replayed_records_revoke_values_and_recover_after_warmup() {
        let (_root, mut source) = fixture();
        source.collect(0);
        let line = frame(20.0, 10);
        append(&source.path, &line[..line.len() - 1]);
        assert_eq!(source.collect(1).value, None);
        append(&source.path, "\n");
        assert_eq!(source.collect(2).value, Some(50.0));
        append(&source.path, &frame(20.0, 10));
        assert_eq!(source.collect(3).status, SensorStatus::Invalid);
        assert_eq!(source.collect(4).value, None);
        append(&source.path, &frame(40.0, 20));
        assert_eq!(source.collect(5).value, Some(25.0));
        for bad in [
            "NaN,20,0,0,0,0,0,0,0,0,0,0,0,0,0,30",
            "50,0,0,0,0,0,0,0,0,0,0,0,0,0,0,40",
            "60,20,0,0,0,0,0,0,0,0,0,0,0,0,0,50",
            "wrong",
        ] {
            append(&source.path, &format!("{bad}\n"));
            assert_eq!(source.collect(6).value, None);
            source.collect(7);
        }
    }
    #[test]
    fn replaced_file_changes_identity_and_never_replays_old_content() {
        let (root, mut source) = fixture();
        source.collect(0);
        append(&source.path, &frame(10.0, 1));
        let id = source.collect(1).id;
        let next = root.path().join("new.csv");
        std::fs::write(&next, format!("{HEADER}\n{}", frame(20.0, 2))).unwrap();
        std::fs::rename(next, &source.path).unwrap();
        let replaced = source.collect(2);
        assert_ne!(replaced.id, id);
        assert_eq!(replaced.value, None);
        append(&source.path, &frame(20.0, 3));
        assert_eq!(source.collect(3).value, Some(50.0));
    }
    #[test]
    fn symlink_fifo_wrong_schema_and_output_budgets_never_block_or_invent_values() {
        let (root, mut source) = fixture();
        source.collect(0);
        append(&source.path, &"x".repeat(BATCH as usize + 1));
        assert_eq!(source.collect(1).status, SensorStatus::Invalid);
        std::fs::remove_file(&source.path).unwrap();
        std::os::unix::fs::symlink("missing", &source.path).unwrap();
        assert_eq!(source.collect(2).value, None);
        std::fs::remove_file(&source.path).unwrap();
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &source.path,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        assert_eq!(source.collect(3).value, None);
        std::fs::remove_file(&source.path).unwrap();
        std::fs::write(&source.path, "fps,elapsed\n60,1\n").unwrap();
        assert_eq!(source.collect(4).status, SensorStatus::Invalid);
        let mut relative = MangoHud::new("game.csv".into());
        assert_eq!(relative.collect(0).status, SensorStatus::Invalid);
        assert!(root.path().exists());
    }
}
