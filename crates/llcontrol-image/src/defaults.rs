//! Original, immutable LCD images bundled with the application.
//! Versioned keys never alias user imports (which use 32 hexadecimal digits).

/// A bundled image, available without filesystem bootstrap or writes.
pub struct BuiltinImage {
    pub key: &'static str,
    pub name: &'static str,
    pub contexts: &'static [llcontrol_ipc::ImageGalleryContext],
    pub png: &'static [u8],
}

/// Stable order: mark first, background second. Existing keys must never be reused
/// for different artwork: saved themes and profiles refer to them across upgrades.
pub const IMAGES: &[BuiltinImage] = &[
    BuiltinImage {
        key: llcontrol_ipc::BUILTIN_MARK_KEY,
        name: "Орбита",
        contexts: &[
            llcontrol_ipc::ImageGalleryContext::Image,
            llcontrol_ipc::ImageGalleryContext::Background,
            llcontrol_ipc::ImageGalleryContext::Watermark,
        ],
        png: include_bytes!("../assets/defaults/orbit-mark.png"),
    },
    BuiltinImage {
        key: llcontrol_ipc::BUILTIN_IMAGE_KEY,
        name: "Полночь",
        contexts: &[
            llcontrol_ipc::ImageGalleryContext::Image,
            llcontrol_ipc::ImageGalleryContext::Background,
        ],
        png: include_bytes!("../assets/defaults/midnight.png"),
    },
];

pub fn image(key: &str) -> Option<&'static BuiltinImage> {
    IMAGES.iter().find(|asset| asset.key == key)
}
