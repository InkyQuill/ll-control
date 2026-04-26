//! lconnectd — the L-Connect Linux daemon.
//!
//! Body lands across P1D-2..P1D-15. This scaffold establishes the
//! tokio runtime, sets up tracing, and exits.

#![forbid(unsafe_code)]

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    tracing::info!("lconnectd starting (scaffold; no work yet)");
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("lconnectd=info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
