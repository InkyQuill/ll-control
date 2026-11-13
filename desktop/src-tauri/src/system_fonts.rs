//! Read-only fontconfig catalogue. The webview receives opaque IDs, never a file picker path.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{self, Read},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const CATALOGUE_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_CATALOGUE_BYTES: usize = 4 * 1024 * 1024;

// Poll the pipe in the same worker as the child. A reader thread could remain
// blocked forever if a helper (or one of its descendants) kept stdout open.
fn bounded_output(
    command: &mut Command,
    timeout: Duration,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("Fontconfig: {e}"))?;
    let mut reaped = false;
    let result = (|| {
        let mut stdout = child.stdout.take().ok_or("Fontconfig stdout unavailable")?;
        let fd = stdout.as_raw_fd();
        // SAFETY: fd belongs to the live stdout pipe; fcntl does not retain it.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(format!("Fontconfig pipe: {}", io::Error::last_os_error()));
        }
        let deadline = Instant::now() + timeout;
        let mut bytes = Vec::new();
        let mut eof = false;
        loop {
            if Instant::now() >= deadline {
                return Err("Fontconfig enumeration timed out".into());
            }
            let mut buffer = [0; 8192];
            if !eof {
                match stdout.read(&mut buffer) {
                    Ok(0) => eof = true,
                    Ok(count) => {
                        if count > limit.saturating_sub(bytes.len()) {
                            return Err("Fontconfig catalogue exceeds output limit".into());
                        }
                        bytes.extend_from_slice(&buffer[..count]);
                        continue;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(format!("Fontconfig output: {e}")),
                }
            }
            if eof {
                if let Some(status) = child.try_wait().map_err(|e| format!("Fontconfig: {e}"))? {
                    reaped = true;
                    return if status.success() {
                        Ok(bytes)
                    } else {
                        Err("Fontconfig could not list installed fonts".into())
                    };
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    if !reaped {
        // Descendants may hold the pipe. Only signal while the unreaped child
        // still reserves its PID, so a reused process group cannot be targeted.
        // SAFETY: the spawned child's PID is the process group ID set above.
        unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

#[derive(Clone, Serialize)]
pub struct SystemFont {
    pub id: String,
    pub name: String,
}
fn parse_catalogue(raw: &str) -> BTreeMap<String, (String, PathBuf)> {
    let mut fonts = BTreeMap::new();
    for line in raw.lines() {
        let mut fields = line.split('\t');
        let (Some(file), Some(family), Some(style), Some(index)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        // The renderer loads the default face; named variable instances cannot be selected.
        if index != "0" {
            continue;
        }
        let path = PathBuf::from(file);
        if !path.is_absolute()
            || !matches!(
                path.extension()
                    .and_then(|s| s.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("ttf" | "otf")
            )
        {
            continue;
        }
        let name = format!(
            "{} · {}",
            family.split(',').next().unwrap_or(family),
            style.split(',').next().unwrap_or(style)
        );
        let id = format!("{:x}", Sha256::digest(file.as_bytes()));
        fonts.insert(id, (name, path));
    }
    fonts
}
fn catalogue() -> Result<BTreeMap<String, (String, PathBuf)>, String> {
    let output = bounded_output(
        Command::new("fc-list").arg("--format=%{file}\t%{family}\t%{style}\t%{index}\n"),
        CATALOGUE_TIMEOUT,
        MAX_CATALOGUE_BYTES,
    )?;
    Ok(parse_catalogue(&String::from_utf8_lossy(&output)))
}
pub fn list() -> Result<Vec<SystemFont>, String> {
    let mut fonts: Vec<_> = catalogue()?
        .into_iter()
        .map(|(id, (name, _))| SystemFont { id, name })
        .collect();
    fonts.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(fonts)
}
pub fn read(id: &str) -> Result<Vec<u8>, String> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Unknown installed font ID".into());
    }
    let mut catalogue = catalogue()?;
    let (_, path) = catalogue
        .remove(id)
        .ok_or("Installed font is no longer available")?;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Font is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
        return Err("Font must be between 1 byte and 16 MiB".into());
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(script: &str, timeout: Duration, limit: usize) -> Result<Vec<u8>, String> {
        bounded_output(Command::new("sh").args(["-c", script]), timeout, limit)
    }

    #[test]
    fn subprocess_accepts_exact_limit_and_rejects_failed_exit() {
        assert_eq!(
            fixture("printf font", Duration::from_secs(2), 4).unwrap(),
            b"font"
        );
        assert!(fixture("printf font; exit 1", Duration::from_secs(2), 4)
            .unwrap_err()
            .contains("could not list"));
    }

    #[test]
    fn subprocess_bounds_output_while_producer_is_running() {
        let started = Instant::now();
        assert!(fixture(
            "while :; do printf 1234567890; done",
            Duration::from_secs(2),
            32
        )
        .unwrap_err()
        .contains("output limit"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn subprocess_times_out_and_reaps_stalled_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");
        let mut command = Command::new("sh");
        command
            .args(["-c", "echo $$ > \"$1\"; exec sleep 30", "fixture"])
            .arg(&pid_file);
        let started = Instant::now();
        assert!(bounded_output(&mut command, Duration::from_millis(150), 32)
            .unwrap_err()
            .contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(2));
        let pid = std::fs::read_to_string(pid_file).unwrap();
        assert!(!std::path::Path::new(&format!("/proc/{}", pid.trim())).exists());
    }

    #[test]
    fn subprocess_deadline_covers_eof_and_inherited_open_pipe() {
        for script in ["exec 1>&-; exec sleep 30", "sleep 30 & exit 0"] {
            let started = Instant::now();
            assert!(fixture(script, Duration::from_millis(100), 32)
                .unwrap_err()
                .contains("timed out"));
            assert!(started.elapsed() < Duration::from_secs(2));
        }
    }

    #[test]
    fn catalogue_filters_unsupported_collections_and_deduplicates_files() {
        let fonts=parse_catalogue("/fonts/a.ttf\tAlpha,Alternate\tRegular,Normal\t0\n/fonts/a.ttf\tAlpha\tRegular\t0\n/fonts/b.ttc\tCollection\tRegular\t0\nrelative.otf\tRelative\tRegular\t0\n/fonts/variable.ttf\tNamed\tBold\t65536\n/fonts/c.OTF\tGamma\tBold\t0\n");
        assert_eq!(fonts.len(), 2);
        assert!(fonts.values().any(|(name, _)| name == "Alpha · Regular"));
        assert!(fonts.keys().all(|id| id.len() == 64));
    }
    #[test]
    fn arbitrary_path_is_not_a_catalogue_id() {
        let fonts = parse_catalogue("/fonts/a.ttf\tAlpha\tRegular\t0\n");
        assert!(!fonts.contains_key("/fonts/a.ttf"));
        assert!(read("/fonts/a.ttf").is_err());
    }
}
