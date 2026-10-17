use lconnect_daemon::{
    media_assets::MediaAssetStore, profile_archive::ProfileArchives, profiles::ProfileStore,
    theme_assets::ThemeAssetStore, theme_fonts::ThemeFontStore,
};
use lconnect_ipc::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read, Write},
    path::Path,
};
const FONT: &[u8] = include_bytes!("../../lconnect-image/assets/DejaVuSans.ttf");
fn stores(
    root: &Path,
) -> (
    ProfileArchives,
    ThemeAssetStore,
    ThemeFontStore,
    MediaAssetStore,
    ProfileStore,
) {
    let images = ThemeAssetStore::new(root.join("images"));
    let fonts = ThemeFontStore::new(root.join("fonts"));
    let media = MediaAssetStore::new(root.join("media"));
    (
        ProfileArchives::new(images.clone(), fonts.clone(), media.clone()),
        images,
        fonts,
        media,
        ProfileStore::new(root.join("state")),
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
fn loaded_clip(media: &MediaAssetStore, key: &str) -> H264Clip {
    let mut source = media.acquire_source(key).unwrap();
    H264Clip {
        fps: source.fps(),
        units: (0..source.frames())
            .map(|i| source.read_unit(i).unwrap())
            .collect(),
    }
}
fn style(font: &str) -> Value {
    json!({"x":240,"y":240,"size":24,"color":[255,255,255],"align":"center","font_asset":font})
}
fn fixture(root: &Path) -> (ProfileArchives, NamedProfile) {
    let (archive, images, fonts, media, _) = stores(root);
    let image = images
        .import_named(lconnect_image::defaults::IMAGES[0].png, Some("Моя орбита"))
        .unwrap();
    let font = fonts.import(FONT).unwrap();
    let video = media.import("Motion".into(), clip()).unwrap().key;
    let mut theme:LcdTheme=serde_json::from_value(json!({"version":1,"name":"Portable","background":[0,0,0],"background_image":{"asset":image,"fit":"crop"},"layers":[
 {"kind":"image","asset":image,"x":0,"y":0,"width":480,"height":480,"fit":"contain","opacity":100},
 {"kind":"text","text":image,"style":style(&font)},
 {"kind":"sensor","sensor_id":image,"label":"Exact ID, not an asset","unit":"percent","temperature_unit":"celsius","decimals":0,"style":style(&font),"parts":{"label":style(&font),"unit":style(&font)}}]})).unwrap();
    let rain:LcdTheme=serde_json::from_value(json!({"version":3,"name":"Rain cached","background":[0,0,0],"background_video":video,"layers":[{"kind":"weather","effect":{"kind":"rain","intensity":1,"dynamic":false,"sensor":null,"seed":42}},{"kind":"clock","twenty_four_hour":false,"show_seconds":true,"style":style(&font)},{"kind":"image","asset":BUILTIN_IMAGE_KEY,"x":0,"y":0,"width":480,"height":480,"fit":"contain","opacity":100}]})).unwrap();
    theme.page_cache.push(SavedLcdPage {
        page: LcdPageKind::Rain,
        theme: Box::new(rain),
    });
    let profile = NamedProfile {
        slot: 2,
        name: "Переносимый экран".into(),
        content: ProfileContent::Screen(theme),
    };
    profile.validate().unwrap();
    (archive, profile)
}
fn mutate(bytes: &[u8], change: impl FnOnce(&mut Value)) -> Vec<u8> {
    let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut header: Value = serde_json::from_slice(&bytes[44..44 + len]).unwrap();
    change(&mut header);
    let encoded = serde_json::to_vec(&header).unwrap();
    let mut out = b"LCPROF01".to_vec();
    out.extend((encoded.len() as u32).to_le_bytes());
    out.extend(Sha256::digest(&encoded));
    out.extend(encoded);
    out.extend(&bytes[44 + len..]);
    out
}
#[test]
fn roundtrip_copies_all_cached_page_dependencies_remaps_only_asset_fields_and_survives_restart() {
    let source = tempfile::tempdir().unwrap();
    let (exporter, original) = fixture(source.path());
    let mut bytes = Vec::new();
    exporter.export(&original, &mut bytes).unwrap();
    let dest = tempfile::tempdir().unwrap();
    let (importer, images, fonts, media, library) = stores(dest.path());
    let untouched = NamedProfile {
        slot: 0,
        name: "Fan retained".into(),
        content: ProfileContent::Fan(CoolingPolicy::Fixed { percent: 40 }),
    };
    library
        .edit(
            0,
            ProfileEdit::Save {
                profile: untouched.clone(),
            },
        )
        .unwrap();
    let saved = importer
        .import(&mut Cursor::new(&bytes), &library, 1, 7)
        .unwrap();
    assert_eq!(saved.revision, 2);
    assert_eq!(saved.profiles[0], untouched);
    let profile = &saved.profiles[1];
    assert_eq!(profile.slot, 7);
    assert_eq!(profile.name, original.name);
    let ProfileContent::Screen(theme) = &profile.content else {
        panic!()
    };
    let ProfileContent::Screen(before) = &original.content else {
        panic!()
    };
    assert_eq!(theme.pages().count(), 2);
    assert_eq!(theme.image_assets().len(), 2);
    assert_eq!(theme.font_assets().len(), 1);
    assert_eq!(theme.video_assets().len(), 1);
    assert!(theme.image_assets().is_disjoint(&before.image_assets()));
    assert!(theme.font_assets().is_disjoint(&before.font_assets()));
    assert!(theme.video_assets().is_disjoint(&before.video_assets()));
    let old_id = before.background_image.as_ref().unwrap().asset.clone();
    let ThemeLayer::Text { text, .. } = &theme.layers[1] else {
        panic!()
    };
    assert_eq!(text, &old_id);
    let ThemeLayer::Sensor { sensor_id, .. } = &theme.layers[2] else {
        panic!()
    };
    assert_eq!(sensor_id, &old_id, "sensor identities must not be remapped");
    images.load(theme).unwrap();
    fonts.load_theme(theme).unwrap();
    let key = *theme.video_assets().iter().next().unwrap();
    assert_eq!(loaded_clip(&media, key), clip());
    assert_eq!(media.acquire_source(key).unwrap().name(), "Motion");
    assert!(images
        .list_named()
        .unwrap()
        .iter()
        .any(|i| i.name.as_deref() == Some("Моя орбита")));
    for key in theme.font_assets() {
        assert_eq!(fonts.load(key).unwrap(), FONT);
    }
    assert_eq!(
        ProfileStore::new(dest.path().join("state")).list().unwrap(),
        saved
    );
    let mut again = Vec::new();
    importer.export(profile, &mut again).unwrap();
    assert!(again.len() > FONT.len());
}
#[test]
fn incomplete_corrupt_missing_duplicate_oversized_and_extra_data_never_publish_assets_or_replace_slots(
) {
    let source = tempfile::tempdir().unwrap();
    let (exporter, profile) = fixture(source.path());
    let mut bytes = Vec::new();
    exporter.export(&profile, &mut bytes).unwrap();
    let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut corrupt = bytes.clone();
    corrupt[44 + header_len] ^= 1;
    let mut trailing = bytes.clone();
    trailing.push(1);
    let mut corrupt_manifest = bytes.clone();
    corrupt_manifest[44] ^= 1;
    let cases = vec![
        corrupt_manifest,
        bytes[..8].to_vec(),
        bytes[..bytes.len() - 1].to_vec(),
        corrupt,
        trailing,
        mutate(&bytes, |h| {
            h["assets"].as_array_mut().unwrap().pop();
        }),
        mutate(&bytes, |h| {
            let e = h["assets"][0].clone();
            h["assets"].as_array_mut().unwrap().push(e);
        }),
        mutate(&bytes, |h| h["assets"][0]["bytes"] = json!(u64::MAX)),
        mutate(&bytes, |h| h["assets"][0]["key"] = json!("../../outside")),
        mutate(&bytes, |h| h["version"] = json!(2)),
    ];
    for bytes in cases {
        let dest = tempfile::tempdir().unwrap();
        let (importer, images, _, media, library) = stores(dest.path());
        let before = library
            .edit(
                0,
                ProfileEdit::Save {
                    profile: NamedProfile::defaults(ProfileGroup::Screen, 0, None).unwrap(),
                },
            )
            .unwrap();
        assert!(importer
            .import(&mut Cursor::new(bytes), &library, 1, 0)
            .is_err());
        assert_eq!(library.list().unwrap(), before);
        assert_eq!(images.list().unwrap().len(), 2, "no published user images");
        assert_eq!(media.list().unwrap().len(), 1, "only builtin media");
        assert!(!dest.path().join("fonts").exists());
    }
}
#[test]
fn cooling_and_media_slots_roundtrip_without_applying_and_export_never_overwrites() {
    let source = tempfile::tempdir().unwrap();
    let (exporter, _, _, media, _) = stores(source.path());
    let dest = tempfile::tempdir().unwrap();
    let (importer, _, _, new_media, library) = stores(dest.path());
    let key = media.import("Clip".into(), clip()).unwrap().key;
    let profiles = [
        NamedProfile {
            slot: 0,
            name: "Media".into(),
            content: ProfileContent::ScreenMedia(key),
        },
        NamedProfile {
            slot: 0,
            name: "Fan".into(),
            content: ProfileContent::Fan(CoolingPolicy::Fixed { percent: 35 }),
        },
        NamedProfile {
            slot: 1,
            name: "Pump".into(),
            content: ProfileContent::Pump(CoolingPolicy::Fixed { percent: 70 }),
        },
    ];
    for (revision, p) in profiles.iter().enumerate() {
        let path = source.path().join(format!("profile{revision}.lcp"));
        exporter.export_file(p, &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        assert!(exporter.export_file(p, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let saved = importer
            .import(
                &mut Cursor::new(original),
                &library,
                revision as u64,
                p.slot,
            )
            .unwrap();
        assert_eq!(saved.revision, revision as u64 + 1);
    }
    let saved = library.list().unwrap();
    let ProfileContent::ScreenMedia(key) = &saved.profiles[0].content else {
        panic!()
    };
    assert_eq!(loaded_clip(&new_media, key), clip());
    assert_eq!(saved.profiles[1], profiles[1]);
    assert_eq!(saved.profiles[2], profiles[2]);
}
#[test]
fn stale_revision_rejects_before_read_and_invalid_target_rejects_before_payload() {
    struct NoRead;
    impl Read for NoRead {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("stale import must not read input")
        }
    }
    let dest = tempfile::tempdir().unwrap();
    let (importer, _, _, _, library) = stores(dest.path());
    assert!(importer.import(&mut NoRead, &library, 1, 0).is_err());
    let source = tempfile::tempdir().unwrap();
    let (exporter, profile) = fixture(source.path());
    let mut bytes = Vec::new();
    exporter.export(&profile, &mut bytes).unwrap();
    let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut input = Cursor::new(bytes);
    assert!(importer.import(&mut input, &library, 0, 8).is_err());
    assert_eq!(input.position(), 44 + length as u64);
    assert!(library.list().unwrap().profiles.is_empty());
}
#[test]
fn missing_export_dependency_leaves_destination_absent_and_write_failure_is_reported() {
    let source = tempfile::tempdir().unwrap();
    let (exporter, mut profile) = fixture(source.path());
    let ProfileContent::Screen(theme) = &mut profile.content else {
        panic!()
    };
    theme.background_image.as_mut().unwrap().asset = "a".repeat(32);
    let path = source.path().join("failed.lcp");
    assert!(exporter.export_file(&profile, &path).is_err());
    assert!(!path.exists());
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected write failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let profile = NamedProfile::defaults(ProfileGroup::Fan, 0, Some("cpu")).unwrap();
    assert!(exporter.export(&profile, &mut Broken).is_err());
}

#[test]
fn valid_checksums_do_not_bypass_image_font_or_media_validation() {
    let source = tempfile::tempdir().unwrap();
    let (exporter, profile) = fixture(source.path());
    let mut bytes = Vec::new();
    exporter.export(&profile, &mut bytes).unwrap();
    let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let header: Value = serde_json::from_slice(&bytes[44..44 + length]).unwrap();
    for kind in ["image", "font", "media"] {
        let mut changed = header.clone();
        let entries = changed["assets"].as_array_mut().unwrap();
        let index = entries.iter().position(|e| e["kind"] == kind).unwrap();
        let offset: usize = entries[..index]
            .iter()
            .map(|e| e["bytes"].as_u64().unwrap() as usize)
            .sum();
        let old = entries[index]["bytes"].as_u64().unwrap() as usize;
        let invalid = b"not a valid typed dependency";
        entries[index]["bytes"] = json!(invalid.len());
        entries[index]["sha256"] = json!(format!("{:x}", Sha256::digest(invalid)));
        let encoded = serde_json::to_vec(&changed).unwrap();
        let mut payload = bytes[44 + length..].to_vec();
        payload.splice(offset..offset + old, invalid.iter().copied());
        let mut archive = b"LCPROF01".to_vec();
        archive.extend((encoded.len() as u32).to_le_bytes());
        archive.extend(Sha256::digest(&encoded));
        archive.extend(encoded);
        archive.extend(payload);
        let dest = tempfile::tempdir().unwrap();
        let (importer, images, _, media, library) = stores(dest.path());
        assert!(
            importer
                .import(&mut Cursor::new(archive), &library, 0, 0)
                .is_err(),
            "{kind}"
        );
        assert!(library.list().unwrap().profiles.is_empty());
        assert_eq!(images.list().unwrap().len(), 2);
        assert_eq!(media.list().unwrap().len(), 1);
        assert!(!dest.path().join("fonts").exists());
    }
}

#[test]
fn concurrent_library_edit_during_archive_read_is_not_overwritten() {
    let source = tempfile::tempdir().unwrap();
    let (exporter, profile) = fixture(source.path());
    let mut bytes = Vec::new();
    exporter.export(&profile, &mut bytes).unwrap();
    let dest = tempfile::tempdir().unwrap();
    let (importer, _, _, _, library) = stores(dest.path());
    struct Racing<'a> {
        store: &'a ProfileStore,
        input: Cursor<Vec<u8>>,
        edited: bool,
    }
    impl Read for Racing<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            if !self.edited {
                self.edited = true;
                self.store
                    .edit(
                        0,
                        ProfileEdit::Save {
                            profile: NamedProfile::defaults(
                                ProfileGroup::Fan,
                                0,
                                Some("retained:sensor"),
                            )
                            .unwrap(),
                        },
                    )
                    .unwrap();
            }
            self.input.read(bytes)
        }
    }
    let mut input = Racing {
        store: &library,
        input: Cursor::new(bytes),
        edited: false,
    };
    assert!(importer.import(&mut input, &library, 0, 7).is_err());
    let saved = library.list().unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.profiles.len(), 1);
    assert_eq!(saved.profiles[0].content.group(), ProfileGroup::Fan);
}

#[test]
fn indexed_video_larger_than_diagnostic_limit_uses_bounded_transfer_buffers() {
    let source = tempfile::tempdir().unwrap();
    let (exporter, _, _, media, _) = stores(source.path());
    let mut writer = media.create_file("Long portable video".into()).unwrap();
    for i in 0..600 {
        let mut unit = if i == 0 {
            clip().units[0].clone()
        } else {
            clip().units[1].clone()
        };
        unit.bytes.resize(4096, 1);
        writer.append(&unit).unwrap();
    }
    let key = media.publish_file(writer).unwrap().key;
    let profile = NamedProfile {
        slot: 0,
        name: "Long".into(),
        content: ProfileContent::ScreenMedia(key),
    };
    struct BoundedOutput {
        bytes: Vec<u8>,
    }
    impl Write for BoundedOutput {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            assert!(bytes.len() <= 65536);
            self.bytes.extend(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = BoundedOutput { bytes: Vec::new() };
    exporter.export(&profile, &mut output).unwrap();
    assert!(output.bytes.len() > 1024 * 1024);
    struct BoundedInput(Cursor<Vec<u8>>);
    impl Read for BoundedInput {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            assert!(bytes.len() <= 65536);
            self.0.read(bytes)
        }
    }
    let dest = tempfile::tempdir().unwrap();
    let (importer, _, _, media, library) = stores(dest.path());
    let saved = importer
        .import(&mut BoundedInput(Cursor::new(output.bytes)), &library, 0, 7)
        .unwrap();
    let ProfileContent::ScreenMedia(key) = &saved.profiles[0].content else {
        panic!()
    };
    let mut source = media.acquire_source(key).unwrap();
    assert_eq!(source.frames(), 600);
    assert_eq!(source.read_unit(599).unwrap().bytes.len(), 4096);
    assert_eq!(source.name(), "Long portable video");
}

#[test]
fn inactive_basic_editor_dependencies_survive_archive_remap_profile_restart_and_device_toml() {
    let source = tempfile::tempdir().unwrap();
    let (exporter, mut original) = fixture(source.path());
    let ProfileContent::Screen(theme) = &mut original.content else {
        panic!()
    };
    let video = theme.page_cache[0].theme.background_video.clone().unwrap();
    let mut template = theme.clone();
    template.page_cache.clear();
    template.background_video = Some(video);
    template.background_image = None;
    template.version = 2;
    let bank = BasicEditorBank {
        background: BasicBackground::ImageVideo,
        selected_overlay: BasicOverlay::Custom,
        overlays: [
            BasicOverlay::None,
            BasicOverlay::SliderSensor,
            BasicOverlay::DuoSensor,
            BasicOverlay::ThreeSensor,
            BasicOverlay::FourSensor,
            BasicOverlay::Time,
            BasicOverlay::Custom,
        ]
        .into_iter()
        .map(|overlay| BasicOverlayTheme {
            overlay,
            theme: Box::new(template.clone()),
        })
        .collect(),
    };
    theme.page_cache.clear();
    theme.layers.clear();
    theme.background_image = None;
    theme.basic_editor = Some(Box::new(BasicEditorState {
        selected_background: BasicBackground::ImageVideo,
        banks: vec![bank],
    }));
    theme.validate().unwrap();
    assert!(!theme.requires_streaming());
    let before = theme.clone();
    let mut bytes = Vec::new();
    exporter.export(&original, &mut bytes).unwrap();
    let target = tempfile::tempdir().unwrap();
    let (importer, images, fonts, media, library) = stores(target.path());
    let saved = importer
        .import(&mut Cursor::new(bytes), &library, 0, 7)
        .unwrap();
    let ProfileContent::Screen(theme) = &saved.profiles[0].content else {
        panic!()
    };
    assert!(
        theme.layers.is_empty() && theme.background_image.is_none() && !theme.requires_streaming()
    );
    assert_eq!(theme.pages().count(), 8);
    assert!(theme.image_assets().is_disjoint(&before.image_assets()));
    assert!(theme.font_assets().is_disjoint(&before.font_assets()));
    assert!(theme.video_assets().is_disjoint(&before.video_assets()));
    images.load(theme).unwrap();
    fonts.load_theme(theme).unwrap();
    assert_eq!(theme.image_assets().len(), 1);
    assert_eq!(theme.font_assets().len(), 1);
    assert_eq!(theme.video_assets().len(), 1);
    let key = *theme.video_assets().iter().next().unwrap();
    assert_eq!(loaded_clip(&media, key), clip());
    let template = &theme.basic_editor.as_ref().unwrap().banks[0].overlays[0].theme;
    let ThemeLayer::Sensor { sensor_id, .. } = &template.layers[2] else {
        panic!()
    };
    assert_eq!(
        sensor_id,
        &before.basic_editor.as_ref().unwrap().banks[0].overlays[0]
            .theme
            .layers
            .iter()
            .find_map(|layer| match layer {
                ThemeLayer::Sensor { sensor_id, .. } => Some(sensor_id.clone()),
                _ => None,
            })
            .unwrap()
    );
    assert_eq!(
        ProfileStore::new(target.path().join("state"))
            .list()
            .unwrap(),
        saved
    );
    // Device current-theme uses TOML, named slots use JSON. Both must retain caches.
    let settings = lconnect_daemon::persistence::DeviceSettings {
        theme: Some(theme.clone()),
        ..Default::default()
    };
    let toml = toml::to_string(&settings).unwrap();
    let reloaded: lconnect_daemon::persistence::DeviceSettings = toml::from_str(&toml).unwrap();
    assert_eq!(reloaded.theme.as_ref(), Some(theme));
    let selected = theme
        .select_basic_editor(BasicBackground::ImageVideo, BasicOverlay::None)
        .unwrap();
    assert!(
        !selected.requires_streaming(),
        "text selection keeps the active shared background"
    );
    let mut reexported = Vec::new();
    importer
        .export(&saved.profiles[0], &mut reexported)
        .unwrap();
    assert!(reexported.len() > FONT.len());
}

#[test]
fn editable_fractional_dividers_survive_portable_asset_remap_in_active_and_cached_pages() {
    let source = tempfile::tempdir().unwrap();
    let (exporter, mut original) = fixture(source.path());
    let divider:ThemeLayer=serde_json::from_value(json!({"kind":"divider","axis":"vertical","x":240.5,"y":264,"length":166,"thickness":1.5,"color":[71,201,137],"opacity":73,"shadow":{"x":-2,"y":3,"blur":5,"color":[9,8,7],"opacity":35}})).unwrap();
    let ProfileContent::Screen(theme) = &mut original.content else {
        panic!()
    };
    theme.layers.push(divider.clone());
    theme.page_cache[0].theme.layers.push(divider.clone());
    let mut bytes = Vec::new();
    exporter.export(&original, &mut bytes).unwrap();
    let dest = tempfile::tempdir().unwrap();
    let (importer, _, _, _, library) = stores(dest.path());
    let saved = importer
        .import(&mut Cursor::new(bytes), &library, 0, 6)
        .unwrap();
    let ProfileContent::Screen(theme) = &saved.profiles[0].content else {
        panic!()
    };
    assert_eq!(theme.layers.last(), Some(&divider));
    assert_eq!(theme.page_cache[0].theme.layers.last(), Some(&divider));
    theme.validate().unwrap();
}
