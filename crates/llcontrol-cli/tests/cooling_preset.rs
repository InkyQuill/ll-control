use llcontrol_ipc::{CoolingPolicy, CoolingTarget, NamedProfile, ProfileContent, ProfileGroup};
use std::{io, os::unix::net::UnixListener, process::Command};

#[test]
fn fan_presets_are_offline_portable_policies_with_windows_endpoints() {
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    for name in ["quiet", "standard", "high", "full"] {
        let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
            .env("LLCONTROL_SOCKET", &socket)
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .args([
                "cooling",
                "preset",
                name,
                "--target",
                "fan",
                "--sensor",
                "fixture:cpu",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let policy: CoolingPolicy = serde_json::from_slice(&output.stdout).unwrap();
        policy.validate(CoolingTarget::Fan).unwrap();
        let CoolingPolicy::FanCurve {
            points, sensor_id, ..
        } = &policy
        else {
            panic!("fan reference policy")
        };
        assert_eq!(sensor_id, "fixture:cpu");
        assert_eq!(
            (
                points.first().unwrap().celsius,
                points.first().unwrap().percent
            ),
            (0.0, 10)
        );
        assert_eq!(
            (
                points.last().unwrap().celsius,
                points.last().unwrap().percent
            ),
            (100.0, 100)
        );
        // The exported policy remains acceptable to the same named-slot/reset
        // consumer used by the desktop; no special CLI-only representation.
        let profile = NamedProfile {
            slot: 0,
            name: name.into(),
            content: ProfileContent::Fan(policy),
        };
        profile.validate().unwrap();
        assert_eq!(profile.content.group(), ProfileGroup::Fan);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(!root.path().join("config").exists());
    }
}
