use lconnect_daemon::profiles::ProfileStore;
use lconnect_ipc::{
    CoolingPolicy, LcdTheme, NamedProfile, ProfileContent, ProfileEdit, ProfileGroup,
};

fn screen(slot: u8) -> ProfileEdit {
    ProfileEdit::Save {
        profile: NamedProfile {
            slot,
            name: "Экран".into(),
            content: ProfileContent::Screen(LcdTheme {
                page_cache: Vec::new(),
                version: 1,
                name: "Clock".into(),
                background: [0, 0, 0],
                background_image: None,
                background_video: None,
                background_fill: None,
                layers: vec![],
            }),
        },
    }
}

#[test]
fn restart_rename_delete_preserve_other_groups_and_reject_stale_edit() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().into());
    store.edit(0, screen(7)).unwrap();
    let fan = ProfileEdit::Save {
        profile: NamedProfile {
            slot: 0,
            name: "Quiet".into(),
            content: ProfileContent::Fan(CoolingPolicy::Fixed { percent: 40 }),
        },
    };
    let saved = store.edit(1, fan).unwrap();
    assert_eq!(ProfileStore::new(dir.path().into()).list().unwrap(), saved);
    assert!(store.edit(1, screen(0)).is_err());
    let renamed = store
        .edit(
            2,
            ProfileEdit::Rename {
                group: ProfileGroup::Screen,
                slot: 7,
                name: "Ночь".into(),
            },
        )
        .unwrap();
    assert_eq!(renamed.profiles[0].name, "Ночь");
    let deleted = store
        .edit(
            3,
            ProfileEdit::Delete {
                group: ProfileGroup::Screen,
                slot: 7,
            },
        )
        .unwrap();
    assert_eq!(deleted.profiles, vec![saved.profiles[1].clone()]);
    assert!(store.edit(4, screen(8)).is_err());
    assert_eq!(store.list().unwrap(), deleted);
}

#[test]
fn corrupt_and_future_schema_are_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().into());
    let path = dir.path().join("profiles.json");
    for input in ["broken", r#"{"version":2,"revision":0,"profiles":[]}"#] {
        std::fs::write(&path, input).unwrap();
        assert!(store.list().is_err());
        assert!(store.edit(0, screen(0)).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), input);
    }
}

#[test]
fn concurrent_editors_cannot_lose_an_update() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().into());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|slot| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.edit(0, screen(slot))
            })
        })
        .collect();
    let successes = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(Result::is_ok)
        .count();
    assert_eq!(successes, 1);
    let saved = store.list().unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.profiles.len(), 1);
}

#[test]
fn reset_restores_reference_curve_and_preserves_other_slots() {
    use lconnect_ipc::{FanPreset, NamedProfile};
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().into());
    let initial = store.edit(0, screen(7)).unwrap();
    assert!(store
        .edit(
            1,
            ProfileEdit::Reset {
                group: ProfileGroup::Fan,
                slot: 0,
                sensor_id: None
            }
        )
        .is_err());
    assert_eq!(store.list().unwrap(), initial);
    let reset = store
        .edit(
            1,
            ProfileEdit::Reset {
                group: ProfileGroup::Fan,
                slot: 0,
                sensor_id: Some("cpu:explicit".into()),
            },
        )
        .unwrap();
    assert_eq!(reset.profiles[0], initial.profiles[0]);
    assert_eq!(
        reset.profiles[1].content,
        ProfileContent::Fan(FanPreset::Quiet.policy("cpu:explicit".into(), 2.0, 100))
    );
    let screen_reset = store
        .edit(
            2,
            ProfileEdit::Reset {
                group: ProfileGroup::Screen,
                slot: 7,
                sensor_id: None,
            },
        )
        .unwrap();
    assert_eq!(
        screen_reset.profiles[0],
        NamedProfile::defaults(ProfileGroup::Screen, 7, None).unwrap()
    );
    assert_eq!(screen_reset.profiles[1], reset.profiles[1]);
    assert_eq!(
        ProfileStore::new(dir.path().into()).list().unwrap(),
        screen_reset
    );
}

#[test]
fn media_and_theme_share_eight_screen_slots_and_reset_removes_media_reference() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().into());
    let media = NamedProfile {
        slot: 7,
        name: "Cinema".into(),
        content: ProfileContent::ScreenMedia("a".repeat(32)),
    };
    let saved = store
        .edit(
            0,
            ProfileEdit::Save {
                profile: media.clone(),
            },
        )
        .unwrap();
    assert_eq!(saved.profiles[0], media);
    assert_eq!(ProfileStore::new(dir.path().into()).list().unwrap(), saved);
    let mut duplicate = saved.clone();
    duplicate
        .profiles
        .push(NamedProfile::defaults(ProfileGroup::Screen, 7, None).unwrap());
    assert!(duplicate.validate().is_err());
    let replaced = store.edit(1, screen(7)).unwrap();
    assert_eq!(replaced.profiles.len(), 1);
    assert!(matches!(
        replaced.profiles[0].content,
        ProfileContent::Screen(_)
    ));
    for key in ["../bad".into(), "A".repeat(32), "a".repeat(31)] {
        assert!(ProfileContent::ScreenMedia(key).validate().is_err());
    }
    let mut outside = media;
    outside.slot = 8;
    assert!(outside.validate().is_err());
}

#[test]
fn group_reset_replaces_all_slots_once_preserves_neighbors_and_user_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().into());
    let mut library = store.edit(0, screen(7)).unwrap();
    for group in [ProfileGroup::Fan, ProfileGroup::Pump] {
        library = store
            .edit(
                library.revision,
                ProfileEdit::Reset {
                    group,
                    slot: 3,
                    sensor_id: Some("cpu:explicit".into()),
                },
            )
            .unwrap();
    }
    let neighbors: Vec<_> = library
        .profiles
        .iter()
        .filter(|p| p.content.group() != ProfileGroup::Screen)
        .cloned()
        .collect();
    let asset = dir.path().join("user-media.bin");
    std::fs::write(&asset, b"retained user content").unwrap();
    let revision = library.revision;
    let reset = store
        .edit(
            revision,
            ProfileEdit::ResetGroup {
                group: ProfileGroup::Screen,
                sensor_id: None,
            },
        )
        .unwrap();
    assert_eq!(reset.revision, revision + 1);
    assert_eq!(&reset.profiles[..neighbors.len()], &neighbors);
    let defaults: Vec<_> = (0..8)
        .map(|slot| NamedProfile::defaults(ProfileGroup::Screen, slot, None).unwrap())
        .collect();
    assert_eq!(&reset.profiles[neighbors.len()..], &defaults);
    assert_eq!(std::fs::read(&asset).unwrap(), b"retained user content");
    let reopened = ProfileStore::new(dir.path().into());
    assert_eq!(reopened.list().unwrap(), reset);
    // A lost reply cannot turn a retry into a second mutation.
    assert!(reopened
        .edit(
            revision,
            ProfileEdit::ResetGroup {
                group: ProfileGroup::Screen,
                sensor_id: None,
            }
        )
        .is_err());
    assert_eq!(reopened.list().unwrap(), reset);
}

#[test]
fn group_reset_validates_binding_before_mutation_and_resets_four_cooling_slots() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().into());
    let before = store.edit(0, screen(7)).unwrap();
    for (group, sensor_id) in [
        (ProfileGroup::Fan, None),
        (ProfileGroup::Pump, Some(" ".into())),
        (ProfileGroup::Screen, Some("cpu:explicit".into())),
    ] {
        assert!(store
            .edit(
                before.revision,
                ProfileEdit::ResetGroup { group, sensor_id }
            )
            .is_err());
        assert_eq!(store.list().unwrap(), before);
    }
    let mut library = before.clone();
    for group in [ProfileGroup::Fan, ProfileGroup::Pump] {
        library = store
            .edit(
                library.revision,
                ProfileEdit::ResetGroup {
                    group,
                    sensor_id: Some("cpu:explicit".into()),
                },
            )
            .unwrap();
        let actual: Vec<_> = library
            .profiles
            .iter()
            .filter(|p| p.content.group() == group)
            .cloned()
            .collect();
        let expected: Vec<_> = (0..4)
            .map(|slot| NamedProfile::defaults(group, slot, Some("cpu:explicit")).unwrap())
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(library.profiles[0], before.profiles[0]);
    }
}
