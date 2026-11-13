use llcontrol_daemon::{
    persistence::{DeviceSettings, PersistenceStore},
    profile_archive::ProfileArchives,
    profile_group::{GroupKind, GroupProfile, GroupSettings},
};
use llcontrol_ipc::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};

const FONT: &[u8] = include_bytes!("../../llcontrol-image/assets/DejaVuSans.ttf");
fn archives(store: &PersistenceStore) -> ProfileArchives {
    ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    )
}
fn clip() -> H264Clip {
    H264Clip {
        fps: 24,
        units: vec![
            H264AccessUnit {
                bytes: vec![0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
                keyframe: true,
            },
            H264AccessUnit {
                bytes: vec![0, 0, 1, 0x41, 1],
                keyframe: false,
            },
        ],
    }
}
fn theme(image: &str, font: &str, video: &str) -> LcdTheme {
    let style =
        json!({"x":30,"y":50,"size":24,"color":[255,255,255],"align":"left","font_asset":font});
    let mut theme:LcdTheme=serde_json::from_value(json!({"version":1,"name":"Current screen","background":[0,0,0],"background_image":{"asset":image,"fit":"crop"},"layers":[{"kind":"text","text":image,"style":style},{"kind":"sensor","sensor_id":image,"label":"Exact identity","unit":"percent","temperature_unit":"celsius","decimals":0,"style":style}]})).unwrap();
    let rain=serde_json::from_value(json!({"version":3,"name":"Inactive Rain","background":[0,0,0],"background_video":video,"layers":[{"kind":"weather","effect":{"kind":"rain","intensity":1,"dynamic":false,"sensor":null,"seed":42}}]})).unwrap();
    theme.page_cache.push(SavedLcdPage {
        page: LcdPageKind::Rain,
        theme: Box::new(rain),
    });
    theme
}
fn manifest(bytes: &[u8]) -> Value {
    let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    serde_json::from_slice(&bytes[44..44 + len]).unwrap()
}
fn mutate(bytes: &[u8], f: impl FnOnce(&mut Value)) -> Vec<u8> {
    let old = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut header = manifest(bytes);
    f(&mut header);
    let encoded = serde_json::to_vec(&header).unwrap();
    let mut out = b"LCGROUP1".to_vec();
    out.extend((encoded.len() as u32).to_le_bytes());
    out.extend(Sha256::digest(&encoded));
    out.extend(encoded);
    out.extend(&bytes[44 + old..]);
    out
}
fn sample(store: &PersistenceStore, legacy: bool) -> GroupProfile {
    let image = store
        .theme_assets()
        .import_named(
            llcontrol_image::defaults::IMAGES[0].png,
            Some("Current image"),
        )
        .unwrap();
    let gallery = store
        .theme_assets()
        .import_named(
            llcontrol_image::defaults::IMAGES[1].png,
            Some("Gallery only"),
        )
        .unwrap();
    store
        .theme_assets()
        .import_named(
            llcontrol_image::defaults::IMAGES[0].png,
            Some("Unlisted upload"),
        )
        .unwrap();
    let font = store.theme_fonts().import(FONT).unwrap();
    let video = store
        .media_assets()
        .import("Cached background".into(), clip())
        .unwrap()
        .key;
    let gallery_video = store
        .media_assets()
        .import("Gallery video".into(), clip())
        .unwrap()
        .key;
    let active = theme(&image, &font, &video);
    let mut settings = DeviceSettings::default();
    settings.lcd.brightness = 73;
    settings.lcd.rotation = "270".into();
    settings.lcd.control_mode = "Application".into();
    settings.theme = Some(active.clone());
    if !legacy {
        let mut collections = ScreenCollections::defaults(18);
        collections.images.push(gallery.clone());
        collections.watermarks.push(gallery);
        collections
            .backgrounds
            .push(ScreenBackgroundAsset::Video(gallery_video.clone()));
        settings.screen_collections = Some(collections);
    }
    let library = ProfileLibrary {
        profiles: vec![
            NamedProfile {
                slot: 0,
                name: "First".into(),
                content: ProfileContent::Screen(active),
            },
            NamedProfile {
                slot: 7,
                name: "Video eighth".into(),
                content: ProfileContent::ScreenMedia(gallery_video),
            },
        ],
        ..Default::default()
    };
    GroupProfile::capture(&settings, &library, GroupKind::Screen)
        .unwrap()
        .profile
}

#[test]
fn full_screen_clean_store_roundtrip_carries_active_cached_slot_gallery_and_unlisted_uploads() {
    let source_dir = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(source_dir.path().into());
    let before = sample(&source, false);
    let mut bytes = Vec::new();
    archives(&source).export_group(&before, &mut bytes).unwrap();
    assert_eq!(&bytes[..8], b"LCGROUP1");
    let header = manifest(&bytes);
    assert_eq!(
        header["assets"].as_array().unwrap().len(),
        6,
        "three user images, two videos and one shared font"
    );
    assert_eq!(header["uploads"]["images"].as_array().unwrap().len(), 3);
    let dest_dir = tempfile::tempdir().unwrap();
    let dest = PersistenceStore::new_with_root(dest_dir.path().into());
    let mut latest = DeviceSettings::default();
    latest.pump.pwm_percent = 81;
    dest.profiles()
        .edit(
            0,
            ProfileEdit::Reset {
                group: ProfileGroup::Fan,
                slot: 3,
                sensor_id: Some("target:cpu".into()),
            },
        )
        .unwrap();
    let target = dest
        .snapshot_profile_group("target", &latest, GroupKind::Screen)
        .unwrap();
    let importer = archives(&dest);
    let prepared = importer.prepare_group(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(
        dest.profiles().list().unwrap().revision,
        1,
        "preparation does not change profiles"
    );
    assert!(dest.read("target").unwrap().theme.is_none());
    assert_eq!(prepared.uploaded_images().len(), 3);
    assert_eq!(prepared.uploaded_media().len(), 2);
    let (saved, library) = dest
        .commit_profile_group("target", &latest, &target.revision, prepared.profile())
        .unwrap();
    assert_eq!(saved.pump.pwm_percent, 81);
    assert_eq!(saved.lcd.brightness, 73);
    assert_eq!(saved.lcd.rotation, "270");
    assert_eq!(library.profiles.len(), 3);
    let GroupSettings::Screen(original) = &before.content else {
        unreachable!()
    };
    let old = original.theme.as_ref().unwrap();
    let new = saved.theme.as_ref().unwrap();
    assert!(old.image_assets().is_disjoint(&new.image_assets()));
    assert!(old.video_assets().is_disjoint(&new.video_assets()));
    let ThemeLayer::Sensor { sensor_id, .. } = &new.layers[1] else {
        unreachable!()
    };
    assert_eq!(
        sensor_id,
        old.background_image.as_ref().unwrap().asset.as_str()
    );
    let ThemeLayer::Text { text, .. } = &new.layers[0] else {
        unreachable!()
    };
    assert_eq!(text, sensor_id);
    assert_eq!(
        saved.screen_collections.as_ref().unwrap().images.len(),
        3,
        "protected defaults do not grow duplicates"
    );
    let gallery = saved.screen_collections.as_ref().unwrap();
    gallery.validate().unwrap();
    let names: Vec<_> = dest
        .theme_assets()
        .list_named()
        .unwrap()
        .into_iter()
        .filter_map(|x| x.name)
        .collect();
    assert!(
        names.contains(&"Gallery only".to_owned()) && names.contains(&"Unlisted upload".to_owned())
    );
    drop(prepared);
    let restarted = PersistenceStore::new_with_root(dest_dir.path().into());
    let settings = restarted.read("target").unwrap();
    let restored = settings.theme.as_ref().unwrap();
    restarted.theme_assets().load(restored).unwrap();
    restarted.theme_fonts().load_theme(restored).unwrap();
    for key in restored.font_assets() {
        assert_eq!(restarted.theme_fonts().load(key).unwrap(), FONT);
    }
    for key in restored.video_assets() {
        let mut source = restarted.media_assets().acquire_source(key).unwrap();
        assert_eq!(source.read_unit(1).unwrap().bytes, clip().units[1].bytes);
    }
    assert_eq!(restarted.profiles().list().unwrap(), library);
    let fresh = restarted
        .snapshot_profile_group("target", &settings, GroupKind::Screen)
        .unwrap();
    let mut again = Vec::new();
    archives(&restarted)
        .export_group(&fresh.profile, &mut again)
        .unwrap();
    assert_eq!(manifest(&again)["assets"].as_array().unwrap().len(), 6);
}

#[test]
fn legacy_catalog_is_materialized_and_gallery_limit_is_not_spent_on_default_copies() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let profile = sample(&store, true);
    let mut bytes = Vec::new();
    archives(&store).export_group(&profile, &mut bytes).unwrap();
    let dest_dir = tempfile::tempdir().unwrap();
    let dest = PersistenceStore::new_with_root(dest_dir.path().into());
    let prepared = archives(&dest)
        .prepare_group(&mut Cursor::new(&bytes))
        .unwrap();
    let GroupSettings::Screen(group) = &prepared.profile().content else {
        unreachable!()
    };
    let collections = group.collections.as_ref().unwrap();
    assert_eq!(collections.images.len(), 5);
    assert_eq!(collections.watermarks.len(), 4);
    assert_eq!(collections.backgrounds.len(), 8);
    collections.validate().unwrap();
    assert!(prepared
        .uploaded_images()
        .iter()
        .all(|key| collections.images.contains(key)));
}

#[test]
fn default_only_full_collections_keep_256_entry_limit_after_remap() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let mut collections = ScreenCollections::defaults(1);
    for _ in 0..254 {
        let key = store
            .theme_assets()
            .import(llcontrol_image::defaults::IMAGES[0].png)
            .unwrap();
        collections.images.push(key);
    }
    let settings = DeviceSettings {
        screen_collections: Some(collections),
        ..Default::default()
    };
    let profile = GroupProfile::capture(&settings, &Default::default(), GroupKind::Screen)
        .unwrap()
        .profile;
    let mut bytes = Vec::new();
    archives(&store).export_group(&profile, &mut bytes).unwrap();
    let other = tempfile::tempdir().unwrap();
    let target = PersistenceStore::new_with_root(other.path().into());
    let prepared = archives(&target)
        .prepare_group(&mut Cursor::new(bytes))
        .unwrap();
    let GroupSettings::Screen(group) = &prepared.profile().content else {
        unreachable!()
    };
    assert_eq!(group.collections.as_ref().unwrap().images.len(), 256);
}

#[test]
fn malformed_manifest_and_payloads_never_publish_any_managed_asset_or_profile() {
    let dir = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(dir.path().into());
    let profile = sample(&source, false);
    let mut bytes = Vec::new();
    archives(&source)
        .export_group(&profile, &mut bytes)
        .unwrap();
    let mut bad_header = bytes.clone();
    bad_header[44] ^= 1;
    let mut bad_payload = bytes.clone();
    let offset = 44 + u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    bad_payload[offset] ^= 1;
    let mut extra = bytes.clone();
    extra.push(1);
    let malformed = [
        bad_header,
        bad_payload,
        extra,
        bytes[..bytes.len() - 1].to_vec(),
        mutate(&bytes, |m| m["version"] = 2.into()),
        mutate(&bytes, |m| {
            m["assets"].as_array_mut().unwrap().pop();
        }),
        mutate(&bytes, |m| {
            let e = m["assets"][0].clone();
            m["assets"].as_array_mut().unwrap().push(e);
        }),
        mutate(&bytes, |m| m["uploads"]["images"] = json!(["../../other"])),
    ];
    for bad in malformed {
        let dest_dir = tempfile::tempdir().unwrap();
        let dest = PersistenceStore::new_with_root(dest_dir.path().into());
        assert!(archives(&dest)
            .prepare_group(&mut Cursor::new(bad))
            .is_err());
        assert_eq!(dest.theme_assets().list().unwrap().len(), 2);
        assert_eq!(dest.media_assets().list().unwrap().len(), 1);
        assert_eq!(dest.profiles().list().unwrap().revision, 0);
        assert!(!dest_dir.path().join("theme-fonts").exists());
    }
}

#[test]
fn fan_and_lighting_group_has_no_screen_payloads_and_atomic_export_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let _screen = sample(&store, false);
    let mut latest = DeviceSettings::default();
    latest.fan.pwm_percent = 55;
    let group = GroupProfile::capture(&latest, &Default::default(), GroupKind::FanAndLighting)
        .unwrap()
        .profile;
    let path = dir.path().join("Cooling.lcg");
    archives(&store).export_group_file(&group, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(manifest(&bytes)["assets"], json!([]));
    assert!(archives(&store).export_group_file(&group, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let prepared = archives(&store)
        .prepare_group(&mut Cursor::new(&bytes))
        .unwrap();
    assert_eq!(
        serde_json::to_value(prepared.profile()).unwrap(),
        serde_json::to_value(&group).unwrap()
    );
    let mut slot = Vec::new();
    let profile = NamedProfile::defaults(ProfileGroup::Screen, 0, None).unwrap();
    archives(&store).export(&profile, &mut slot).unwrap();
    assert!(archives(&store)
        .prepare_group(&mut Cursor::new(&slot))
        .is_err());
    assert!(archives(&store).inspect(&mut Cursor::new(&bytes)).is_err());
}

#[test]
fn bounded_streams_and_prepared_pins_hold_assets_until_owner_commit_or_drop() {
    struct LimitedRead<'a>(&'a mut Cursor<Vec<u8>>);
    impl Read for LimitedRead<'_> {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            assert!(b.len() <= 64 * 1024);
            self.0.read(b)
        }
    }
    struct BoundedWriter<'a> {
        store: &'a PersistenceStore,
        bytes: Vec<u8>,
    }
    impl Write for BoundedWriter<'_> {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            assert!(b.len() <= 64 * 1024);
            self.store.theme_assets().list().unwrap();
            self.store.media_assets().list().unwrap();
            self.bytes.extend(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(dir.path().into());
    let group = sample(&source, false);
    let mut indexed = source
        .media_assets()
        .create_file("Large indexed upload".into())
        .unwrap();
    for index in 0..600 {
        let mut unit = if index == 0 {
            clip().units[0].clone()
        } else {
            clip().units[1].clone()
        };
        unit.bytes.resize(2048, 0x55);
        indexed.append(&unit).unwrap();
    }
    source.media_assets().publish_file(indexed).unwrap();
    let mut writer = BoundedWriter {
        store: &source,
        bytes: Vec::new(),
    };
    archives(&source).export_group(&group, &mut writer).unwrap();
    let dest_dir = tempfile::tempdir().unwrap();
    let dest = PersistenceStore::new_with_root(dest_dir.path().into());
    assert!(writer.bytes.len() > 1024 * 1024);
    let prepared = archives(&dest)
        .prepare_group(&mut LimitedRead(&mut Cursor::new(writer.bytes)))
        .unwrap();
    let large = dest
        .media_assets()
        .list()
        .unwrap()
        .into_iter()
        .find(|v| v.name == "Large indexed upload")
        .unwrap();
    let mut indexed = dest.media_assets().acquire_source(&large.key).unwrap();
    assert_eq!(indexed.frames(), 600);
    assert_eq!(indexed.read_unit(599).unwrap().bytes.len(), 2048);
    drop(indexed);
    let key = prepared.uploaded_images().first().unwrap().clone();
    let revision = dest
        .theme_assets()
        .list_named()
        .unwrap()
        .into_iter()
        .find(|a| a.key == key)
        .unwrap()
        .revision;
    assert!(dest
        .theme_assets()
        .delete_unreferenced(&key, revision, &[], || Ok(false))
        .is_err());
    let video = prepared.uploaded_media().first().unwrap().clone();
    assert!(dest
        .media_assets()
        .delete_unreferenced(&video, || Ok(false))
        .is_err());
    drop(prepared);
    dest.theme_assets()
        .delete_unreferenced(&key, revision, &[], || Ok(false))
        .unwrap();
    dest.media_assets()
        .delete_unreferenced(&video, || Ok(false))
        .unwrap();
}

#[test]
fn valid_checksums_do_not_bypass_decoding_including_last_video_unit() {
    let dir = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(dir.path().into());
    let group = sample(&source, false);
    let mut bytes = Vec::new();
    archives(&source).export_group(&group, &mut bytes).unwrap();
    let header = manifest(&bytes);
    let prefix = 44 + u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    for kind in ["image", "font", "media"] {
        let entries = header["assets"].as_array().unwrap();
        let index = entries.iter().position(|e| e["kind"] == kind).unwrap();
        let start = prefix
            + entries[..index]
                .iter()
                .map(|e| e["bytes"].as_u64().unwrap() as usize)
                .sum::<usize>();
        let length = entries[index]["bytes"].as_u64().unwrap() as usize;
        let mut damaged = bytes.clone();
        if kind == "media" {
            // The final unit in our fixture is [0,0,1,0x41,1]. Keep all
            // index/header/earlier access units intact, break only that NAL.
            damaged[start + length - 5..start + length].fill(0);
        } else {
            damaged[start..start + length].fill(0x41);
        }
        let checksum = format!("{:x}", Sha256::digest(&damaged[start..start + length]));
        let damaged = mutate(&damaged, |m| m["assets"][index]["sha256"] = checksum.into());
        let dest_dir = tempfile::tempdir().unwrap();
        let dest = PersistenceStore::new_with_root(dest_dir.path().into());
        assert!(archives(&dest)
            .inspect_group(&mut Cursor::new(&damaged))
            .is_ok());
        assert!(
            archives(&dest)
                .prepare_group(&mut Cursor::new(&damaged))
                .is_err(),
            "kind={kind}"
        );
        assert_eq!(dest.theme_assets().list().unwrap().len(), 2);
        assert_eq!(dest.media_assets().list().unwrap().len(), 1);
        assert!(!dest_dir.path().join("theme-fonts").exists());
    }
}

#[test]
fn failed_export_does_not_publish_a_file_and_writer_error_is_propagated() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected writer failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let valid = sample(&store, false);
    assert!(archives(&store)
        .export_group(&valid, &mut Broken)
        .unwrap_err()
        .to_string()
        .contains("injected writer failure"));
    let mut invalid = valid;
    let GroupSettings::Screen(ref mut screen) = invalid.content else {
        unreachable!()
    };
    screen
        .theme
        .as_mut()
        .unwrap()
        .background_image
        .as_mut()
        .unwrap()
        .asset = "f".repeat(32);
    let dest = dir.path().join("incomplete.lcg");
    assert!(archives(&store).export_group_file(&invalid, &dest).is_err());
    assert!(!dest.exists());
}

#[test]
fn manifest_font_budget_applies_to_each_theme_instead_of_all_slots_together() {
    let keys: Vec<_> = (1..=6).map(|i| format!("{i:032x}")).collect();
    let make_theme = |keys: &[String]| -> LcdTheme {
        serde_json::from_value(json!({"version":1,"name":"Fonts","background":[0,0,0],"layers":keys.iter().map(|key|json!({"kind":"text","text":"Text","style":{"x":0,"y":0,"size":24,"color":[255,255,255],"align":"left","font_asset":key}})).collect::<Vec<_>>()})).unwrap()
    };
    let mut settings = DeviceSettings {
        theme: Some(make_theme(&keys[..3])),
        screen_collections: Some(ScreenCollections::defaults(1)),
        ..Default::default()
    };
    let library = ProfileLibrary {
        profiles: vec![NamedProfile {
            slot: 7,
            name: "Other theme".into(),
            content: ProfileContent::Screen(make_theme(&keys[3..])),
        }],
        ..Default::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let make_archive = |profile: GroupProfile| {
        let header = json!({"version":1,"profile":profile,"uploads":{"images":[],"media":[]},"assets":keys.iter().map(|key|json!({"kind":"font","key":key,"bytes":16*1024*1024,"sha256":"0".repeat(64)})).collect::<Vec<_>>()});
        let header = serde_json::to_vec(&header).unwrap();
        let mut bytes = b"LCGROUP1".to_vec();
        bytes.extend((header.len() as u32).to_le_bytes());
        bytes.extend(Sha256::digest(&header));
        bytes.extend(header);
        bytes
    };
    let profile = GroupProfile::capture(&settings, &library, GroupKind::Screen)
        .unwrap()
        .profile;
    let bytes = make_archive(profile);
    assert!(
        archives(&store)
            .inspect_group(&mut Cursor::new(&bytes))
            .is_ok(),
        "96MiB aggregate across two48MiB themes is valid metadata"
    );
    assert!(
        archives(&store)
            .prepare_group(&mut Cursor::new(&bytes))
            .is_err(),
        "metadata inspection never validates absent payloads"
    );
    settings.theme = Some(make_theme(&keys));
    let profile = GroupProfile::capture(&settings, &library, GroupKind::Screen)
        .unwrap()
        .profile;
    assert!(
        archives(&store)
            .inspect_group(&mut Cursor::new(make_archive(profile)))
            .is_err(),
        "a single96MiB theme exceeds renderer budget"
    );
}

#[tokio::test]
async fn owner_snapshot_includes_queued_deltas_without_flushing_or_changing_device_intent() {
    use llcontrol_daemon::{
        actor::command_dispatch::{spawn_updates_observed, SettingsUpdate},
        persistence::{FanSection, LcdSection},
    };
    use std::sync::Arc;
    use tokio::sync::{mpsc, oneshot, watch};
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let (tx, rx) = mpsc::channel(8);
    let (_stop, stop) = watch::channel(false);
    let (task, status) = spawn_updates_observed(
        store.clone(),
        "device".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    tx.send(SettingsUpdate::Fan(FanSection {
        pwm_percent: 63,
        ..Default::default()
    }))
    .await
    .unwrap();
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::ReadProfileGroup {
        kind: GroupKind::FanAndLighting,
        reply,
    })
    .await
    .unwrap();
    let snapshot = response.await.unwrap().unwrap();
    let GroupSettings::FanAndLighting(group) = snapshot.profile.content else {
        unreachable!()
    };
    assert_eq!(group.fan.pwm_percent, 63);
    assert!(status.borrow().pending);
    tx.send(SettingsUpdate::Lcd(LcdSection {
        brightness: 78,
        rotation: "90".into(),
        control_mode: "Application".into(),
    }))
    .await
    .unwrap();
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::ReadProfileGroup {
        kind: GroupKind::Screen,
        reply,
    })
    .await
    .unwrap();
    let snapshot = response.await.unwrap().unwrap();
    let GroupSettings::Screen(group) = snapshot.profile.content else {
        unreachable!()
    };
    assert_eq!(group.lcd.brightness, 78);
    assert!(status.borrow().pending);
    assert_eq!(snapshot.revision.profile_revision, 0);
    drop(tx);
    task.await.unwrap();
    let saved = store.read("device").unwrap();
    assert_eq!(saved.fan.pwm_percent, 63);
    assert_eq!(saved.lcd.brightness, 78);
    assert_eq!(
        store.profiles().list().unwrap().revision,
        0,
        "read must not commit a new profile group"
    );
}
