mod app;
mod device;
mod sensors;

use app::LConnectApp;
use tracing::{info, Level};
use tracing_subscriber;

fn main() -> anyhow::Result<()> {
    // Initialize logger
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("LL-Control starting...");

    // Run GUI
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "LL-Control",
        options,
        Box::new(|cc| {
            Box::new(LConnectApp::new(cc))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
}
