use ab_glyph::Font;
use llcontrol_image::fonts::{validate_font, FontAssetError, MAX_FONT_BYTES};

#[test]
fn accepts_complete_latin_and_full_tc_fonts_without_altering_bytes() {
    for data in [
        include_bytes!("../assets/DejaVuSans.ttf").as_slice(),
        include_bytes!("../assets/NotoSansTC-Regular.otf").as_slice(),
        include_bytes!("../assets/NotoSansTC-Bold.otf").as_slice(),
    ] {
        let font = validate_font(data).unwrap();
        assert_eq!(font.font_data(), data);
        assert_ne!(font.glyph_id('A').0, 0);
    }
}

#[test]
fn rejects_oversize_collections_webfonts_and_broken_sfnt_tables() {
    assert!(matches!(validate_font(&[]), Err(FontAssetError::Size)));
    assert!(matches!(
        validate_font(&vec![0; MAX_FONT_BYTES + 1]),
        Err(FontAssetError::Size)
    ));
    for tag in [b"ttcf", b"wOFF", b"wOF2", b"nope"] {
        assert!(matches!(validate_font(tag), Err(FontAssetError::Container)));
    }
    let data = include_bytes!("../assets/DejaVuSans.ttf");
    for length in [4, 12, 20, 100] {
        assert!(validate_font(&data[..length]).is_err());
    }
    let mut corrupted = data.to_vec();
    let count = u16::from_be_bytes([data[4], data[5]]) as usize;
    let hhea = (0..count)
        .map(|i| 12 + i * 16)
        .find(|i| &data[*i..*i + 4] == b"hhea")
        .unwrap();
    corrupted[hhea + 8..hhea + 12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(validate_font(&corrupted).is_err());
}
