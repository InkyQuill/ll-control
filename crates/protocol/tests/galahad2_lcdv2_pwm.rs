//! Tests for Pwm newtype (used by SetFanPWM and SetPumpPWM).

use llcontrol_protocol::galahad2_lcdv2::pwm::Pwm;

#[test]
fn pwm_accepts_zero() {
    let p = Pwm::new(0).expect("0 is valid");
    assert_eq!(p.percent(), 0);
}

#[test]
fn pwm_accepts_full_range() {
    for v in 0..=100u8 {
        Pwm::new(v).unwrap_or_else(|_| panic!("expected {v} to be valid"));
    }
}

#[test]
fn pwm_rejects_above_100() {
    for v in [101u8, 150, 200, 255] {
        let err = Pwm::new(v).unwrap_err();
        assert!(matches!(
            err,
            llcontrol_protocol::Error::InvalidPwmPercent(actual) if actual == v
        ));
    }
}

#[test]
fn pwm_max_helper_returns_100() {
    assert_eq!(Pwm::max().percent(), 100);
}
