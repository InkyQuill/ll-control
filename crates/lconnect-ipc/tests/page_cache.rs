use lconnect_ipc::*;
fn basic() -> LcdTheme {
    serde_json::from_value(
        serde_json::json!({"version":1,"name":"Basic","background":[12,18,30],"layers":[]}),
    )
    .unwrap()
}
fn weather(kind: &str, video: Option<&str>) -> LcdTheme {
    let mut theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":kind,"background":[0,0,0],"layers":[{"kind":"weather","effect":{"kind":kind,"intensity":1,"dynamic":false,"sensor":null,"seed":42}}]})).unwrap();
    theme.background_video = video.map(str::to_owned);
    theme
}
#[test]
fn pages_preserve_full_background_overlay_references_without_activating_inactive_scenes() {
    let mut root = basic();
    let mut rain = weather("rain", Some(&"a".repeat(32)));
    rain.layers.push(serde_json::from_value(serde_json::json!({"kind":"text","text":"Rain","style":{"x":240,"y":80,"size":24,"color":[255,0,0],"align":"center","font_asset":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}})).unwrap());
    let mut snow = weather("snow", None);
    snow.background_image = Some(BackgroundImage {
        asset: "builtin-midnight-v1".into(),
        fit: ImageFit::Crop,
    });
    root.page_cache = vec![
        SavedLcdPage {
            page: LcdPageKind::Rain,
            theme: Box::new(rain),
        },
        SavedLcdPage {
            page: LcdPageKind::Snow,
            theme: Box::new(snow),
        },
    ];
    root.validate().unwrap();
    assert!(!root.requires_streaming());
    assert_eq!(root.page_kind(), LcdPageKind::Basic);
    assert_eq!(
        root.video_assets().into_iter().collect::<Vec<_>>(),
        vec!["a".repeat(32)]
    );
    assert!(root
        .font_assets()
        .contains("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
    assert!(root.image_assets().contains("builtin-midnight-v1"));
    let profile = ProfileContent::Screen(root.clone());
    profile.validate().unwrap();
    assert_eq!(
        serde_json::from_slice::<ProfileContent>(&serde_json::to_vec(&profile).unwrap()).unwrap(),
        profile
    );
    let old = basic();
    assert!(old.page_cache.is_empty());
    assert!(!serde_json::to_value(old)
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("page_cache"));
}
#[test]
fn nested_mismatched_duplicate_and_oversized_caches_are_rejected() {
    let mut root = basic();
    root.page_cache.push(SavedLcdPage {
        page: LcdPageKind::Rain,
        theme: Box::new(weather("rain", None)),
    });
    root.validate().unwrap();
    root.page_cache[0].theme.page_cache.push(SavedLcdPage {
        page: LcdPageKind::Basic,
        theme: Box::new(basic()),
    });
    assert!(root.validate().is_err());
    root.page_cache[0].theme.page_cache.clear();
    root.page_cache[0].page = LcdPageKind::Snow;
    assert!(root.validate().is_err());
    root.page_cache[0].page = LcdPageKind::Rain;
    root.page_cache.push(root.page_cache[0].clone());
    assert!(root.validate().is_err());
    root.page_cache.push(root.page_cache[0].clone());
    assert!(root.validate().is_err());
    root.page_cache.push(root.page_cache[0].clone());
    root.page_cache.push(root.page_cache[0].clone());
    root.page_cache.push(root.page_cache[0].clone());
    assert!(root
        .validate()
        .unwrap_err()
        .to_string()
        .contains("at most five"));
}
