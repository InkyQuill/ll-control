use super::*;
fn write(root: &Path, path: &str, bytes: impl AsRef<[u8]>) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}
fn fixture(root: &Path) -> LinuxSpec {
    LinuxSpec {
        proc_root: root.join("proc"),
        sys_root: root.join("sys"),
        pci_ids: vec![root.join("pci.ids")],
    }
}
fn populated(root: &Path) {
    write(root,"proc/cpuinfo","processor : 0\nphysical id : 0\nmodel name : Fixture CPU\nvendor_id : GenuineIntel\ncpu cores : 1\n\nprocessor : 1\nphysical id : 0\nmodel name : Fixture CPU\nvendor_id : GenuineIntel\ncpu cores : 1\n\n");
    write(root, "proc/meminfo", "MemTotal: 32768000 kB\n");
    write(root, "sys/bus/pci/devices/0000:01:00.0/class", "0x030000\n");
    write(root, "sys/bus/pci/devices/0000:01:00.0/vendor", "0x1002");
    write(root, "sys/bus/pci/devices/0000:01:00.0/device", "0x67df");
    write(root, "pci.ids", "1002  AMD\n\t67df  Fixture GPU\n");
    write(root, "sys/class/dmi/id/board_name", "Fixture Mainboard");
    write(root, "sys/class/dmi/id/bios_version", "2.1");
    write(root, "sys/block/sda/device/model", "Fixture SSD");
    write(root, "sys/block/sda/size", "1953525168");
    write(root, "sys/block/sda/queue/rotational", "0");
    write(root, "sys/class/drm/card0-HDMI-A-1/status", "connected");
}
#[test]
fn six_groups_report_real_static_fields_without_devices_or_fabricating_missing_values() {
    let root = tempfile::tempdir().unwrap();
    populated(root.path());
    let s = fixture(root.path()).collect().unwrap();
    assert_eq!(s.groups.len(), 6);
    assert_eq!(s.groups[0].items[0].title, "Fixture CPU");
    assert_eq!(
        s.groups[0].items[0]
            .fields
            .iter()
            .find(|f| f.key == "logical_threads")
            .unwrap()
            .value,
        Some(Value::Count(2))
    );
    assert_eq!(s.groups[1].items[0].title, "Fixture GPU");
    let cap = s.groups[5].items[0]
        .fields
        .iter()
        .find(|f| f.key == "capacity")
        .unwrap();
    assert_eq!(cap.value, Some(Value::Bytes(1953525168 * 512)));
    assert!(s.groups[3].items[0]
        .fields
        .iter()
        .find(|f| f.key == "model")
        .unwrap()
        .value
        .is_none());
    assert_eq!(s.groups[4].items[0].title, "Fixture Mainboard");
    assert!(s.groups[2].items[0]
        .fields
        .iter()
        .find(|f| f.key == "reported_modules")
        .unwrap()
        .value
        .is_none());
}
#[test]
fn absent_invalid_and_overflow_inputs_stay_unavailable_and_do_not_cross_groups() {
    let root = tempfile::tempdir().unwrap();
    let p = fixture(root.path());
    let s = p.collect().unwrap();
    assert!(s.groups[0].items.is_empty());
    assert!(s.groups[2].items[0]
        .fields
        .iter()
        .all(|f| f.value.is_none()));
    populated(root.path());
    write(root.path(), "sys/block/sda/size", u64::MAX.to_string());
    write(
        root.path(),
        "sys/class/dmi/id/board_name",
        "To be filled by O.E.M.",
    );
    let s = p.collect().unwrap();
    assert!(s.groups[5].items[0]
        .fields
        .iter()
        .find(|f| f.key == "capacity")
        .unwrap()
        .value
        .is_none());
    assert_ne!(s.groups[4].items[0].title, "To be filled by O.E.M.");
    write(root.path(), "proc/cpuinfo", vec![b'x'; 1024 * 1024 + 1]);
    assert!(p.collect().unwrap().groups[0].items.is_empty());
}
#[test]
fn dmi_memory_size_strings_and_edid_checksum_are_validated_before_details() {
    let mut raw = vec![0; 34];
    raw[0] = 17;
    raw[1] = 34;
    raw[12..14].copy_from_slice(&0x7fffu16.to_le_bytes());
    raw[28..32].copy_from_slice(&32768u32.to_le_bytes());
    raw[16] = 1;
    raw[23] = 2;
    raw[26] = 3;
    raw[18] = 0x22;
    raw[32..34].copy_from_slice(&6000u16.to_le_bytes());
    raw.extend_from_slice(b"DIMM A2\0Fixture RAM\0Module X\0\0");
    let item = binary::dimm(&raw, "17-0").unwrap();
    assert_eq!(item.title, "DIMM A2");
    assert_eq!(
        item.fields
            .iter()
            .find(|f| f.key == "capacity")
            .unwrap()
            .value,
        Some(Value::Bytes(32 * 1024 * 1024 * 1024))
    );
    assert_eq!(
        item.fields
            .iter()
            .find(|f| f.key == "configured_transfer_rate")
            .unwrap()
            .value,
        Some(Value::Text("6000 MT/s".into()))
    );
    assert!(binary::dimm(&raw[..35], "17-0").is_none());
    raw[12..14].copy_from_slice(&0xffffu16.to_le_bytes());
    assert!(binary::dimm(&raw, "17-0")
        .unwrap()
        .fields
        .iter()
        .find(|f| f.key == "capacity")
        .unwrap()
        .value
        .is_none());
    let mut edid = vec![0u8; 128];
    edid[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    edid[8..10].copy_from_slice(&0x10acu16.to_be_bytes());
    edid[18] = 1;
    edid[19] = 4;
    edid[57] = 0xfc;
    edid[59..72].copy_from_slice(b"Fixture LCD\n ");
    edid[127] = 0u8.wrapping_sub(edid[..127].iter().fold(0u8, |s, v| s.wrapping_add(*v)));
    assert_eq!(
        binary::edid(&edid).unwrap().0.as_deref(),
        Some("Fixture LCD")
    );
    edid[20] ^= 1;
    assert!(binary::edid(&edid).is_none());
}
#[tokio::test]
async fn refresh_is_explicit_and_cached_inventory_is_not_recollected_by_readback() {
    let root = tempfile::tempdir().unwrap();
    populated(root.path());
    let service = SpecService::new(fixture(root.path()));
    let before = service.get(false).await.unwrap();
    write(root.path(), "sys/class/dmi/id/board_name", "Changed board");
    assert_eq!(service.get(false).await.unwrap(), before);
    assert_eq!(
        service.get(true).await.unwrap().groups[4].items[0].title,
        "Changed board"
    );
}

#[test]
fn pci_comments_and_non_ascii_noise_do_not_hide_an_accessible_gpu() {
    assert_eq!(
        pci_name(
            "💻 malformed\n1002  Vendor\n# comment\n\t67df  Fixture GPU\n",
            "1002",
            "67df"
        )
        .as_deref(),
        Some("Fixture GPU")
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_caller_keeps_worker_permit_until_collection_and_cache_publication_finish() {
    let root = tempfile::tempdir().unwrap();
    populated(root.path());
    let service = SpecService::new(fixture(root.path()));
    let cache = service.cache.clone();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let _guard = cache.lock().unwrap();
        ready_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    ready_rx.await.unwrap();
    let copy = service.clone();
    let caller = tokio::spawn(async move { copy.get(true).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while service.jobs.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert!(matches!(
        service.get(true).await,
        Err(lconnect_ipc::IpcError::DeviceBusy(_))
    ));
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while service.jobs.available_permits() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(service.cache.lock().unwrap().is_some());
    assert_eq!(
        service.get(false).await.unwrap().groups[0].items[0].title,
        "Fixture CPU"
    );
}

#[test]
fn unreadable_link_or_non_regular_cpu_source_does_not_suppress_other_groups() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    populated(root.path());
    let cpu = root.path().join("proc/cpuinfo");
    let saved = root.path().join("saved-cpu");
    fs::rename(&cpu, &saved).unwrap();
    symlink(&saved, &cpu).unwrap();
    let spec = fixture(root.path()).collect().unwrap();
    assert!(spec.groups[0].items.is_empty());
    assert_eq!(spec.groups[4].items[0].title, "Fixture Mainboard");
    fs::remove_file(&cpu).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &cpu,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    assert!(fixture(root.path()).collect().unwrap().groups[0]
        .items
        .is_empty());
    fs::remove_file(&cpu).unwrap();
    fs::create_dir(&cpu).unwrap();
    assert!(fixture(root.path()).collect().unwrap().groups[0]
        .items
        .is_empty());
}
