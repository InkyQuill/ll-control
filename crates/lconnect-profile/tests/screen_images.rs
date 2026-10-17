#[path = "support/screen_images.rs"]
mod fixture;
use lconnect_profile::{
    screen::inspect_windows_screen,
    screen_assets::{prepare_images, read_verified},
};
use std::{collections::BTreeSet, io::Cursor};
const PNG: &[u8] = include_bytes!("fixtures/ScreenImage.png");
#[test]
fn verified_images_normalize_with_alpha_without_publishing_or_exposing_bytes() {
    let zip = fixture::archive(PNG, "wmsq/unused.png", PNG);
    let source = inspect_windows_screen(Cursor::new(&zip), 0).unwrap();
    let images = prepare_images(Cursor::new(&zip), &source).unwrap();
    assert_eq!(images.images.len(), 2);
    assert!(images.deferred_entries.is_empty());
    assert!(!images.assets_published && !images.complete_screen_group_supported);
    assert!(images.images_decoded && images.source_payloads_verified);
    for image in &images.images {
        let decoded = lconnect_image::assets::decode_asset(&image.png)
            .unwrap()
            .into_rgba8();
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [200, 20, 30, 128]);
        assert_eq!(decoded.get_pixel(1, 0).0, [10, 220, 30, 255]);
        assert_eq!(image.normalized_bytes, image.png.len());
        assert_eq!(
            image.source_sha256,
            source
                .payloads
                .iter()
                .find(|p| p.entry == image.source_entry)
                .unwrap()
                .sha256
        );
    }
    let json = serde_json::to_value(images).unwrap();
    assert!(json["images"][0].get("png").is_none());
}
#[test]
fn source_change_unknown_entry_and_encoding_budget_fail_before_normalization() {
    let zip = fixture::archive(PNG, "wmsq/unused.bin", b"owned");
    let source = inspect_windows_screen(Cursor::new(&zip), 0).unwrap();
    let mut changed = PNG.to_vec();
    changed[40] ^= 1;
    let altered = fixture::archive(&changed, "wmsq/unused.bin", b"owned");
    assert!(prepare_images(Cursor::new(altered), &source).is_err());
    let names = BTreeSet::from(["wmsq/art.png".to_owned()]);
    assert!(read_verified(Cursor::new(&zip), &source, &names, 4, |_, _| Ok(())).is_err());
    let names = BTreeSet::from(["wmsq/not-inspected.png".to_owned()]);
    assert!(read_verified(Cursor::new(&zip), &source, &names, 1024, |_, _| Ok(())).is_err());
}
#[test]
fn animation_is_deferred_and_disguised_gif_is_rejected_instead_of_becoming_first_frame() {
    let zip = fixture::archive(PNG, "wmsq/motion.gif", b"GIF89a");
    let source = inspect_windows_screen(Cursor::new(&zip), 0).unwrap();
    let images = prepare_images(Cursor::new(&zip), &source).unwrap();
    assert_eq!(images.images.len(), 1);
    assert_eq!(images.deferred_entries, vec!["wmsq/motion.gif"]);
    let zip = fixture::archive(b"GIF89a", "wmsq/other.bin", b"owned");
    let source = inspect_windows_screen(Cursor::new(&zip), 0).unwrap();
    assert!(prepare_images(Cursor::new(&zip), &source).is_err());
    let mut apng = PNG[..33].to_vec();
    apng.extend_from_slice(&[0, 0, 0, 8]);
    apng.extend_from_slice(b"acTL");
    apng.extend_from_slice(&[0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0]);
    apng.extend_from_slice(&PNG[33..]);
    let zip = fixture::archive(&apng, "wmsq/other.bin", b"owned");
    let source = inspect_windows_screen(Cursor::new(&zip), 0).unwrap();
    assert!(prepare_images(Cursor::new(&zip), &source).is_err());
}
#[test]
fn jpeg_payload_uses_the_same_verified_pipeline_and_becomes_static_png() {
    let jpeg = lconnect_image::prepare_jpeg_from_bytes(
        PNG,
        lconnect_image::FitMode::Stretch,
        lconnect_image::Quality::default(),
    )
    .unwrap();
    let zip = fixture::archive(PNG, "wmsq/photo.JPEG", &jpeg.bytes);
    let source = inspect_windows_screen(Cursor::new(&zip), 0).unwrap();
    let prepared = prepare_images(Cursor::new(&zip), &source).unwrap();
    let photo = prepared
        .images
        .iter()
        .find(|image| image.source_entry == "wmsq/photo.JPEG")
        .unwrap();
    assert_eq!((photo.width, photo.height), (480, 480));
    assert!(photo.png.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = lconnect_image::assets::decode_asset(&photo.png)
        .unwrap()
        .into_rgba8();
    assert_eq!(decoded.get_pixel(240, 240)[3], 255);
}
