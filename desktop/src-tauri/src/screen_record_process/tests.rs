use super::*;

#[test]
fn closed_worker_socket_has_the_same_recovery_error_on_read_and_write() {
    for writing in [false, true] {
        let (mut socket, peer) = UnixStream::pair().unwrap();
        drop(peer);
        let error = transfer(&mut socket, &mut [0], writing, &|| Ok(())).unwrap_err();
        assert_eq!(error.to_string(), "Recording worker connection closed");
        if writing {
            // Keep the original errno available for diagnostics while callers
            // receive the same stable recovery class as a zero-byte read.
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::BrokenPipe
            );
        }
    }
}

// Re-exec the ordinary Rust test executable so cargo test needs no prebuilt GUI
// or extra installed worker binary. Production re-execs lconnect-gui before Tauri.
#[test]
fn worker_entry() {
    if std::env::var_os("LCONNECT_TEST_RECORD_WORKER").is_some() {
        let result = worker_main();
        if let Err(error) = &result {
            eprintln!("test recording worker: {error:#}");
        }
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
}
pub(super) fn fault_worker(socket: &mut UnixStream, init: &Init, mode: &str) -> Result<()> {
    if let Some(descriptor) = mode.strip_prefix("exclude:") {
        let fd: i32 = descriptor.parse()?;
        ensure!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::EBADF),
            "Unexpected inherited descriptor"
        );
    }
    if mode == "startup" {
        loop {
            std::thread::park();
        }
    }
    if mode == "fd" {
        // SAFETY: the parent supplied a private readable descriptor as4.
        let mut file = std::fs::File::from(unsafe { OwnedFd::from_raw_fd(4) });
        let mut value = [0; 8];
        file.read_exact(&mut value)?;
        ensure!(&value == b"selected", "Wrong inherited descriptor");
    }
    let recorder =
        screen_record::Recorder::prepare_worker(None, 0, None, init.directory.clone(), true, true)?;
    write_message(socket, &Reply::Status(recorder.status()), &|| Ok(()))?;
    let _: Request = read_message(socket, &|| Ok(()))?;
    match mode {
        "crash" => std::process::exit(73),
        "oversize" => {
            socket.write_all(&((MAX_MESSAGE + 1) as u32).to_be_bytes())?;
            loop {
                std::thread::park();
            }
        }
        "fd" => {
            write_message(socket, &Reply::Status(recorder.status()), &|| Ok(()))?;
            loop {
                std::thread::park();
            }
        }
        _ => loop {
            std::thread::park();
        },
    }
}
fn pid(recorder: &Recorder) -> u32 {
    recorder.worker.lock().unwrap().child.id()
}
fn reaped(pid: u32) {
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "worker {pid} was not reaped"
    );
}
#[test]
fn isolated_recording_setup_crop_pause_resume_finish_preserves_owned_mp4() {
    let recorder = Recorder::synthetic_prepare().unwrap();
    let child = pid(&recorder);
    let root = recorder.directory.as_ref().unwrap().path().to_owned();
    assert_eq!(recorder.status().phase, Phase::Setup);
    let until = Instant::now() + Duration::from_secs(3);
    let jpeg = loop {
        match recorder.snapshot() {
            Ok(jpeg) => break jpeg,
            Err(_) => {
                assert!(Instant::now() < until);
                std::thread::sleep(IO_TICK);
            }
        }
    };
    assert_eq!(&jpeg[..2], &[255, 216]);
    let region = VideoCrop {
        x: 0.1,
        y: 0.1,
        width: 0.5,
        height: 0.5,
    };
    recorder.set_region(region).unwrap();
    recorder.resume().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    recorder.pause().unwrap();
    assert_eq!(recorder.status().region, Some(region));
    recorder.resume().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let file = recorder.finish().unwrap();
    reaped(child);
    assert!(file.frames > 5 && file.length() > 0);
    assert!(file.path().is_file());
    let metadata = crate::media_prepare::inspect_file(
        &file.path(),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    assert!(metadata
        .duration_seconds
        .is_some_and(|duration| duration > 0.0));
    drop(file);
    assert!(!root.exists());
}
#[test]
fn stalled_worker_cancel_joins_before_staging_release_and_allows_new_recording() {
    let (cancel, cancelled) = watch::channel(false);
    let recorder = Recorder::open(
        Source::Fault("command".into()),
        None,
        true,
        Some(cancelled),
        None,
    )
    .unwrap();
    let child = pid(&recorder);
    let root = recorder.directory.as_ref().unwrap().path().to_owned();
    let killer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        cancel.send_replace(true);
    });
    let now = Instant::now();
    let status = recorder.status();
    assert!(matches!(status.phase, Phase::Failed(ref message) if message.contains("cancelled")));
    assert!(now.elapsed() < Duration::from_secs(2));
    reaped(child);
    assert!(root.is_dir()); // cleanup has not abandoned the parent's lease
    drop(recorder);
    assert!(!root.exists());
    killer.join().unwrap();
    let next = Recorder::synthetic().unwrap();
    let child = pid(&next);
    drop(next);
    reaped(child);
}
#[test]
fn startup_cancellation_interrupts_missing_native_acknowledgement() {
    let (cancel, cancelled) = watch::channel(false);
    let killer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        cancel.send_replace(true);
    });
    let now = Instant::now();
    let result = Recorder::open(
        Source::Fault("startup".into()),
        None,
        true,
        Some(cancelled),
        None,
    );
    assert!(result.is_err());
    assert!(now.elapsed() < Duration::from_secs(2));
    killer.join().unwrap();
}
#[test]
fn command_deadline_crash_and_oversized_reply_reap_worker_without_a_ready_file() {
    for mode in ["command", "crash", "oversize"] {
        let recorder = Recorder::open(Source::Fault(mode.into()), None, true, None, None).unwrap();
        let child = pid(&recorder);
        let root = recorder.directory.as_ref().unwrap().path().to_owned();
        let now = Instant::now();
        let status = recorder.status();
        assert!(
            matches!(status.phase, Phase::Failed(_)),
            "{mode}: {status:?}"
        );
        assert!(now.elapsed() < Duration::from_secs(8), "{mode}");
        reaped(child);
        assert!(!root.join("recording.mp4").exists());
        drop(recorder);
        assert!(!root.exists());
    }
}
#[test]
fn portal_closed_interrupts_stalled_finish_and_reaps_before_return() {
    let (closed, receiver) = watch::channel(false);
    let recorder = Recorder::open(
        Source::Fault("command".into()),
        None,
        true,
        None,
        Some(receiver),
    )
    .unwrap();
    let child = pid(&recorder);
    let root = recorder.directory.as_ref().unwrap().path().to_owned();
    let closer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        closed.send_replace(true);
    });
    let now = Instant::now();
    assert!(recorder.finish().is_err());
    assert!(now.elapsed() < Duration::from_secs(2));
    closer.join().unwrap();
    reaped(child);
    assert!(!root.exists());
}
#[test]
fn selected_fd_is_inherited_without_consuming_the_parent_descriptor() {
    let (mut writer, reader) = UnixStream::pair().unwrap();
    writer.write_all(b"selected").unwrap();
    let descriptor = reader.try_clone().unwrap().into();
    let recorder = Recorder::open(
        Source::Fault("fd".into()),
        Some(descriptor),
        true,
        None,
        None,
    )
    .unwrap();
    assert_eq!(recorder.status().phase, Phase::Setup);
    let child = pid(&recorder);
    drop(recorder);
    reaped(child);
    writer.write_all(b"parent").unwrap();
    let mut bytes = [0; 6];
    (&reader).read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"parent");
}

#[test]
fn unrelated_non_cloexec_descriptor_is_excluded_but_parent_keeps_it() {
    let file = std::fs::File::open("/dev/null").unwrap();
    let sentinel = rustix::io::fcntl_dupfd_cloexec(&file, 200).unwrap();
    // SAFETY: sentinel is a live owned descriptor; only its flags change.
    assert_eq!(
        unsafe { libc::fcntl(sentinel.as_raw_fd(), libc::F_SETFD, 0) },
        0
    );
    let recorder = Recorder::open(
        Source::Fault(format!("exclude:{}", sentinel.as_raw_fd())),
        None,
        true,
        None,
        None,
    )
    .unwrap();
    let child = pid(&recorder);
    drop(recorder);
    reaped(child);
    assert!(unsafe { libc::fcntl(sentinel.as_raw_fd(), libc::F_GETFD) } >= 0);
}

#[test]
fn pipewire_connection_poll_does_not_consume_pending_protocol_bytes() {
    use std::{
        io::{Read, Write},
        os::unix::net::UnixStream,
    };
    let (socket, mut remote) = UnixStream::pair().unwrap();
    let mut reader = socket.try_clone().unwrap();
    let fd: OwnedFd = socket.into();
    assert!(pipewire_connected(&fd).unwrap());
    remote.write_all(b"protocol").unwrap();
    assert!(pipewire_connected(&fd).unwrap());
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"protocol");
    remote.write_all(b"pending").unwrap();
    drop(remote);
    assert!(!pipewire_connected(&fd).unwrap());
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(&bytes, b"pending");
}

#[test]
fn disconnected_remote_interrupts_stalled_worker_before_command_deadline() {
    let mut recorder =
        Recorder::open(Source::Fault("command".into()), None, true, None, None).unwrap();
    let (socket, remote) = UnixStream::pair().unwrap();
    recorder.remote = Some(socket.into());
    let child = pid(&recorder);
    let root = recorder.directory.as_ref().unwrap().path().to_owned();
    let closer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        drop(remote);
    });
    let now = Instant::now();
    let status = recorder.status();
    assert!(
        matches!(status.phase, Phase::Failed(ref text) if text.contains("PipeWire connection closed"))
    );
    assert!(now.elapsed() < Duration::from_secs(2));
    closer.join().unwrap();
    reaped(child);
    assert!(root.exists()); // Parent still owns the lease until Recorder drops.
    drop(recorder);
    assert!(!root.exists());
}
