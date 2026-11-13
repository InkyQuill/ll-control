use llcontrol_daemon::theme_fonts::ThemeFontStore;
const FONT: &[u8] = include_bytes!("../../llcontrol-image/assets/DejaVuSans.ttf");

#[test]
fn font_import_preserves_bytes_and_survives_new_store_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fonts");
    let store = ThemeFontStore::new(root.clone());
    assert!(!root.exists());
    let a = store.import(FONT).unwrap();
    let b = store.import(FONT).unwrap();
    assert_ne!(a, b);
    let reopened = ThemeFontStore::new(root);
    assert_eq!(reopened.load(&a).unwrap(), FONT);
    assert_eq!(reopened.load(&b).unwrap(), FONT);
    for key in [
        "../outside",
        "/etc/passwd",
        "",
        "0123456789abcdef0123456789abcdeF",
    ] {
        assert!(reopened.load(key).is_err());
    }
}

#[test]
fn invalid_input_missing_corrupt_symlink_and_directory_fail_without_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fonts");
    let store = ThemeFontStore::new(root.clone());
    assert!(store.import(b"not a font").is_err());
    assert!(!root.exists());
    let key = store.import(FONT).unwrap();
    let path = root.join(format!("{key}.font"));
    std::fs::write(&path, b"broken").unwrap();
    assert!(store.load(&key).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    std::fs::remove_file(&path).unwrap();
    assert!(store.load(&key).is_err());
    let outside = temp.path().join("outside.ttf");
    std::fs::write(&outside, FONT).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(store.load(&key).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(store.load(&key).is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), FONT);
}

#[test]
fn oversize_disk_asset_and_unwritable_root_never_replace_existing_data() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fonts");
    std::fs::write(&root, b"keep me").unwrap();
    let store = ThemeFontStore::new(root.clone());
    assert!(store.import(FONT).is_err());
    assert_eq!(std::fs::read(&root).unwrap(), b"keep me");
    std::fs::remove_file(&root).unwrap();
    let key = store.import(FONT).unwrap();
    let path = root.join(format!("{key}.font"));
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(llcontrol_image::fonts::MAX_FONT_BYTES as u64 + 1)
        .unwrap();
    assert!(store.load(&key).is_err());
    assert_eq!(
        std::fs::metadata(path).unwrap().len(),
        llcontrol_image::fonts::MAX_FONT_BYTES as u64 + 1
    );
}

#[tokio::test]
async fn prepared_runtime_owns_fonts_after_file_loss_but_new_preparation_fails() {
    use llcontrol_daemon::theme_runtime::ThemeRuntime;
    use llcontrol_image::theme::ThemeAssets;
    use std::sync::Arc;
    let temp = tempfile::tempdir().unwrap();
    let store = ThemeFontStore::new(temp.path().into());
    let key = store.import(FONT).unwrap();
    let theme = serde_json::from_value(serde_json::json!({"version":1,"name":"Fonts","background":[0,0,0],"layers":[{
        "kind":"text","text":"Hello", "style":{"font_asset":key,"x":1,"y":1,"size":20,"color":[255,255,255],"align":"left"}
    }]})).unwrap();
    let fonts = store.load_theme(&theme).unwrap();
    assert_eq!(fonts.len(), 1);
    let runtime = Arc::new(
        ThemeRuntime::with_fonts(theme.clone(), ThemeAssets::new(), fonts, vec![]).unwrap(),
    );
    let before = runtime.frame_at(0).await.unwrap();
    std::fs::remove_file(temp.path().join(format!("{key}.font"))).unwrap();
    assert_eq!(runtime.frame_at(0).await.unwrap(), before);
    assert!(store.load_theme(&theme).is_err());
    assert!(ThemeRuntime::new(theme, ThemeAssets::new(), vec![]).is_err());
}

#[test]
fn font_preload_deduplicates_keys_and_bounds_total_bytes_at_64_mib() {
    let temp = tempfile::tempdir().unwrap();
    let store = ThemeFontStore::new(temp.path().into());
    let mut bytes = FONT.to_vec();
    bytes.resize(llcontrol_image::fonts::MAX_FONT_BYTES, 0);
    let keys: Vec<String> = (0..5).map(|i| format!("{i:032x}")).collect();
    for key in &keys {
        std::fs::write(temp.path().join(format!("{key}.font")), &bytes).unwrap();
    }
    let layer = |key: &str| {
        serde_json::json!({"kind":"text","text":"A", "style":{
            "font_asset":key,"x":1,"y":1,"size":20,"color":[255,255,255],"align":"left"
        }})
    };
    let mut layers: Vec<_> = keys[..4].iter().map(|key| layer(key)).collect();
    layers.push(layer(&keys[0]));
    let theme = |layers| {
        serde_json::from_value(serde_json::json!({"version":1,"name":"Bounded fonts","background":[0,0,0],"layers":layers})).unwrap()
    };
    assert_eq!(store.load_theme(&theme(layers.clone())).unwrap().len(), 4);
    layers.push(layer(&keys[4]));
    let error = store.load_theme(&theme(layers)).unwrap_err();
    assert!(error.to_string().contains("64 MiB"));
    assert_eq!(
        std::fs::metadata(temp.path().join(format!("{}.font", keys[4])))
            .unwrap()
            .len(),
        bytes.len() as u64
    );
}
