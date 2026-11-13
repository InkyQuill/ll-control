use llcontrol_daemon::media_file::{MediaFile, MediaFileWriter};
use llcontrol_ipc::H264AccessUnit;
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    process::Command,
};
fn first() -> H264AccessUnit {
    H264AccessUnit {
        bytes: vec![0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
        keyframe: true,
    }
}
fn next() -> H264AccessUnit {
    H264AccessUnit {
        bytes: vec![0, 0, 1, 0x41, 1],
        keyframe: false,
    }
}
fn fixture(root: &Path) -> std::path::PathBuf {
    let path = root.join("media.lcm");
    let mut writer = MediaFileWriter::new(root, "Test".into()).unwrap();
    writer.append(&first()).unwrap();
    writer.append(&next()).unwrap();
    writer.finish(&path).unwrap();
    path
}
#[test]
fn full_hour_index_survives_reopen_and_rejects_an_extra_frame_without_damage() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("hour.lcm");
    let mut writer = MediaFileWriter::new(root.path(), "One hour".into()).unwrap();
    writer.append(&first()).unwrap();
    for _ in 1..llcontrol_ipc::MAX_MEDIA_UPLOAD_FRAMES {
        writer.append(&next()).unwrap();
    }
    assert_eq!(
        writer.append(&next()).unwrap_err().to_string(),
        "media frame count exceeds resource budget"
    );
    writer.finish(&path).unwrap();
    let mut reader = MediaFile::open(&path).unwrap();
    assert_eq!(reader.fps(), 24);
    assert_eq!(reader.frames(), 24 * 60 * 60);
    assert_eq!(reader.read_unit(0).unwrap(), first());
    for index in [1, reader.frames() / 2, reader.frames() - 1] {
        assert_eq!(reader.read_unit(index).unwrap(), next());
    }
    assert!(reader.read_unit(reader.frames()).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn full_payload_budget_survives_reopen_and_rejects_an_extra_unit_without_damage() {
    // Structural H.264 units exercise storage boundaries; real codec acceptance
    // is covered separately. Do not claim these padded units are decodable video.
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("full.lcm");
    let mut unit = first();
    unit.bytes.resize(128 * 1024, 0xff);
    let frames = llcontrol_ipc::MAX_MEDIA_UPLOAD_BYTES as usize / unit.bytes.len();
    let mut writer = MediaFileWriter::new(root.path(), "Full payload".into()).unwrap();
    for _ in 0..frames {
        writer.append(&unit).unwrap();
    }
    assert_eq!(
        writer.append(&next()).unwrap_err().to_string(),
        "media payload exceeds resource budget"
    );
    writer.finish(&path).unwrap();
    let mut reader = MediaFile::open(&path).unwrap();
    assert_eq!(
        reader.encoded_bytes(),
        llcontrol_ipc::MAX_MEDIA_UPLOAD_BYTES
    );
    assert_eq!(reader.frames(), frames);
    for index in [0, frames / 2, frames - 1] {
        assert_eq!(reader.read_unit(index).unwrap(), unit);
    }
    assert!(reader.read_unit(frames).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
#[test]
fn staged_media_cancels_on_drop_and_never_overwrites() {
    let root = tempfile::tempdir().unwrap();
    {
        let mut writer = MediaFileWriter::new(root.path(), "Cancelled".into()).unwrap();
        writer.append(&first()).unwrap();
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let path = fixture(root.path());
    let before = fs::read(&path).unwrap();
    let mut writer = MediaFileWriter::new(root.path(), "Replacement".into()).unwrap();
    writer.append(&first()).unwrap();
    assert!(writer.finish(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    let mut reader = MediaFile::open(&path).unwrap();
    assert_eq!(reader.name(), "Test");
    assert_eq!(reader.read_unit(1).unwrap(), next());
    assert_eq!(reader.read_unit(0).unwrap(), first());
    assert!(reader.read_unit(2).is_err());
    assert!(MediaFileWriter::new(root.path(), " ".into()).is_err());
    assert!(MediaFileWriter::new(root.path(), "Empty".into())
        .unwrap()
        .finish(&root.path().join("empty"))
        .is_err());
}
fn rewrite(path: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let bytes = fs::read(path).unwrap();
    let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut index: serde_json::Value = serde_json::from_slice(&bytes[12..12 + length]).unwrap();
    edit(&mut index);
    let header = serde_json::to_vec(&index).unwrap();
    let mut file = fs::File::create(path).unwrap();
    file.write_all(b"LCMEDIA2").unwrap();
    file.write_all(&(header.len() as u32).to_le_bytes())
        .unwrap();
    file.write_all(&header).unwrap();
    file.write_all(&bytes[12 + length..]).unwrap();
}
#[test]
fn malformed_index_truncation_and_changed_frame_are_rejected() {
    for mutation in ["version", "gap", "overlap", "length", "keyframe", "unknown"] {
        let root = tempfile::tempdir().unwrap();
        let path = fixture(root.path());
        rewrite(&path, |index| match mutation {
            "version" => index["version"] = 99.into(),
            "gap" => index["units"][1]["offset"] = 100.into(),
            "overlap" => index["units"][1]["offset"] = 0.into(),
            "length" => index["units"][1]["length"] = u32::MAX.into(),
            "keyframe" => index["units"][0]["keyframe"] = false.into(),
            _ => index["unknown"] = true.into(),
        });
        assert!(MediaFile::open(&path).is_err(), "{mutation}");
    }
    let root = tempfile::tempdir().unwrap();
    let path = fixture(root.path());
    let mut reader = MediaFile::open(&path).unwrap();
    let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::End(-5)).unwrap();
    file.write_all(&[9; 5]).unwrap();
    assert!(reader.read_unit(1).is_err());
    file.set_len(12).unwrap();
    assert!(MediaFile::open(&path).is_err());
    fs::write(
        &path,
        [b"LCMEDIA2".as_slice(), &u32::MAX.to_le_bytes()].concat(),
    )
    .unwrap();
    assert!(MediaFile::open(&path).is_err());
    #[cfg(unix)]
    {
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(MediaFile::open(&link).is_err());
    }
}
fn run(command: &mut Command) -> Vec<u8> {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
#[test]
fn vp9_mp4_586_frames_transcodes_and_roundtrips_without_diagnostic_limits() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("source.mp4");
    let h264 = root.path().join("encoded.h264");
    run(Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=480x480:r=24",
            "-frames:v",
            "586",
            "-an",
            "-c:v",
            "libvpx-vp9",
            "-threads",
            "1",
            "-deadline",
            "realtime",
            "-cpu-used",
            "8",
        ])
        .arg(&input));
    run(Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(&input)
        .args([
            "-an",
            "-c:v",
            "libx264",
            "-threads",
            "1",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-profile:v",
            "baseline",
            "-level:v",
            "3",
            "-pix_fmt",
            "yuv420p",
            "-x264-params",
            "keyint=24:min-keyint=24:scenecut=0:aud=1",
            "-f",
            "h264",
        ])
        .arg(&h264));
    let packets: serde_json::Value = serde_json::from_slice(&run(Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "packet=pos,size,flags",
            "-of",
            "json",
        ])
        .arg(&h264)))
    .unwrap();
    let packets = packets["packets"].as_array().unwrap();
    assert_eq!(packets.len(), 586);
    let mut source = fs::File::open(&h264).unwrap();
    let mut writer =
        MediaFileWriter::new(root.path(), "24.416667 seconds VP9 input".into()).unwrap();
    for packet in packets {
        let offset = packet["pos"].as_str().unwrap().parse::<u64>().unwrap();
        let size = packet["size"].as_str().unwrap().parse::<usize>().unwrap();
        source.seek(SeekFrom::Start(offset)).unwrap();
        let mut bytes = vec![0; size];
        source.read_exact(&mut bytes).unwrap();
        writer
            .append(&H264AccessUnit {
                bytes,
                keyframe: packet["flags"].as_str().unwrap().contains('K'),
            })
            .unwrap();
    }
    let path = root.path().join("long.lcm");
    writer.finish(&path).unwrap();
    let mut reader = MediaFile::open(&path).unwrap();
    assert_eq!(reader.frames(), 586);
    assert_eq!(reader.fps(), 24);
    assert!(reader.encoded_bytes() > 1024 * 1024);
    let rebuilt = root.path().join("rebuilt.h264");
    let mut output = fs::File::create(&rebuilt).unwrap();
    for frame in 0..reader.frames() {
        output
            .write_all(&reader.read_unit(frame).unwrap().bytes)
            .unwrap();
    }
    assert_eq!(
        reader.read_unit(0).unwrap().bytes.len(),
        packets[0]["size"]
            .as_str()
            .unwrap()
            .parse::<usize>()
            .unwrap()
    );
    drop(output);
    let decoded = run(Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "csv=p=0",
        ])
        .arg(&rebuilt));
    assert_eq!(String::from_utf8(decoded).unwrap().trim(), "586");
    assert_eq!(
        fs::metadata(&rebuilt).unwrap().len(),
        reader.encoded_bytes()
    );
}
