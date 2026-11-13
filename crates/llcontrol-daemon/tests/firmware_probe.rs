//! Private journal files + scripted MockBackend only; no actor tree or host HID.
use llcontrol_daemon::{
    actor::{
        firmware_probe::{Observation, ReconnectProbe},
        firmware_update::{CoolingHold, Outcome, Phase, Progress, Step},
    },
    firmware_operation::Journal,
    persistence::{DeviceSettings, PersistenceStore},
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use llcontrol_protocol::galahad2_lcdv2::{firmware, packets::ACommandPacket};
use std::sync::{Arc, Mutex};
const TOKEN: &str = "N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6";
fn info(serial: Option<&str>, path: &str) -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        serial.map(str::to_owned),
        path.into(),
    )
}
fn setup(serial: Option<&str>, phase: Phase) -> (tempfile::TempDir, Journal, DeviceInfo) {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let journal = store.firmware_operations();
    let target = info(serial, "/mock/before");
    let mut job = journal
        .arm(&target, CoolingHold::PumpOnly, DeviceSettings::default())
        .unwrap();
    if phase != Phase::NotStarted {
        job.effects_possible().unwrap();
    }
    let mut p = Progress {
        phase,
        total_chunks: 619,
        cooling_hold: Some(CoolingHold::PumpOnly),
        ..Progress::default()
    };
    if phase != Phase::NotStarted {
        p.query_written = true;
        p.attempted = Some(Step::PumpHold);
    }
    if phase == Phase::AwaitingReconnect {
        p.pump_hold_written = true;
        p.chunk_writes = 619;
        p.chunk_echoes = 619;
        p.activation_written = true;
        p.attempted = Some(Step::Activate);
    }
    job.finish(&Outcome {
        progress: p,
        diagnostic: None,
    })
    .unwrap();
    (root, journal, target)
}
#[derive(Default)]
struct Behavior {
    token: String,
    date: Option<String>,
    opens: usize,
    closes: usize,
    enumerate_error: bool,
    open_error: bool,
    write_error: bool,
    read_error: bool,
    noisy: bool,
    no_reply: bool,
    remove_on_query: bool,
    journal_edit: Option<std::path::PathBuf>,
}
#[derive(Clone)]
struct Script {
    mock: MockBackend,
    behavior: Arc<Mutex<Behavior>>,
}
impl Script {
    fn new() -> Self {
        Self {
            mock: MockBackend::new(),
            behavior: Arc::new(Mutex::new(Behavior {
                token: TOKEN.into(),
                ..Behavior::default()
            })),
        }
    }
    fn opens(&self) -> usize {
        self.behavior.lock().unwrap().opens
    }
}
struct Handle {
    script: Script,
    info: DeviceInfo,
    inner: Box<dyn OpenHandle>,
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.script.behavior.lock().unwrap().closes += 1;
    }
}
fn error() -> llcontrol_hid::Error {
    llcontrol_hid::Error::BlockingTaskFailed("injected probe fault".into())
}
impl HidBackend for Script {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        if self.behavior.lock().unwrap().enumerate_error {
            return Err(error());
        }
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
        let mut b = self.behavior.lock().unwrap();
        b.opens += 1;
        if b.open_error {
            return Err(error());
        }
        Ok(Box::new(Handle {
            script: self.clone(),
            info: info.clone(),
            inner: self.mock.open(info)?,
        }))
    }
}
impl OpenHandle for Handle {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        assert_eq!(
            bytes,
            firmware::build_request(),
            "probe must only send identity query"
        );
        self.inner.write(bytes)?;
        let b = self.script.behavior.lock().unwrap();
        if let Some(path) = &b.journal_edit {
            std::fs::write(path, b"external edit").unwrap();
        }
        if b.remove_on_query {
            self.script.mock.remove_device(&self.info);
        }
        if b.write_error {
            return Err(error());
        }
        if !b.no_reply {
            self.script.mock.queue_response(
                &self.info,
                ACommandPacket::new_request(134, b.token.as_bytes())
                    .as_bytes()
                    .to_vec(),
            );
            if let Some(date) = &b.date {
                let mut packet = *ACommandPacket::new_request(134, date.as_bytes()).as_bytes();
                packet[4] = 1;
                self.script.mock.queue_response(&self.info, packet.to_vec());
            }
        }
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], ms: i32) -> llcontrol_hid::Result<usize> {
        let b = self.script.behavior.lock().unwrap();
        if b.read_error {
            return Err(error());
        }
        if b.noisy && ms == 0 {
            bytes[0] = 9;
            return Ok(1);
        }
        drop(b);
        self.inner.read_timeout(bytes, ms)
    }
}
#[test]
fn waits_for_observed_detach_then_probes_once_without_replay_or_clearing_quarantine() {
    for date in [None, Some("Oct 30 2025 12:34:56")] {
        let (root, journal, target) = setup(Some("SN-A"), Phase::AwaitingReconnect);
        let mut probe = ReconnectProbe::new(journal.clone()).unwrap();
        let script = Script::new();
        script.mock.add_device(target.clone());
        assert!(matches!(
            probe.observe(&script).unwrap(),
            Observation::WaitingForDetach
        ));
        assert_eq!(script.opens(), 0);
        script.behavior.lock().unwrap().enumerate_error = true;
        assert!(probe.observe(&script).is_err());
        script.behavior.lock().unwrap().enumerate_error = false;
        assert!(matches!(
            probe.observe(&script).unwrap(),
            Observation::WaitingForDetach
        ));
        script.mock.remove_device(&target);
        assert!(matches!(
            probe.observe(&script).unwrap(),
            Observation::WaitingForReconnect
        ));
        let next = info(
            Some("SN-A"),
            if date.is_some() {
                "/mock/before"
            } else {
                "/mock/after"
            },
        );
        script.mock.add_device(next.clone());
        script.behavior.lock().unwrap().date = date.map(str::to_owned);
        script.mock.queue_response(
            &next,
            ACommandPacket::new_request(134, b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.5")
                .as_bytes()
                .to_vec(),
        );
        let before = std::fs::read(root.path().join("firmware-operation.json")).unwrap();
        let Observation::Evidence(evidence) = probe.observe(&script).unwrap() else {
            panic!("missing evidence")
        };
        assert_eq!(
            evidence.operation_id(),
            journal.load().unwrap().unwrap().operation_id
        );
        assert_eq!(evidence.target(), &next);
        assert_eq!(evidence.identity().user_version, "1.6");
        assert_eq!(evidence.identity().raw_datetime, date.unwrap_or_default());
        assert_eq!(script.opens(), 1);
        assert_eq!(script.behavior.lock().unwrap().closes, 1);
        assert_eq!(
            script.mock.write_history(&next),
            vec![firmware::build_request().to_vec()]
        );
        assert_eq!(
            std::fs::read(root.path().join("firmware-operation.json")).unwrap(),
            before
        );
        assert!(journal.ensure_replay_allowed(&next).is_err());
        assert!(probe.observe(&script).is_err());
    }
}
#[test]
fn uncertain_no_effects_or_serialless_operations_cannot_create_verified_probe() {
    for phase in [Phase::Unknown, Phase::NotStarted] {
        let (_root, journal, _) = setup(Some("SN-A"), phase);
        assert!(ReconnectProbe::new(journal).is_err());
    }
    let (_root, journal, _) = setup(None, Phase::AwaitingReconnect);
    assert!(ReconnectProbe::new(journal).is_err());
}
#[test]
fn different_serial_and_duplicate_candidates_never_open_hid() {
    let (_root, journal, _) = setup(Some("SN-A"), Phase::AwaitingReconnect);
    let mut probe = ReconnectProbe::new(journal).unwrap();
    let script = Script::new();
    script.mock.add_device(info(Some("SN-B"), "/mock/other"));
    assert!(matches!(
        probe.observe(&script).unwrap(),
        Observation::WaitingForReconnect
    ));
    script.mock.add_device(info(Some("SN-A"), "/mock/a"));
    script.mock.add_device(info(Some("SN-A"), "/mock/b"));
    assert!(probe.observe(&script).is_err());
    assert_eq!(script.opens(), 0);
}
#[test]
fn transport_identity_and_disappearance_faults_never_unlock_or_restore() {
    for fault in 0..9 {
        let (root, journal, target) = setup(Some("SN-A"), Phase::AwaitingReconnect);
        let mut probe = ReconnectProbe::new(journal.clone()).unwrap();
        let script = Script::new();
        assert!(matches!(
            probe.observe(&script).unwrap(),
            Observation::WaitingForReconnect
        ));
        script.mock.add_device(target.clone());
        {
            let mut b = script.behavior.lock().unwrap();
            match fault {
                0 => b.open_error = true,
                1 => b.write_error = true,
                2 => b.read_error = true,
                3 => b.noisy = true,
                4 => b.no_reply = true,
                5 => b.token = "N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.5".into(),
                6 => b.token = "N9,01,HS,SQ,WRONG,V2.01.041,1.6".into(),
                7 => b.remove_on_query = true,
                _ => b.journal_edit = Some(root.path().join("firmware-operation.json")),
            }
        }
        assert!(probe.observe(&script).is_err(), "fault {fault}");
        assert!(journal.ensure_replay_allowed(&target).is_err());
        assert!(script
            .mock
            .write_history(&target)
            .iter()
            .all(|p| p == &firmware::build_request()));
        if fault == 8 {
            assert_eq!(
                std::fs::read(root.path().join("firmware-operation.json")).unwrap(),
                b"external edit"
            );
        }
        assert_eq!(
            script.behavior.lock().unwrap().closes,
            usize::from(fault != 0)
        );
    }
}
#[test]
fn missing_or_foreign_guard_and_changed_journal_refuse_probe_without_new_writes() {
    for kind in 0..4 {
        let (root, journal, target) = setup(Some("SN-A"), Phase::AwaitingReconnect);
        let mut probe = ReconnectProbe::new(journal.clone()).unwrap();
        let script = Script::new();
        probe.observe(&script).unwrap();
        script.mock.add_device(target.clone());
        let path = root.path().join("devices").join(format!(
            "{}.toml",
            llcontrol_daemon::persistence::settings_key(&target)
        ));
        match kind {
            0 => {
                std::fs::remove_file(path).unwrap();
            }
            1 => {
                let s = std::fs::read_to_string(&path).unwrap();
                let id = journal.load().unwrap().unwrap().operation_id;
                std::fs::write(path, s.replace(&id, "00000000000000000000000000000000")).unwrap();
            }
            2 => {
                std::fs::write(root.path().join("firmware-operation.json"), b"corrupt").unwrap();
            }
            _ => {
                std::fs::remove_file(root.path().join("firmware-operation.json")).unwrap();
            }
        }
        assert!(probe.observe(&script).is_err());
        assert_eq!(script.opens(), 0);
    }
}

#[test]
fn pending_profile_recovery_cannot_make_probe_write_files_or_remove_guard() {
    let (root, journal, target) = setup(Some("SN-A"), Phase::AwaitingReconnect);
    let key = llcontrol_daemon::persistence::settings_key(&target);
    let path = root.path().join("devices").join(format!("{key}.toml"));
    let before = std::fs::read_to_string(&path).unwrap();
    let transaction = serde_json::json!({
        "version":1,"device_key":key,"device_before":before,
        "device_after":toml::to_string_pretty(&DeviceSettings::default()).unwrap(),
        "profiles_before":null,
        "profiles_after":serde_json::to_string_pretty(&llcontrol_ipc::ProfileLibrary::default()).unwrap()
    });
    let transaction_path = root.path().join("screen-reset-transaction.json");
    let bytes = serde_json::to_vec_pretty(&transaction).unwrap();
    std::fs::write(&transaction_path, &bytes).unwrap();
    assert!(ReconnectProbe::new(journal).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    assert_eq!(std::fs::read(&transaction_path).unwrap(), bytes);
    assert!(!root.path().join("profiles.json").exists());
    let store = PersistenceStore::new_with_root(root.path().into());
    assert!(
        store.read(&key).is_err(),
        "ordinary recovery must also refuse guard removal"
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), before);
    assert_eq!(std::fs::read(transaction_path).unwrap(), bytes);
}
