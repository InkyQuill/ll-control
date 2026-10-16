//! Read-only provider smoke: no daemon, HID backend, or settings replay.
fn main() -> anyhow::Result<()> {
    let mut provider = lconnect_daemon::sensors::LinuxSensors::default();
    provider.collect(); // CPU utilization needs two counter samples.
    std::thread::sleep(std::time::Duration::from_secs(1));
    println!("{}", serde_json::to_string_pretty(&provider.collect())?);
    Ok(())
}
