//! Two-process proof that the losing daemon exits before owner work and
//! leaves the winner's socket and connected clients in place.

use llcontrol_ipc::socket::{DaemonOwnership, SocketIdentity};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const TEST_NAME: &str = "second_process_loses_before_owner_work";
const HELLO: &[u8] = b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n";

#[test]
fn second_process_loses_before_owner_work() {
    if let Ok(role) = std::env::var("LLCONTROL_OWNERSHIP_CHILD") {
        if let Err(error) = child(&role) {
            let dir = PathBuf::from(std::env::var("LLCONTROL_OWNERSHIP_DIR").unwrap());
            fs::write(dir.join("loser-error"), error.to_string()).unwrap();
            std::process::exit(2);
        }
        return;
    }

    parent().unwrap();
}

fn child(role: &str) -> io::Result<()> {
    let dir = PathBuf::from(std::env::var("LLCONTROL_OWNERSHIP_DIR").unwrap());
    let ownership = DaemonOwnership::acquire(&dir.join("llcontrold.sock"))?;
    // The daemon reaches persistence, HID, and replay only after acquire returns.
    fs::write(
        dir.join(format!("owner-work-{role}-{}", std::process::id())),
        b"hid-and-replay",
    )?;
    if role == "winner" {
        fs::write(dir.join("winner-ready"), std::process::id().to_string())?;
        serve_until_stopped(&ownership, &dir)?;
    }
    Ok(())
}

fn serve_until_stopped(ownership: &DaemonOwnership, dir: &Path) -> io::Result<()> {
    let listener = ownership.listener()?;
    listener.set_nonblocking(true)?;
    while !dir.join("winner-stop").exists() {
        match listener.accept() {
            Ok((stream, _)) => {
                std::thread::spawn(move || handle_client(stream));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn handle_client(mut stream: UnixStream) {
    if stream.set_nonblocking(false).is_err() || stream.write_all(HELLO).is_err() {
        return;
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = [0u8; 16];
    if let Ok(n) = stream.read(&mut buf) {
        if buf[..n].starts_with(b"ping") {
            let _ = stream.write_all(b"pong\n");
            let _ = stream.read(&mut [0u8; 1]);
        }
    }
}

fn parent() -> io::Result<()> {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = temp.path();
    let socket = dir.join("llcontrold.sock");
    let mut winner = KillOnDrop(Some(spawn(dir, "winner")?));
    wait_until(
        dir.join("winner-ready"),
        Duration::from_secs(5),
        &mut winner,
    )?;
    let winner_pid = fs::read_to_string(dir.join("winner-ready"))?;
    let identity = SocketIdentity::of_path(&socket)?;

    let mut client = UnixStream::connect(&socket)?;
    client.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut hello = [0u8; HELLO.len()];
    client.read_exact(&mut hello)?;
    assert_eq!(&hello, HELLO);

    let mut loser = spawn(dir, "loser")?;
    let status = wait_deadline(&mut loser, Duration::from_secs(5))?;
    assert_eq!(status.code(), Some(2), "loser stderr: {}", read_loss(dir));
    let loss = fs::read_to_string(dir.join("loser-error"))?;
    assert!(
        loss.contains("another llcontrold holds the runtime lock"),
        "{loss}"
    );
    assert!(loss.contains(winner_pid.trim()), "{loss}");
    assert_eq!(SocketIdentity::of_path(&socket)?, identity);

    let owner_work = owner_work_files(dir)?;
    assert_eq!(owner_work.len(), 1, "{owner_work:?}");
    assert!(
        owner_work[0].contains("owner-work-winner-"),
        "{owner_work:?}"
    );

    client.write_all(b"ping\n")?;
    let mut pong = [0u8; 5];
    client.read_exact(&mut pong)?;
    assert_eq!(&pong, b"pong\n");

    let mut again = UnixStream::connect(&socket)?;
    again.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut hello = [0u8; HELLO.len()];
    again.read_exact(&mut hello)?;
    assert_eq!(&hello, HELLO);
    assert_eq!(SocketIdentity::of_path(&socket)?, identity);

    fs::write(dir.join("winner-stop"), b"1")?;
    let winner_status = wait_deadline(winner.0.as_mut().unwrap(), Duration::from_secs(5))?;
    winner.0.take();
    assert!(winner_status.success(), "winner failed: {}", read_log(dir));
    Ok(())
}

fn owner_work_files(dir: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("owner-work-") {
            names.push(name.into_owned());
        }
    }
    names.sort();
    Ok(names)
}

fn spawn(dir: &Path, role: &str) -> io::Result<Child> {
    let exe = std::env::current_exe()?;
    let mut command = Command::new(exe);
    command
        .arg(TEST_NAME)
        .arg("--exact")
        .env("LLCONTROL_OWNERSHIP_CHILD", role)
        .env("LLCONTROL_OWNERSHIP_DIR", dir)
        .stdout(Stdio::from(File::create(dir.join(format!("{role}.out")))?))
        .stderr(Stdio::from(File::create(dir.join(format!("{role}.err")))?));
    command.spawn()
}

fn wait_until(path: PathBuf, limit: Duration, winner: &mut KillOnDrop) -> io::Result<()> {
    let started = Instant::now();
    while !path.exists() {
        if let Some(status) = winner.0.as_mut().unwrap().try_wait()? {
            return Err(io::Error::other(format!(
                "winner exited early ({status}): {}",
                read_log(path.parent().unwrap())
            )));
        }
        if started.elapsed() > limit {
            return Err(io::Error::other(format!(
                "winner did not become ready: {}",
                read_log(path.parent().unwrap())
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn wait_deadline(child: &mut Child, limit: Duration) -> io::Result<ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "child timed out"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn read_loss(dir: &Path) -> String {
    fs::read_to_string(dir.join("loser-error")).unwrap_or_else(|_| read_log(dir))
}

fn read_log(dir: &Path) -> String {
    format!(
        "out={} err={}",
        fs::read_to_string(dir.join("winner.out")).unwrap_or_default(),
        fs::read_to_string(dir.join("winner.err")).unwrap_or_default()
    )
}

struct KillOnDrop(Option<Child>);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
