use lconnect_daemon::theme_assets::ThemeAssetStore;
use lconnect_ipc::{ImageFit, LcdTheme, ThemeLayer};
fn theme(asset: &str) -> LcdTheme {
    LcdTheme {
        page_cache: Vec::new(),
        basic_editor: None,
        version: 1,
        name: "Imported".into(),
        background: [0; 3],
        background_image: None,
        background_video: None,
        background_fill: None,
        layers: vec![ThemeLayer::Image {
            editor: None,
            watermark_position: None,
            shadow: None,
            asset: asset.into(),
            x: 0,
            y: 0,
            width: 480,
            height: 480,
            fit: ImageFit::Contain,
            opacity: 255,
        }],
    }
}
#[test]
fn import_persists_and_reload_requires_all_referenced_assets() {
    let directory = tempfile::tempdir().unwrap();
    let store = ThemeAssetStore::new(directory.path().join("assets"));
    // A real 1x1 24-bit BMP; the repository icon is an empty placeholder.
    let mut bmp = vec![0u8; 58];
    bmp[..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&58u32.to_le_bytes());
    bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&1u32.to_le_bytes());
    bmp[22..26].copy_from_slice(&1u32.to_le_bytes());
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
    bmp[54..57].copy_from_slice(&[10, 20, 30]);
    let image = bmp.as_slice();
    let key = store.import(image).unwrap();
    let saved = theme(&key);
    let loaded = ThemeAssetStore::new(directory.path().join("assets"))
        .load(&saved)
        .unwrap();
    assert_eq!(loaded.len(), 1);
    assert!(loaded[&key].width() <= 960);
    let second = store.import(image).unwrap();
    assert_ne!(key, second);
    std::fs::remove_file(directory.path().join("assets").join(format!("{key}.png"))).unwrap();
    assert!(store.load(&saved).is_err());
    assert!(store.load(&theme("../escape")).is_err());
    assert!(store.import(b"invalid").is_err());
}
#[cfg(unix)]
#[test]
fn nonregular_and_corrupt_assets_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let store = ThemeAssetStore::new(directory.path().to_owned());
    std::os::unix::fs::symlink("/dev/zero", directory.path().join("link.png")).unwrap();
    assert!(store.load(&theme("link")).is_err());
    std::fs::create_dir(directory.path().join("dir.png")).unwrap();
    assert!(store.load(&theme("dir")).is_err());
    std::fs::write(directory.path().join("broken.png"), b"bad png").unwrap();
    assert!(store.load(&theme("broken")).is_err());
}

fn png() -> Vec<u8> {
    include_bytes!("../../../desktop/src-tauri/icons/icon.png").to_vec()
}

#[test]
fn cleanup_retains_disk_references_drafts_and_shared_session_pins() {
    use std::collections::BTreeSet;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("assets");
    let previous = ThemeAssetStore::new(root.clone());
    let saved = previous.import(&png()).unwrap();
    let draft = previous.import(&png()).unwrap();
    let orphan = previous.import(&png()).unwrap();
    let replay = previous.import(&png()).unwrap();
    assert!(previous
        .cleanup(&BTreeSet::new(), &[], true)
        .unwrap()
        .deleted
        .is_empty());
    // New process has no old session pins. Loading via one clone protects all clones.
    let current = ThemeAssetStore::new(root.clone());
    current.clone().load(&theme(&replay)).unwrap();
    let imported = current.import(&png()).unwrap();
    let refs = BTreeSet::from([saved.clone()]);
    let preview = current
        .cleanup(&refs, std::slice::from_ref(&draft), false)
        .unwrap();
    assert_eq!(preview.candidates.as_slice(), std::slice::from_ref(&orphan));
    assert!(root.join(format!("{orphan}.png")).exists());
    let deleted = current
        .cleanup(&refs, std::slice::from_ref(&draft), true)
        .unwrap();
    assert_eq!(deleted.deleted, [orphan]);
    assert_eq!(deleted.retained_referenced, 1);
    assert_eq!(deleted.retained_draft, 1);
    assert_eq!(deleted.retained_session, 2);
    for key in [saved, draft, replay, imported] {
        assert!(root.join(format!("{key}.png")).exists());
    }
}

#[test]
fn strict_settings_scan_blocks_cleanup_on_unreadable_schema() {
    use lconnect_daemon::persistence::{DeviceSettings, PersistenceStore};
    let directory = tempfile::tempdir().unwrap();
    let persistence = PersistenceStore::new_with_root(directory.path().into());
    let key = persistence.theme_assets().import(&png()).unwrap();
    persistence
        .write(
            "test",
            &DeviceSettings {
                theme: Some(theme(&key)),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(persistence
        .referenced_theme_assets()
        .unwrap()
        .contains(&key));
    std::fs::write(
        directory.path().join("devices/test.toml"),
        "schema_version = 2",
    )
    .unwrap();
    assert!(persistence.referenced_theme_assets().is_err());
    std::fs::write(
        directory.path().join("devices/test.toml"),
        "invalid settings",
    )
    .unwrap();
    assert!(persistence.referenced_theme_assets().is_err());
}

#[test]
fn load_and_cleanup_serialize_so_a_successfully_loaded_asset_survives() {
    for _ in 0..8 {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("assets");
        let key = ThemeAssetStore::new(root.clone()).import(&png()).unwrap();
        let store = ThemeAssetStore::new(root.clone());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let loading = {
            let store = store.clone();
            let barrier = barrier.clone();
            let key = key.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.load(&theme(&key))
            })
        };
        barrier.wait();
        let report = store.cleanup(&Default::default(), &[], true).unwrap();
        let loaded = loading.join().unwrap();
        if loaded.is_ok() {
            assert!(report.deleted.is_empty());
            assert!(root.join(format!("{key}.png")).exists());
        } else {
            assert_eq!(report.deleted, [key]);
        }
    }
}

#[test]
fn gallery_survives_restart_and_browsing_does_not_pin_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("assets");
    let initial = ThemeAssetStore::new(root.clone());
    assert_eq!(initial.list().unwrap(), builtin_keys());
    let key = initial.import(&png()).unwrap();
    let current = ThemeAssetStore::new(root.clone());
    assert_eq!(
        current.list().unwrap(),
        [builtin_keys(), vec![key.clone()]].concat()
    );
    let preview = current.thumbnail(&key).unwrap();
    let image = lconnect_image::assets::decode_asset(&preview).unwrap();
    assert!(image.width() <= 144 && image.height() <= 144);
    assert!(current.thumbnail("../escape").is_err());
    std::fs::write(root.join("notes.png"), b"not managed").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.join(format!("{key}.png")),
        root.join(format!("{}.png", "f".repeat(32))),
    )
    .unwrap();
    assert_eq!(
        current.list().unwrap(),
        [builtin_keys(), vec![key.clone()]].concat()
    );
    let report = current.cleanup(&Default::default(), &[], true).unwrap();
    assert_eq!(report.deleted, vec![key.clone()]);
    assert!(current.thumbnail(&key).is_err());
}

#[test]
fn image_names_are_durable_versioned_and_do_not_change_pixels_or_references() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("assets");
    let store = ThemeAssetStore::new(root.clone());
    let legacy = store.import(&png()).unwrap();
    let key = store.import_named(&png(), Some("Цветок.png")).unwrap();
    let original = std::fs::read(root.join(format!("{key}.png"))).unwrap();
    assert!(store.import_named(&png(), Some(" ")).is_err());
    let current = ThemeAssetStore::new(root.clone());
    let names = current.list_named().unwrap();
    assert_eq!(names.iter().find(|n| n.key == legacy).unwrap().revision, 0);
    assert_eq!(
        names.iter().find(|n| n.key == key).unwrap().name.as_deref(),
        Some("Цветок.png")
    );
    assert_eq!(current.rename(&key, "Сад", 1).unwrap().revision, 2);
    assert!(current.rename(&key, "Stale", 1).is_err());
    assert!(current.rename(&key, "bad\nname", 2).is_err());
    assert_eq!(
        std::fs::read(root.join(format!("{key}.png"))).unwrap(),
        original
    );
    assert!(current.load(&theme(&key)).is_ok());
    let metadata = root.join(format!("{key}.name.json"));
    std::fs::write(&metadata, br#"{"version":99,"revision":3,"name":"Future"}"#).unwrap();
    assert!(current.list_named().is_err());
    assert!(current.rename(&key, "Overwrite", 3).is_err());
    assert!(std::fs::read_to_string(metadata).unwrap().contains("99"));
    // Legacy names may be added; cleanup removes their sidecar as well.
    current.rename(&legacy, "Legacy", 0).unwrap();
    let fresh = ThemeAssetStore::new(root.clone());
    fresh
        .cleanup(&[key].into_iter().collect(), &[], true)
        .unwrap();
    assert!(!root.join(format!("{legacy}.name.json")).exists());
}

#[test]
fn concurrent_image_rename_has_one_winner() {
    let directory = tempfile::tempdir().unwrap();
    let store = ThemeAssetStore::new(directory.path().join("assets"));
    let key = store.import(&png()).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = ["First", "Second"]
        .into_iter()
        .map(|name| {
            let store = store.clone();
            let key = key.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.rename(&key, name, 0).is_ok()
            })
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|ok| *ok)
            .count(),
        1
    );
    assert_eq!(
        store
            .list_named()
            .unwrap()
            .iter()
            .find(|entry| entry.key == key)
            .unwrap()
            .revision,
        1
    );
}

fn builtin_keys() -> Vec<String> {
    lconnect_image::defaults::IMAGES
        .iter()
        .map(|a| a.key.to_owned())
        .collect()
}

#[test]
fn embedded_defaults_survive_restart_cleanup_and_disk_shadowing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("absent");
    let store = ThemeAssetStore::new(root.clone());
    assert_eq!(store.list().unwrap(), builtin_keys());
    assert!(!root.exists(), "browsing must not require bootstrap writes");
    let entries = store.list_named().unwrap();
    for entry in entries {
        assert!(entry.is_default);
        let before = store.thumbnail(&entry.key).unwrap();
        let decoded = store.load(&theme(&entry.key)).unwrap();
        assert_eq!(
            (decoded[&entry.key].width(), decoded[&entry.key].height()),
            (480, 480)
        );
        assert!(store.rename(&entry.key, "Changed", 0).is_err());
        assert!(store
            .delete_unreferenced(&entry.key, 0, &[], || Ok(false))
            .is_err());
        // User files cannot replace embedded artwork or metadata, even across restart.
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(format!("{}.png", entry.key)), b"corrupt").unwrap();
        std::fs::write(root.join(format!("{}.name.json", entry.key)), b"corrupt").unwrap();
        let restarted = ThemeAssetStore::new(root.clone());
        assert_eq!(restarted.thumbnail(&entry.key).unwrap(), before);
        assert!(restarted.load(&theme(&entry.key)).is_ok());
        assert!(restarted
            .cleanup(&Default::default(), &[], true)
            .unwrap()
            .deleted
            .is_empty());
        assert_eq!(restarted.list().unwrap(), builtin_keys());
        assert_eq!(restarted.list_named().unwrap().len(), 2);
    }
    let user = store.import_named(&png(), Some("User image")).unwrap();
    assert!(
        !store
            .list_named()
            .unwrap()
            .iter()
            .find(|a| a.key == user)
            .unwrap()
            .is_default
    );
    store.rename(&user, "Renamed", 1).unwrap();
    store
        .delete_unreferenced(&user, 2, &[], || Ok(false))
        .unwrap();
    assert_eq!(store.list().unwrap(), builtin_keys());
}

#[test]
fn static_background_is_preloaded_and_held_by_the_same_live_lease_as_overlays() {
    let dir = tempfile::tempdir().unwrap();
    let store = ThemeAssetStore::new(dir.path().to_owned());
    let key = store.import(&png()).unwrap();
    let mut saved = theme(&key);
    saved.layers.clear();
    saved.background_image = Some(lconnect_ipc::BackgroundImage {
        asset: key.clone(),
        fit: ImageFit::Crop,
    });
    let loaded = store.load(&saved).unwrap();
    assert!(loaded.contains_key(&key));
    assert!(store
        .delete_unreferenced(&key, 0, &[], || Ok(false))
        .is_err());
    drop(loaded);
    store
        .delete_unreferenced(&key, 0, &[], || Ok(false))
        .unwrap();
    assert!(store.load(&saved).is_err());
}

#[test]
fn editor_reads_full_immutable_asset_and_rejects_builtin_paths_and_bad_files() {
    let root = tempfile::tempdir().unwrap();
    let store = ThemeAssetStore::new(root.path().into());
    let source = include_bytes!("../../../desktop/src-tauri/tests/fixtures/editor-wide.png");
    let key = store.import(source).unwrap();
    let path = root.path().join(format!("{key}.png"));
    let expected = std::fs::read(&path).unwrap();
    assert_eq!(store.read_for_edit(&key).unwrap(), expected);
    let decoded = lconnect_image::assets::decode_asset(&expected).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (960, 480));
    assert!(store
        .read_for_edit(lconnect_ipc::BUILTIN_IMAGE_KEY)
        .is_err());
    assert!(store.read_for_edit("../escape").is_err());
    std::fs::write(&path, b"corrupt PNG").unwrap();
    assert!(store.read_for_edit(&key).is_err());
    std::fs::write(&path, vec![0; 1024 * 1024 + 1]).unwrap();
    assert!(store.read_for_edit(&key).is_err());
    std::fs::remove_file(&path).unwrap();
    assert!(store.read_for_edit(&key).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/dev/zero", &path).unwrap();
        assert!(store.read_for_edit(&key).is_err());
    }
}
