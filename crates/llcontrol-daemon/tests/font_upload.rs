use llcontrol_daemon::{font_upload::FontUploads, theme_fonts::ThemeFontStore};
use llcontrol_ipc::{FontUploadRequest as Request, FONT_CHUNK_BYTES, MAX_FONT_BYTES};
const FONT: &[u8] = include_bytes!("../../llcontrol-image/assets/DejaVuSans.ttf");

#[test]
fn chunk_retry_offsets_finish_retry_and_cancel_preserve_one_asset() {
    let dir = tempfile::tempdir().unwrap();
    let store = ThemeFontStore::new(dir.path().join("fonts"));
    let uploads = FontUploads::new(store.clone());
    let id = uploads
        .handle(Request::Begin { size: FONT.len() })
        .unwrap()
        .id;
    assert!(uploads.handle(Request::Finish { id: id.clone() }).is_err());
    assert!(uploads
        .handle(Request::Chunk {
            id: id.clone(),
            offset: 1,
            bytes: vec![0]
        })
        .is_err());
    for (i, chunk) in FONT.chunks(FONT_CHUNK_BYTES).enumerate() {
        let request = Request::Chunk {
            id: id.clone(),
            offset: i * FONT_CHUNK_BYTES,
            bytes: chunk.to_vec(),
        };
        let first = uploads.handle(request.clone()).unwrap();
        let retry = uploads.handle(request).unwrap();
        assert_eq!(first.received, retry.received);
    }
    assert!(uploads
        .handle(Request::Chunk {
            id: id.clone(),
            offset: 0,
            bytes: vec![255]
        })
        .is_err());
    let first = uploads.handle(Request::Finish { id: id.clone() }).unwrap();
    let retry = uploads.handle(Request::Finish { id: id.clone() }).unwrap();
    assert_eq!(first.asset, retry.asset);
    assert_eq!(store.load(first.asset.as_ref().unwrap()).unwrap(), FONT);
    assert_eq!(
        std::fs::read_dir(dir.path().join("fonts")).unwrap().count(),
        1
    );
    assert!(
        uploads
            .handle(Request::Cancel { id: id.clone() })
            .unwrap()
            .cancelled
    );
    assert!(uploads.handle(Request::Status { id }).is_err());
    assert_eq!(store.load(first.asset.as_ref().unwrap()).unwrap(), FONT);
}

#[test]
fn bounded_sessions_bad_chunks_and_invalid_fonts_do_not_create_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("fonts");
    let uploads = FontUploads::new(ThemeFontStore::new(root.clone()));
    for size in [0, MAX_FONT_BYTES + 1] {
        assert!(uploads.handle(Request::Begin { size }).is_err());
    }
    let ids: Vec<_> = (0..4)
        .map(|_| {
            uploads
                .handle(Request::Begin {
                    size: MAX_FONT_BYTES,
                })
                .unwrap()
                .id
        })
        .collect();
    assert!(uploads.handle(Request::Begin { size: 1 }).is_err());
    for bytes in [vec![], vec![0; FONT_CHUNK_BYTES + 1]] {
        assert!(uploads
            .handle(Request::Chunk {
                id: ids[0].clone(),
                offset: 0,
                bytes
            })
            .is_err());
    }
    assert!(uploads
        .handle(Request::Chunk {
            id: ids[0].clone(),
            offset: usize::MAX,
            bytes: vec![0]
        })
        .is_err());
    for id in ids {
        uploads.handle(Request::Cancel { id }).unwrap();
    }
    let id = uploads.handle(Request::Begin { size: 4 }).unwrap().id;
    uploads
        .handle(Request::Chunk {
            id: id.clone(),
            offset: 0,
            bytes: b"nope".to_vec(),
        })
        .unwrap();
    assert!(uploads.handle(Request::Finish { id }).is_err());
    assert!(!root.exists());
}
