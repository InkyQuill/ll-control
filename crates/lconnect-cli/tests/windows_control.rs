#[path = "../../lconnect-profile/tests/support/control.rs"]
mod fixture;
#[path = "../../lconnect-profile/tests/support/wpf_color.rs"]
mod wpf_fixture;
use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{client::IpcClient, server::IpcServer, DeviceId};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};
struct Harness {
    manager: ManagerHandle,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
    socket: PathBuf,
    dev: DeviceId,
}
impl Harness {
    async fn start(root: &Path, store: Arc<PersistenceStore>, mock: MockBackend) -> Self {
        let cfg = Config::default();
        let stats = Arc::new(Stats::new());
        let manager = DeviceManager::new(Arc::new(mock), store.clone(), cfg.clone(), stats.clone())
            .spawn()
            .await;
        let dev = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(d) = manager.list().await.first() {
                    break d.id.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let socket = root.join("lconnectd.sock");
        let path = socket.clone();
        let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store);
        let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&path).await });
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                assert!(
                    !server.is_finished(),
                    "private IPC server exited during startup"
                );
                if IpcClient::connect(&socket).await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        Self {
            manager,
            server,
            socket,
            dev,
        }
    }
    async fn client(&self) -> IpcClient {
        IpcClient::connect(&self.socket).await.unwrap()
    }
    async fn stop(self) {
        self.server.abort();
        let _ = self.server.await;
        self.manager.shutdown().await;
    }
}
fn cli(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .env("LCONNECTCTL_SOCKET", root.join("lconnectd.sock"))
        .env("XDG_CONFIG_HOME", root.join("unused-client-config"))
        .args(args)
        .output()
        .unwrap()
}
fn wpf_archive() -> Vec<u8> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::io::{Cursor, Read, Write};
    use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
    let mut archive = ZipArchive::new(Cursor::new(fixture::archive())).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name("backup")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut backup: Value = serde_json::from_slice(&bytes).unwrap();
    let mut profile: Value = serde_json::from_slice(
        &STANDARD
            .decode(backup["Datas"][1]["Data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    wpf_fixture::storage_colors(&mut profile);
    backup["Datas"][1]["Data"] = json!(STANDARD.encode(serde_json::to_vec(&profile).unwrap()));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("backup", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&backup).unwrap())
        .unwrap();
    zip.finish().unwrap().into_inner()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn actual_cli_preview_import_retry_preserves_extensions_screen_globals_and_restart_readback()
{
    use lconnect_ipc::{CoolingPolicy, NamedProfile, ProfileContent, ProfileEdit, ProfileGroup};
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(
        root.path().join("settings"),
    ));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("windows".into()),
        PathBuf::from("/mock/windows"),
    );
    let key = settings_key(&info);
    let mut settings = DeviceSettings::default();
    settings.lcd.brightness = 73;
    settings.lcd.rotation = "90".into();
    settings.thermal.cpu = Some(lconnect_ipc::ThermalRule {
        sensor_id: "fixture:CPUTemperature".into(),
        threshold_c: 92.0,
        color: lconnect_ipc::Rgb { r: 255, g: 0, b: 0 },
    });
    store.write(&key, &settings).unwrap();
    let extension = NamedProfile {
        slot: 2,
        name: "Keep Linux slot".into(),
        content: ProfileContent::Fan(CoolingPolicy::Fixed { percent: 42 }),
    };
    store
        .profiles()
        .edit(
            0,
            ProfileEdit::Save {
                profile: extension.clone(),
            },
        )
        .unwrap();
    let mock = MockBackend::new();
    mock.add_device(info);
    let h = Harness::start(root.path(), store.clone(), mock.clone()).await;
    let file = root.path().join("windows.zip");
    std::fs::write(&file, wpf_archive()).unwrap();
    let bindings = root.path().join("bindings.json");
    std::fs::write(&bindings, serde_json::to_vec(&fixture::bindings()).unwrap()).unwrap();
    let common = [
        "--json",
        "profiles",
        "windows-control",
        file.to_str().unwrap(),
        "--record",
        "1",
        "--bindings",
        bindings.to_str().unwrap(),
    ];
    let before = store.profiles().list().unwrap();
    let preview = cli(root.path(), &common);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(
        preview["proposed"]["cooling_modes"]["fan"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        preview["lighting"]["fan_modes"].as_array().unwrap().len(),
        6
    );
    assert_eq!(
        preview["lighting"]["pump_modes"].as_array().unwrap().len(),
        16
    );
    assert_eq!(
        preview["lighting"]["dynamic_rgb_profiles"]["modes"]
            .as_array()
            .unwrap()
            .len(),
        16
    );
    assert!(!serde_json::to_string(&preview)
        .unwrap()
        .contains("lighting_source_json"));
    assert_eq!(store.profiles().list().unwrap(), before);
    assert!(store.read(&key).unwrap().cooling_banks.is_none());
    let token = preview["target"]["expected"].as_str().unwrap().to_owned();
    let binding_map = lconnect_profile::linux_lighting::parse_bindings(
        &serde_json::to_vec(&fixture::bindings()).unwrap(),
    )
    .unwrap();
    let draft = lconnect_profile::control::prepare(
        std::io::Cursor::new(fixture::archive()),
        1,
        &binding_map,
        0.0,
        100,
    )
    .unwrap();
    let pristine = serde_json::to_value(&draft).unwrap();
    let client = h.client().await;
    let mut invalid = Vec::new();
    let mut v = pristine.clone();
    v["version"] = json!(2);
    invalid.push(v);
    let mut v = pristine.clone();
    v["lighting"]["pump_modes"]
        .as_object_mut()
        .unwrap()
        .remove("bounce");
    invalid.push(v);
    let mut v = pristine.clone();
    v["cooling"]["fan"] = json!({"mode":"fixed","percent":37});
    invalid.push(v);
    let mut v = pristine.clone();
    v["lighting"]["dynamic_rgb"]["sensor_id"] = json!("different:selected");
    invalid.push(v);
    let mut v = pristine.clone();
    v["lighting_source_json"] =
        json!(serde_json::to_string(&json!({"padding":"x".repeat(128*1024)})).unwrap());
    invalid.push(v);
    for value in invalid {
        let params = json!({"dev":h.dev,"draft_json":serde_json::to_string(&value).unwrap()});
        assert!(client
            .call("preview_windows_control_import", params.clone())
            .await
            .is_err());
        let mut params = params;
        params["operation_id"] = json!("11112222333344445555666677778888");
        params["expected"] = json!(token);
        assert!(client.call("import_windows_control", params).await.is_err());
    }
    assert!(store.read(&key).unwrap().group_imports.is_empty());
    assert_eq!(store.profiles().list().unwrap(), before);
    let id = "aaaabbbbccccddddeeeeffff00001111";
    let mut args = common.to_vec();
    args.extend(["--apply", "--id", id, "--expected", &token]);
    // Forward the real commit, then discard only its successful ACK.
    let loss_path = root.path().join("loss.sock");
    let listener = std::os::unix::net::UnixListener::bind(&loss_path).unwrap();
    let actual = h.socket.clone();
    let proxy = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        let (mut front, _) = listener.accept().unwrap();
        front
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut back = std::os::unix::net::UnixStream::connect(actual).unwrap();
        back.set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut upstream = BufReader::new(back.try_clone().unwrap());
        let mut downstream = BufReader::new(front.try_clone().unwrap());
        let mut hello = String::new();
        upstream.read_line(&mut hello).unwrap();
        front.write_all(hello.as_bytes()).unwrap();
        loop {
            let mut line = String::new();
            assert!(downstream.read_line(&mut line).unwrap() > 0);
            let request: Value = serde_json::from_str(&line).unwrap();
            back.write_all(line.as_bytes()).unwrap();
            loop {
                let mut reply = String::new();
                assert!(upstream.read_line(&mut reply).unwrap() > 0);
                let value: Value = serde_json::from_str(&reply).unwrap();
                if value.get("id") == request.get("id") {
                    if request["method"] == "import_windows_control" {
                        assert!(value.get("error").is_none(), "{value}");
                        return value["result"].clone();
                    }
                    front.write_all(reply.as_bytes()).unwrap();
                    break;
                }
                front.write_all(reply.as_bytes()).unwrap();
            }
        }
    });
    let output = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .env("LCONNECTCTL_SOCKET", &loss_path)
        .env("XDG_CONFIG_HOME", root.path().join("unused-client-config"))
        .args(&args)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains(id));
    assert!(String::from_utf8_lossy(&output.stderr).contains(&token));
    let result = proxy.join().unwrap();
    assert_eq!(result["repeated"], false);
    assert_eq!(result["readback"]["selected_content_current"], true);
    let recovered = cli(root.path(), &args);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    let recovered: Value = serde_json::from_slice(&recovered.stdout).unwrap();
    assert_eq!(recovered["repeated"], true);
    let c = h.client().await;
    let banks = c
        .call("get_cooling_modes", json!({"dev":h.dev}))
        .await
        .unwrap();
    assert_eq!(banks["fan"].as_array().unwrap().len(), 8);
    assert_eq!(banks["pump"].as_array().unwrap().len(), 3);
    let running = c
        .call("get_cooling_state", json!({"dev":h.dev}))
        .await
        .unwrap();
    assert_eq!(running["fan"]["mode_id"], "UserDefine2");
    assert_eq!(running["pump"]["mode_id"], "PWMQuiet");
    let saved = store.read(&key).unwrap();
    assert_eq!(saved.lcd.brightness, 73);
    assert_eq!(saved.lcd.rotation, "90");
    assert_eq!(saved.thermal, settings.thermal);
    assert!(saved
        .lighting
        .windows_source
        .as_deref()
        .unwrap()
        .contains("DynamicSettings"));
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("windows_source"));
    let source_history: Value =
        serde_json::from_str(saved.lighting.windows_source.as_deref().unwrap()).unwrap();
    assert!(!source_history["source_color_objects"]
        .as_object()
        .unwrap()
        .is_empty());
    assert_eq!(saved.lighting.fan_modes.len(), 6);
    assert_eq!(saved.lighting.pump_modes.len(), 16);
    assert_eq!(saved.dynamic_rgb_profiles.modes.len(), 16);
    let captured = lconnect_daemon::profile_group::GroupProfile::capture(
        &saved,
        &store.profiles().list().unwrap(),
        lconnect_daemon::profile_group::GroupKind::FanAndLighting,
    )
    .unwrap();
    let archives = lconnect_daemon::profile_archive::ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    );
    let mut portable = Vec::new();
    archives
        .export_group(&captured.profile, &mut portable)
        .unwrap();
    let decoded = archives
        .prepare_group(&mut std::io::Cursor::new(portable))
        .unwrap();
    assert_eq!(
        serde_json::to_value(decoded.profile()).unwrap(),
        serde_json::to_value(&captured.profile).unwrap(),
        "normalized controls and historical provenance survive the existing portable codec"
    );
    assert_eq!(saved.cooling.fan_mode.as_deref(), Some("UserDefine2"));
    assert_eq!(saved.cooling.pump_mode.as_deref(), Some("PWMQuiet"));
    assert!(store
        .profiles()
        .list()
        .unwrap()
        .profiles
        .contains(&extension));
    // A newer extension-slot edit and manual policy must survive an original-ID retry.
    let library = store.profiles().list().unwrap();
    c.call("edit_profile",json!({"expected_revision":library.revision,"edit":{"action":"rename","group":"fan","slot":2,"name":"Newer Linux slot"}})).await.unwrap();
    c.call(
        "set_cooling_policy",
        json!({"dev":h.dev,"target":"fan","policy":{"mode":"fixed","percent":37}}),
    )
    .await
    .unwrap();
    let later = store.profiles().list().unwrap();
    let retry = cli(root.path(), &args);
    assert!(
        retry.status.success(),
        "{}",
        String::from_utf8_lossy(&retry.stderr)
    );
    let retry: Value = serde_json::from_slice(&retry.stdout).unwrap();
    assert_eq!(retry["repeated"], true);
    assert_eq!(retry["readback"]["selected_content_current"], false);
    assert_eq!(store.profiles().list().unwrap(), later);
    assert_eq!(
        c.call("get_cooling_state", json!({"dev":h.dev}))
            .await
            .unwrap()["fan"]["requested"],
        json!({"mode":"fixed","percent":37})
    );
    let mut stale = common.to_vec();
    stale.extend([
        "--apply",
        "--id",
        "bbbbccccddddeeeeffff000011112222",
        "--expected",
        &token,
    ]);
    let stale = cli(root.path(), &stale);
    assert!(!stale.status.success());
    assert!(stale.stdout.is_empty());
    h.stop().await;
    let h = Harness::start(root.path(), store.clone(), mock).await;
    let c = h.client().await;
    let recovered = c
        .call(
            "read_group_import",
            json!({"dev":h.dev,"group":"fan_and_lighting","operation_id":id}),
        )
        .await
        .unwrap();
    assert_eq!(recovered["selected_content_current"], false);
    assert_eq!(store.profiles().list().unwrap(), later);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let state = c
                .call("get_cooling_state", json!({"dev":h.dev}))
                .await
                .unwrap();
            if state["pump"]["mode_id"] == "PWMQuiet"
                && state["fan"]["requested"] == json!({"mode":"fixed","percent":37})
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(store.read(&key).unwrap().lighting.windows_source.is_some());
    assert_eq!(
        later
            .profiles
            .iter()
            .find(|p| p.slot == 2 && p.content.group() == ProfileGroup::Fan)
            .unwrap()
            .name,
        "Newer Linux slot"
    );
    h.stop().await;
}
