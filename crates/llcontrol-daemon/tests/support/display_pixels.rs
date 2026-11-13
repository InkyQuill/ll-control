//! Compare pixels after both lossy codecs using reduced grayscale images.
use llcontrol_protocol::galahad2_lcdv2::cpacket::CCommandPacket;
use std::process::Command;

pub fn decode_packets(writes: &[Vec<u8>]) -> Vec<Vec<u8>> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut unit = Vec::new();
    for raw in writes.iter().filter(|p| p.starts_with(&[3, 13])) {
        let p = CCommandPacket::from_response(raw, 13).unwrap();
        if p.packet_number() == 0 {
            unit.clear();
        }
        unit.extend_from_slice(p.data());
        if unit.len() == p.data_size() as usize {
            file.write_all(&unit).unwrap();
        }
    }
    let decoded = Command::new("ffmpeg")
        .args(["-v", "error", "-threads", "1", "-f", "h264", "-i"])
        .arg(file.path())
        .args([
            "-filter_threads",
            "1",
            "-vf",
            "scale=120:120:flags=area",
            "-pix_fmt",
            "gray",
            "-threads",
            "1",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    decoded
        .stdout
        .chunks_exact(120 * 120)
        .map(<[u8]>::to_vec)
        .collect()
}
pub fn preview_pixels(jpeg: &[u8]) -> Vec<u8> {
    llcontrol_image::assets::decode_asset(jpeg)
        .unwrap()
        .thumbnail_exact(120, 120)
        .into_luma8()
        .into_raw()
}
pub fn distance(actual: &[u8], expected: &[u8]) -> u64 {
    assert_eq!(actual.len(), expected.len());
    actual
        .iter()
        .zip(expected)
        .map(|(a, b)| {
            let d = i64::from(*a) - i64::from(*b);
            (d * d) as u64
        })
        .sum()
}
