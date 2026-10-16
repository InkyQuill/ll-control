use lconnect_daemon::{
    media_assets::MediaAssetStore, profile_archive::ProfileArchives, theme_assets::ThemeAssetStore,
    theme_fonts::ThemeFontStore,
};
use lconnect_ipc::{NamedProfile, ProfileGroup};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};
#[test]
fn ctrl_c_during_upload_cancels_and_releases_the_exact_session() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fan.lcp");
    let archive = ProfileArchives::new(
        ThemeAssetStore::new(root.path().join("images")),
        ThemeFontStore::new(root.path().join("fonts")),
        MediaAssetStore::new(root.path().join("media")),
    );
    archive
        .export_file(
            &NamedProfile::defaults(ProfileGroup::Fan, 0, Some("cpu")).unwrap(),
            &path,
        )
        .unwrap();
    let listener = UnixListener::bind(root.path().join("lconnectd.sock")).unwrap();
    let (pid_tx, pid_rx) = mpsc::channel::<u32>();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        writeln!(stream,"{}",json!({"jsonrpc":"2.0","method":"hello","params":{"daemon_version":"0.1.0","ipc_version":"1.0"}})).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut read = || {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            serde_json::from_str::<Value>(&line).unwrap()
        };
        let list = read();
        assert_eq!(list["method"], "list_profiles");
        writeln!(stream,"{}",json!({"jsonrpc":"2.0","id":list["id"],"result":{"version":1,"revision":0,"profiles":[]}})).unwrap();
        let begin = read();
        assert_eq!(begin["params"]["request"]["action"], "begin_import");
        let id = begin["params"]["request"]["id"].clone();
        let mut state = json!({"id":id,"direction":"import","group":"fan","slot":0,"expected_revision":0,"phase":"uploading","size":begin["params"]["request"]["size"],"received":0,"profile":null,"library":null,"last_error":null,"committed_revision":null});
        writeln!(stream,"{}",json!({"jsonrpc":"2.0","id":begin["id"],"result":{"state":state,"bytes":[],"offset":null,"eof":false}})).unwrap();
        let chunk = read();
        assert_eq!(chunk["params"]["request"]["action"], "chunk");
        assert_eq!(chunk["params"]["request"]["id"], id);
        // Hold the chunk acknowledgement so SIGINT interrupts a real pending RPC.
        let pid = pid_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status()
            .unwrap()
            .success());
        for action in ["cancel", "release"] {
            let request = read();
            assert_eq!(request["params"]["request"]["action"], action);
            assert_eq!(request["params"]["request"]["id"], id);
            state["phase"] = json!("cancelled");
            writeln!(stream,"{}",json!({"jsonrpc":"2.0","id":request["id"],"result":{"state":state,"bytes":[],"offset":null,"eof":false}})).unwrap();
        }
    });
    let child = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .args(["profiles", "import", "fan", "0", path.to_str().unwrap()])
        .env("XDG_RUNTIME_DIR", root.path())
        .env_remove("LCONNECTCTL_SOCKET")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    pid_tx.send(child.id()).unwrap();
    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("archive command interrupted"));
    assert!(path.exists(), "source archive is preserved");
}
