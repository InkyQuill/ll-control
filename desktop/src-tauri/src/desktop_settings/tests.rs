use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};
fn store(root: &Path, executable: &str) -> Store {
    let binary = root.join(executable);
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    if !binary.exists() {
        fs::write(&binary, b"#!/usr/bin/python3\n").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    }
    Store::new(root.join("app"), root.join("config"), root.join(executable)).unwrap()
}
#[test]
fn desired_and_actual_registration_survive_restart_move_disable_and_stale_writes() {
    let root = tempfile::tempdir().unwrap();
    let a = store(root.path(), "bin/lconnect-gui");
    let initial = a.read().unwrap();
    assert_eq!(initial.revision, 0);
    assert_eq!(initial.registration.enabled, Some(false));
    assert!(!a.autostart.exists());
    let wanted = Preferences {
        autostart: true,
        start_minimized: true,
        close_to_tray: true,
    };
    let enabled = a.apply(0, wanted.clone()).unwrap();
    assert_eq!(enabled.revision, 1);
    assert_eq!(enabled.registration.enabled, Some(true));
    assert!(enabled.integration_error.is_none());
    assert_eq!(a.apply(1, wanted.clone()).unwrap().revision, 1);
    let moved = store(root.path(), "new bin/lconnect-gui");
    let update = moved.apply(1, wanted.clone()).unwrap();
    assert_eq!(update.revision, 2);
    assert_eq!(update.registration.enabled, Some(true));
    assert!(String::from_utf8(fs::read(&moved.autostart).unwrap())
        .unwrap()
        .contains("new bin/lconnect-gui"));
    assert!(a.apply(1, Preferences::default()).is_err());
    let disabled = moved
        .apply(
            2,
            Preferences {
                autostart: false,
                ..wanted
            },
        )
        .unwrap();
    assert_eq!(disabled.registration.enabled, Some(false));
    assert!(!moved.autostart.exists());
    assert_eq!(
        store(root.path(), "third/lconnect-gui")
            .read()
            .unwrap()
            .preferences,
        disabled.preferences
    );
}
#[test]
fn integration_failure_preserves_durable_intent_and_retry_repairs_only_owned_file() {
    let root = tempfile::tempdir().unwrap();
    let a = store(root.path(), "bin/lconnect-gui");
    fs::create_dir_all(root.path().join("config")).unwrap();
    fs::write(root.path().join("config/autostart"), b"blocked directory").unwrap();
    let requested = Preferences {
        autostart: true,
        ..Default::default()
    };
    let failed = a.apply(0, requested.clone()).unwrap();
    assert!(failed.integration_error.is_some());
    assert!(a.read().unwrap().preferences.autostart);
    assert_eq!(failed.registration.enabled, None);
    fs::remove_file(root.path().join("config/autostart")).unwrap();
    let repaired = a.apply(failed.revision, requested).unwrap();
    assert_eq!(repaired.registration.enabled, Some(true));
    assert!(repaired.integration_error.is_none());
    fs::write(
        &a.autostart,
        b"[Desktop Entry]\nType=Application\nExec=custom-launcher\nHidden=true\n",
    )
    .unwrap();
    let foreign = fs::read(&a.autostart).unwrap();
    let disabled = a.apply(repaired.revision, Preferences::default()).unwrap();
    assert!(disabled.integration_error.is_some());
    assert_eq!(disabled.registration.enabled, None);
    assert!(!disabled.registration.managed);
    assert_eq!(fs::read(&a.autostart).unwrap(), foreign);
}
#[test]
fn redirected_corrupt_unknown_and_oversize_settings_are_preserved_without_silent_defaults() {
    for bytes in [b"{broken".to_vec(),br#"{"version":2,"revision":0,"preferences":{"closeToTray":true,"startMinimized":false,"autostart":false},"ownedAutostartHashes":[]}"#.to_vec(),vec![b' ';MAX_BYTES as usize+1]] {
        let root=tempfile::tempdir().unwrap();let a=store(root.path(),"app-bin");fs::create_dir_all(&a.root).unwrap();let path=a.root.join("desktop.json");fs::write(&path,&bytes).unwrap();
        assert!(a.read().is_err());assert!(a.apply(0,Preferences::default()).is_err());assert_eq!(fs::read(path).unwrap(),bytes);assert!(!a.autostart.exists());
    }
    let root = tempfile::tempdir().unwrap();
    let a = store(root.path(), "app-bin");
    fs::create_dir_all(&a.root).unwrap();
    let target = root.path().join("foreign");
    fs::write(&target, b"keep").unwrap();
    symlink(&target, a.root.join("desktop.json")).unwrap();
    assert!(a.read().is_err());
    assert!(a.apply(0, Preferences::default()).is_err());
    assert_eq!(fs::read(target).unwrap(), b"keep");
}
#[test]
fn two_independent_stores_serialize_compare_and_swap() {
    let root = tempfile::tempdir().unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let jobs = [false, true].map(|minimized| {
        let s = store(root.path(), "app-bin");
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            s.apply(
                0,
                Preferences {
                    close_to_tray: false,
                    start_minimized: minimized,
                    autostart: true,
                },
            )
        })
    });
    let results = jobs.map(|j| j.join().unwrap());
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(store(root.path(), "app-bin").read().unwrap().revision, 1);
}
#[test]
fn xdg_entry_launches_exact_executable_with_special_characters_without_a_shell() {
    use std::{
        process::Command,
        time::{Duration, Instant},
    };
    let root = tempfile::tempdir().unwrap();
    let executable = "bin space/Quill '\"$`\\%-.py";
    let a = store(root.path(), executable);
    fs::create_dir_all(a.executable.parent().unwrap()).unwrap();
    let captured = root.path().join("argv.json");
    let capture_literal = serde_json::to_string(captured.to_str().unwrap()).unwrap();
    fs::write(&a.executable,format!("#!/usr/bin/python3\nimport sys,json,pathlib\npathlib.Path({capture_literal}).write_text(json.dumps(sys.argv))\n")).unwrap();
    fs::set_permissions(&a.executable, fs::Permissions::from_mode(0o700)).unwrap();
    let enabled = a
        .apply(
            0,
            Preferences {
                autostart: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(enabled.integration_error.is_none());
    let checked = Command::new("desktop-file-validate")
        .arg(&a.autostart)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let runtime = root.path().join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let launched = Command::new("dbus-run-session")
        .args(["--", "gio", "launch"])
        .arg(&a.autostart)
        .env("XDG_CONFIG_HOME", root.path().join("config"))
        .env("XDG_DATA_HOME", root.path().join("data"))
        .env("XDG_RUNTIME_DIR", runtime)
        .env("GIO_USE_VFS", "local")
        .output()
        .unwrap();
    assert!(
        launched.status.success(),
        "{}",
        String::from_utf8_lossy(&launched.stderr)
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !captured.exists() {
        assert!(
            Instant::now() < deadline,
            "GIO did not run fixture executable"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let args: Vec<String> = serde_json::from_slice(&fs::read(captured).unwrap()).unwrap();
    assert_eq!(args, vec![a.executable.to_str().unwrap(), "--autostart"]);
    assert!(entry(Path::new("/tmp/a=b")).is_err());
    assert!(entry(Path::new("/tmp/a\nb")).is_err());
}

#[test]
fn disabling_system_registration_uses_user_hidden_override_and_never_deletes_system_files() {
    let root = tempfile::tempdir().unwrap();
    let system = root.path().join("system");
    fs::create_dir_all(system.join("autostart")).unwrap();
    let path = system.join("autostart").join(ENTRY);
    let bytes = b"[Desktop Entry]\nType=Application\nName=System launch\nExec=/usr/bin/true\n";
    fs::write(&path, bytes).unwrap();
    let s = store(root.path(), "app-bin")
        .with_config_dirs(vec![system])
        .unwrap();
    assert_eq!(s.read().unwrap().registration.enabled, None);
    let disabled = s.apply(0, Preferences::default()).unwrap();
    assert_eq!(disabled.registration.enabled, Some(false));
    assert!(disabled.registration.managed);
    assert!(String::from_utf8(fs::read(&s.autostart).unwrap())
        .unwrap()
        .contains("Hidden=true"));
    let checked = std::process::Command::new("desktop-file-validate")
        .arg(&s.autostart)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let enabled = s
        .apply(
            disabled.revision,
            Preferences {
                autostart: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(enabled.registration.enabled, Some(true));
    let off = s.apply(enabled.revision, Preferences::default()).unwrap();
    assert_eq!(off.registration.enabled, Some(false));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn matching_desired_bytes_are_not_proof_of_ownership_for_an_existing_foreign_entry() {
    let root = tempfile::tempdir().unwrap();
    let s = store(root.path(), "app-bin");
    fs::create_dir_all(s.autostart.parent().unwrap()).unwrap();
    let bytes = entry(&s.executable).unwrap();
    fs::write(&s.autostart, &bytes).unwrap();
    let enabled = s
        .apply(
            0,
            Preferences {
                autostart: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(enabled.integration_error.is_some());
    assert!(!enabled.registration.managed);
    assert_eq!(enabled.registration.enabled, None);
    let off = s.apply(enabled.revision, Preferences::default()).unwrap();
    assert!(off.integration_error.is_some());
    assert_eq!(fs::read(&s.autostart).unwrap(), bytes);
}
