//! `Pwm` — a `0..=100` percent value.
//!
//! Both `SetFanPWM` (cmd 139) and `SetPumpPWM` (cmd 138) take a single
//! byte percent value the firmware clamps to `100`. We encode that
//! invariant into the type system: callers cannot construct a `Pwm`
//! holding `101+`.
//!
//! Source: `Galahad2VisionDevice.cs:271-281` (`SetFanPWM`) and
//! `Galahad2VisionDevice.cs:316-322` (`SetPumpPWM`) both contain
//! `if (pwm > 100) pwm = 100;` immediately before the write.

/// A PWM duty-cycle percent in the range `0..=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pwm(u8);

impl Pwm {
    /// Construct a `Pwm` from a percent. Returns
    /// [`crate::Error::InvalidPwmPercent`] for values `> 100`.
    pub fn new(percent: u8) -> crate::Result<Self> {
        if percent > 100 {
            return Err(crate::Error::InvalidPwmPercent(percent));
        }
        Ok(Self(percent))
    }

    /// The maximum-valid `Pwm` (100%). Convenience for ramp-to-max tests.
    pub fn max() -> Self {
        Self(100)
    }

    /// The contained percent value, guaranteed `0..=100`.
    pub fn percent(self) -> u8 {
        self.0
    }
}
