use lconnect_ipc::*;
use serde_json::json;
fn basic() -> LcdTheme {
    serde_json::from_value(json!({"version":1,"name":"Basic","background":[1,2,3],"layers":[]}))
        .unwrap()
}
fn state() -> BasicEditorState {
    BasicEditorState {
        selected_background: BasicBackground::ImageVideo,
        banks: [
            BasicBackground::ImageVideo,
            BasicBackground::BackgroundColor,
        ]
        .into_iter()
        .map(|background| BasicEditorBank {
            background,
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
                theme: Box::new(basic()),
            })
            .collect(),
        })
        .collect(),
    }
}
#[test]
fn inactive_templates_hold_resource_dependencies_without_affecting_active_rendering_and_switch_preserves_edits(
) {
    let mut root = basic();
    let mut cache = state();
    let inactive = &mut cache.banks[0].overlays[0].theme;
    inactive.version = 2;
    inactive.background_video = Some("a".repeat(32));
    inactive.layers.push(serde_json::from_value(json!({"kind":"text","text":"Inactive","style":{"font_asset":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","x":10,"y":10,"size":30,"color":[1,2,3],"align":"left"}})).unwrap());
    cache.banks[1].overlays[0].theme.background_image = Some(BackgroundImage {
        asset: BUILTIN_IMAGE_KEY.into(),
        fit: ImageFit::Stretch,
    });
    root.basic_editor = Some(Box::new(cache));
    root.validate().unwrap();
    assert!(!root.requires_streaming());
    assert!(root.layers.is_empty());
    assert_eq!(root.pages().count(), 15);
    assert!(root.video_assets().contains("a".repeat(32).as_str()));
    assert!(root
        .font_assets()
        .contains("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
    assert!(root.image_assets().contains(BUILTIN_IMAGE_KEY));
    root.background = [91, 92, 93];
    let next = root
        .select_basic_editor(BasicBackground::BackgroundColor, BasicOverlay::Time)
        .unwrap();
    assert_eq!(next.background, [1, 2, 3]);
    let restored = next
        .select_basic_editor(BasicBackground::ImageVideo, BasicOverlay::Custom)
        .unwrap();
    assert_eq!(restored.background, [91, 92, 93]);
    assert_eq!(
        serde_json::from_slice::<LcdTheme>(&serde_json::to_vec(&restored).unwrap()).unwrap(),
        restored
    );
    assert!(serde_json::to_value(basic())
        .unwrap()
        .get("basic_editor")
        .is_none());
}
#[test]
fn incomplete_duplicate_nested_and_nonbasic_templates_never_enter_storage() {
    let mut root = basic();
    root.basic_editor = Some(Box::new(state()));
    root.basic_editor.as_mut().unwrap().banks[0].overlays.pop();
    assert!(root.validate().is_err());
    root.basic_editor = Some(Box::new(state()));
    root.basic_editor.as_mut().unwrap().banks[1].background = BasicBackground::ImageVideo;
    assert!(root.validate().is_err());
    root.basic_editor = Some(Box::new(state()));
    let duplicate = root.basic_editor.as_ref().unwrap().banks[0].overlays[0].clone();
    root.basic_editor.as_mut().unwrap().banks[0].overlays[1] = duplicate;
    assert!(root.validate().is_err());
    root.basic_editor = Some(Box::new(state()));
    root.basic_editor.as_mut().unwrap().banks[0].overlays[0]
        .theme
        .basic_editor = Some(Box::new(state()));
    assert!(root.validate().is_err());
    root.basic_editor = Some(Box::new(state()));
    root.basic_editor.as_mut().unwrap().banks[0].overlays[0]
        .theme
        .page_cache
        .push(SavedLcdPage {
            page: LcdPageKind::Basic,
            theme: Box::new(basic()),
        });
    assert!(root.validate().is_err());
    root.basic_editor = Some(Box::new(state()));
    root.basic_editor.as_mut().unwrap().banks[0].overlays[0]
        .theme
        .layers
        .push(ThemeLayer::Snow3d {
            editor: None,
            effect: Snow3dEffect {
                intensity: 1,
                seed: 42,
            },
        });
    assert!(root.validate().is_err());
}

#[test]
fn text_selection_keeps_shared_background_watermark_and_name_but_background_editors_stay_independent(
) {
    let mut root = basic();
    root.basic_editor = Some(Box::new(state()));
    root.name = "Authored name".into();
    root.background = [91, 92, 93];
    let mark:ThemeLayer=serde_json::from_value(json!({"kind":"image","asset":BUILTIN_MARK_KEY,"x":0,"y":0,"width":480,"height":480,"fit":"contain","opacity":90,"watermark_position":{"horizontal":75,"vertical":50}})).unwrap();
    let text:ThemeLayer=serde_json::from_value(json!({"kind":"text","text":"Authored custom","style":{"x":240,"y":100,"size":40,"color":[1,2,3],"align":"center"}})).unwrap();
    root.layers = vec![text, mark.clone()];
    let clock = root
        .select_basic_editor(BasicBackground::ImageVideo, BasicOverlay::Time)
        .unwrap();
    assert_eq!(clock.background, [91, 92, 93]);
    assert_eq!(clock.name, "Authored name");
    assert_eq!(clock.layers, vec![mark.clone()]);
    let color = clock
        .select_basic_editor(BasicBackground::BackgroundColor, BasicOverlay::Custom)
        .unwrap();
    assert_eq!(color.background, [1, 2, 3]);
    assert!(color.layers.is_empty());
    assert_eq!(color.name, "Authored name");
    let restored = color
        .select_basic_editor(BasicBackground::ImageVideo, BasicOverlay::Custom)
        .unwrap();
    assert_eq!(restored.background, [91, 92, 93]);
    assert_eq!(restored.layers, root.layers);
    let mut removed = restored
        .select_basic_editor(BasicBackground::ImageVideo, BasicOverlay::Time)
        .unwrap();
    removed.layers.clear();
    let custom = removed
        .select_basic_editor(BasicBackground::ImageVideo, BasicOverlay::Custom)
        .unwrap();
    assert_eq!(custom.layers.len(), 1);
    assert!(matches!(&custom.layers[0],ThemeLayer::Text{text,..} if text=="Authored custom"));
}
