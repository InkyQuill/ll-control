use llcontrol_ipc::{validate_h264_clip, H264AccessUnit, H264Clip, IpcError};

fn valid_clip() -> H264Clip {
    H264Clip {
        fps: 24,
        units: vec![H264AccessUnit {
            bytes: vec![
                0, 0, 0, 1, 0x67, 0x42, 0, 30, 0, 0, 1, 0x68, 0xce, 0, 0, 1, 0x65, 0x88,
            ],
            keyframe: true,
        }],
    }
}

fn assert_invalid(clip: &H264Clip, loops: u16) {
    assert!(matches!(
        validate_h264_clip(clip, loops),
        Err(IpcError::InvalidArgument(_))
    ));
}

#[test]
fn valid_diagnostic_clip_is_accepted() {
    validate_h264_clip(&valid_clip(), 1).expect("valid diagnostic clip");
}

#[test]
fn clip_wire_types_roundtrip() {
    let clip = valid_clip();
    let encoded = serde_json::to_vec(&clip).expect("serialize clip");
    let decoded: H264Clip = serde_json::from_slice(&encoded).expect("deserialize clip");
    assert_eq!(decoded, clip);
}

#[test]
fn empty_units_are_rejected() {
    let mut clip = valid_clip();
    clip.units.clear();
    assert_invalid(&clip, 1);
}

#[test]
fn unsupported_frame_rates_are_rejected() {
    for fps in [0, 25] {
        let mut clip = valid_clip();
        clip.fps = fps;
        assert_invalid(&clip, 1);
    }
}

#[test]
fn loop_bounds_are_enforced() {
    let clip = valid_clip();
    assert_invalid(&clip, 0);
    assert_invalid(&clip, 901);
}

#[test]
fn oversized_access_unit_is_rejected() {
    let mut clip = valid_clip();
    clip.units[0].bytes.resize(128 * 1024 + 1, 0x55);
    assert_invalid(&clip, 1);
}

#[test]
fn oversized_total_clip_is_rejected() {
    let mut clip = valid_clip();
    let non_keyframe = H264AccessUnit {
        bytes: {
            let mut bytes = vec![0, 0, 1, 0x41];
            bytes.resize(100 * 1024, 0x55);
            bytes
        },
        keyframe: false,
    };
    clip.units.extend((0..11).map(|_| non_keyframe.clone()));
    assert_invalid(&clip, 1);
}

#[test]
fn keyframe_marker_without_idr_is_rejected() {
    let mut clip = valid_clip();
    clip.units[0].bytes = vec![0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x41, 1];
    assert_invalid(&clip, 1);
}

#[test]
fn truncated_start_code_is_rejected() {
    let mut clip = valid_clip();
    clip.units[0].bytes = vec![0, 0, 1];
    assert_invalid(&clip, 1);
}
