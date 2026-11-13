//! Optional observation of completed writes to MockBackend, never real HID.
use llcontrol_protocol::galahad2_lcdv2::{bpacket::BCommandPacket, jpeg::MAX_JPEG_BYTES};
use std::path::Path;

#[derive(Default)]
pub struct Frames {
    cursor: usize,
    sequence: u64,
    pending: Option<(usize, u32, Vec<u8>)>,
}

impl Frames {
    fn packet(&mut self, bytes: &[u8]) -> Option<Vec<u8>> {
        if bytes.get(..2) != Some(&[2, 14]) {
            return None;
        }
        let Ok(packet) = BCommandPacket::from_response(bytes, 14) else {
            self.pending = None;
            return None;
        };
        let total = packet.data_size() as usize;
        if total == 0 || total > MAX_JPEG_BYTES {
            self.pending = None;
            return None;
        }
        if packet.packet_number() == 0 {
            self.pending = Some((total, 0, Vec::with_capacity(total)));
        }
        let (size, next, data) = self.pending.as_mut()?;
        if *size != total || *next != packet.packet_number() || data.len() + packet.length() > total
        {
            self.pending = None;
            return None;
        }
        data.extend_from_slice(packet.data());
        *next += 1;
        if data.len() != total {
            return None;
        }
        let (_, _, data) = self.pending.take()?;
        if !data.starts_with(&[255, 216]) || !data.ends_with(&[255, 217]) {
            return None;
        }
        Some(data)
    }

    pub fn observe(&mut self, history: &[Vec<u8>], root: &Path) -> anyhow::Result<()> {
        let mut latest = None;
        for packet in &history[self.cursor..] {
            if let Some(jpeg) = self.packet(packet) {
                self.sequence += 1;
                latest = Some(jpeg);
            }
        }
        self.cursor = history.len();
        if let Some(jpeg) = latest {
            // One atomic receipt binds sequence and bytes; readers cannot combine
            // a new counter with an old file during continuous rendering.
            let receipt = serde_json::json!({"sequence":self.sequence,"jpeg":jpeg});
            std::fs::write(root.join("lcd-frame.tmp"), serde_json::to_vec(&receipt)?)?;
            std::fs::rename(root.join("lcd-frame.tmp"), root.join("lcd-frame.json"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn jpeg() -> Vec<u8> {
        let mut data = vec![32; 2048];
        data[..2].copy_from_slice(&[255, 216]);
        data[2046..].copy_from_slice(&[255, 217]);
        data
    }
    fn packets(data: &[u8]) -> Vec<Vec<u8>> {
        BCommandPacket::build_packets_for_write(14, data)
            .map(|p| p.as_bytes().to_vec())
            .collect()
    }
    #[test]
    fn exports_only_complete_frames_and_does_not_recount_history() {
        let root = tempfile::tempdir().unwrap();
        let data = jpeg();
        let packets = packets(&data);
        let mut frames = Frames::default();
        frames.observe(&packets[..2], root.path()).unwrap();
        assert!(!root.path().join("lcd-frame.json").exists());
        frames.observe(&packets, root.path()).unwrap();
        frames.observe(&packets, root.path()).unwrap();
        let receipt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.path().join("lcd-frame.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["sequence"], 1);
        assert_eq!(receipt["jpeg"], serde_json::json!(data));
    }
    #[test]
    fn reordered_or_incomplete_frame_cannot_replace_a_complete_receipt() {
        let root = tempfile::tempdir().unwrap();
        let packets = packets(&jpeg());
        let mut history = packets.clone();
        let mut frames = Frames::default();
        frames.observe(&history, root.path()).unwrap();
        let before = std::fs::read(root.path().join("lcd-frame.json")).unwrap();
        history.extend([packets[0].clone(), packets[2].clone(), packets[1].clone()]);
        frames.observe(&history, root.path()).unwrap();
        assert_eq!(
            std::fs::read(root.path().join("lcd-frame.json")).unwrap(),
            before
        );
    }
}

/// Bounded observation of actual completed H.264 writes for isolated acceptance.
/// The saved stream is decoded by the harness; packet counts alone are weaker.
#[derive(Default)]
pub struct Video {
    cursor: usize,
    pending: Option<(u8, usize, u32, Vec<u8>)>,
    frames: u64,
    bytes: usize,
}
impl Video {
    fn packet(&mut self, bytes: &[u8]) -> Option<Vec<u8>> {
        use llcontrol_protocol::galahad2_lcdv2::{
            cpacket::CCommandPacket, h264::MAX_H264_FRAME_BYTES,
        };
        if bytes.get(1) != Some(&13) {
            return None;
        }
        let parsed = match bytes.first() {
            Some(2) => BCommandPacket::from_response(bytes, 13).map(|p| {
                (
                    2,
                    p.data_size() as usize,
                    p.packet_number(),
                    p.data().to_vec(),
                )
            }),
            Some(3) => CCommandPacket::from_response(bytes, 13).map(|p| {
                (
                    3,
                    p.data_size() as usize,
                    p.packet_number(),
                    p.data().to_vec(),
                )
            }),
            _ => {
                self.pending = None;
                return None;
            }
        };
        let Ok((report, total, number, payload)) = parsed else {
            self.pending = None;
            return None;
        };
        if total == 0 || total > MAX_H264_FRAME_BYTES {
            self.pending = None;
            return None;
        }
        if number == 0 {
            self.pending = Some((report, total, 0, Vec::with_capacity(total)));
        }
        let (expected_report, size, next, data) = self.pending.as_mut()?;
        if *expected_report != report
            || *size != total
            || *next != number
            || data.len() + payload.len() > total
        {
            self.pending = None;
            return None;
        }
        data.extend_from_slice(&payload);
        *next += 1;
        if data.len() != total {
            return None;
        }
        Some(self.pending.take()?.3)
    }
    pub fn observe(&mut self, history: &[Vec<u8>], root: &Path) -> anyhow::Result<()> {
        use std::io::Write;
        let mut completed = Vec::new();
        for packet in &history[self.cursor..] {
            if let Some(unit) = self.packet(packet) {
                anyhow::ensure!(
                    self.bytes + completed.len() + unit.len() <= 8 * 1024 * 1024,
                    "mock video observation budget exceeded"
                );
                completed.extend(unit);
                self.frames += 1;
            }
        }
        self.cursor = history.len();
        if completed.is_empty() {
            return Ok(());
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("lcd-video.h264"))?
            .write_all(&completed)?;
        self.bytes += completed.len();
        let receipt = serde_json::json!({"frames":self.frames,"bytes":self.bytes});
        std::fs::write(root.join("lcd-video.tmp"), serde_json::to_vec(&receipt)?)?;
        std::fs::rename(root.join("lcd-video.tmp"), root.join("lcd-video.json"))?;
        Ok(())
    }
}
#[cfg(test)]
mod video_tests {
    use super::*;
    #[test]
    fn observation_reassembles_b_and_c_units_and_ignores_reordered_incomplete_and_repeated_history()
    {
        use llcontrol_protocol::galahad2_lcdv2::h264::{
            build_send_h264_frame_512_request, build_send_h264_frame_request,
        };
        let root = tempfile::tempdir().unwrap();
        let data = vec![42; 2400];
        let b: Vec<_> = build_send_h264_frame_request(&data)
            .unwrap()
            .into_iter()
            .map(|p| p.to_vec())
            .collect();
        let c: Vec<_> = build_send_h264_frame_512_request(&data)
            .unwrap()
            .into_iter()
            .map(|p| p.to_vec())
            .collect();
        let mut history = b.clone();
        let mut observer = Video::default();
        observer.observe(&history, root.path()).unwrap();
        history.extend([b[0].clone(), b[2].clone(), b[1].clone()]);
        history.extend(c);
        observer.observe(&history, root.path()).unwrap();
        observer.observe(&history, root.path()).unwrap();
        assert_eq!(observer.frames, 2);
        assert_eq!(
            std::fs::read(root.path().join("lcd-video.h264")).unwrap(),
            data.repeat(2)
        );
        history.push(b[0].clone());
        observer.observe(&history, root.path()).unwrap();
        assert_eq!(observer.frames, 2);
    }
    #[test]
    fn observation_limit_does_not_append_an_over_budget_unit() {
        let root = tempfile::tempdir().unwrap();
        let mut observer = Video {
            bytes: 8 * 1024 * 1024,
            ..Default::default()
        };
        let packets =
            llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_request(&[1, 2, 3])
                .unwrap()
                .into_iter()
                .map(|p| p.to_vec())
                .collect::<Vec<_>>();
        assert!(observer.observe(&packets, root.path()).is_err());
        assert!(!root.path().join("lcd-video.h264").exists());
    }
}
