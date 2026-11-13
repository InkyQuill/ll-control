//! Journal failures must reach the CLI exit status without hiding child output.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};

fn run_logs(script: Option<&str>, args: &[&str]) -> Output {
    let bin_dir = tempfile::tempdir().unwrap();
    if let Some(script) = script {
        let path = bin_dir.path().join("journalctl");
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    Command::new(env!("CARGO_BIN_EXE_llcontrol"))
        .arg("logs")
        .args(args)
        .env("PATH", bin_dir.path())
        .output()
        .unwrap()
}

#[test]
fn logs_reports_missing_journalctl() {
    let output = run_logs(None, &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to run journalctl"));
}

#[test]
fn logs_reports_journalctl_failure_and_preserves_stderr() {
    let output = run_logs(Some("echo 'journal unavailable' >&2\nexit 23"), &[]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("journal unavailable"), "{stderr}");
    assert!(
        stderr.contains("journalctl failed with exit status: 23"),
        "{stderr}"
    );
}

#[test]
fn logs_forwards_invalid_priority_and_reports_rejection() {
    let output = run_logs(
        Some("[ \"$5\" = '-p' ] && [ \"$6\" = 'not-a-real-priority' ] || exit 99\necho 'Unknown log level not-a-real-priority' >&2\nexit 1"),
        &["--level", "not-a-real-priority"],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown log level not-a-real-priority"),
        "{stderr}"
    );
    assert!(
        stderr.contains("journalctl failed with exit status: 1"),
        "{stderr}"
    );
}

#[test]
fn logs_preserves_output_and_follow_argument_on_success() {
    let output = run_logs(
        Some("[ \"$*\" = '--user -u llcontrold --output=cat --follow -p warning' ] || exit 99\necho 'log entry'"),
        &["--follow", "--level", "warning"],
    );
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"log entry\n");
}
