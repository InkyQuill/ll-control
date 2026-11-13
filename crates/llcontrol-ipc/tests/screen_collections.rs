use llcontrol_ipc::{ScreenBackgroundAsset, ScreenCollections};

#[test]
fn collections_round_trip_shared_assets_and_reject_unsafe_membership() {
    let mut collection = ScreenCollections::defaults(1);
    collection.images.push("shared".into());
    collection.watermarks.push("shared".into());
    collection
        .backgrounds
        .push(ScreenBackgroundAsset::Image("shared".into()));
    collection.validate().unwrap();
    let encoded = serde_json::to_string(&collection).unwrap();
    assert_eq!(
        serde_json::from_str::<ScreenCollections>(&encoded).unwrap(),
        collection
    );
    for key in ["../outside", "", "a/b"] {
        let mut invalid = collection.clone();
        invalid.images.push(key.into());
        assert!(invalid.validate().is_err());
    }
    let mut duplicate = collection.clone();
    duplicate.watermarks.push("shared".into());
    assert!(duplicate.validate().is_err());
    let mut missing_default = collection.clone();
    missing_default.images.remove(0);
    assert!(missing_default.validate().is_err());
    let mut future = collection.clone();
    future.version = 2;
    assert!(future.validate().is_err());
    let mut invalid_video = collection;
    invalid_video
        .backgrounds
        .push(ScreenBackgroundAsset::Video("BAD".into()));
    assert!(invalid_video.validate().is_err());
    assert!(ScreenCollections::defaults(0).validate().is_err());
}
