//! Stub - body lands in P1E-5.

use crate::{LedColorsArgs, LightingArgs};

pub async fn run(_args: LightingArgs) -> anyhow::Result<()> {
    anyhow::bail!("lighting: not yet implemented (P1E-5)")
}

pub async fn led_colors(_args: LedColorsArgs) -> anyhow::Result<()> {
    anyhow::bail!("led-colors: not yet implemented (P1E-5)")
}
