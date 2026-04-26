//! Stub - body lands in P1E-4.

pub async fn fan(_percent: u8, _device: Option<String>) -> anyhow::Result<()> {
    anyhow::bail!("fan: not yet implemented (P1E-4)")
}

pub async fn pump(
    _percent: u8,
    _source: Option<String>,
    _device: Option<String>,
) -> anyhow::Result<()> {
    anyhow::bail!("pump: not yet implemented (P1E-4)")
}
