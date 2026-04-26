//! `DaemonRpc` trait — the IPC contract.
//!
//! Implementations: the daemon's `IpcServerImpl` (production) and any
//! mocks the test harness defines. The trait is the seam for a future
//! DBus migration: only the transport changes; the trait stays put.

use crate::*;
use async_trait::async_trait;
use futures::stream::BoxStream;

#[async_trait]
pub trait DaemonRpc: Send + Sync + 'static {
    // Discovery
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>>;
    async fn get_firmware(&self, dev: DeviceId) -> Result<FirmwareInfo>;

    // Sensors
    async fn get_snapshot(&self, dev: DeviceId) -> Result<SensorSnapshot>;
    async fn subscribe_snapshots(
        &self,
        dev: DeviceId,
    ) -> Result<BoxStream<'static, SensorSnapshot>>;

    // PWM
    async fn set_fan_pwm(&self, dev: DeviceId, percent: Pwm) -> Result<()>;
    async fn set_pump_pwm(&self, dev: DeviceId, percent: Pwm, source: PwmSource) -> Result<()>;

    // Lighting
    async fn get_lighting_info(&self, dev: DeviceId, kind: LightingKind) -> Result<LightingInfo>;
    async fn set_lighting(
        &self,
        dev: DeviceId,
        kind: LightingKind,
        cfg: LightingConfig,
    ) -> Result<()>;
    async fn set_led_colors(
        &self,
        dev: DeviceId,
        target: LedTarget,
        colors: Vec<Rgb>,
    ) -> Result<()>;

    // LCD
    async fn is_lcd_available(&self, dev: DeviceId) -> Result<bool>;
    async fn set_lcd_config(&self, dev: DeviceId, cfg: LcdConfig) -> Result<()>;
    async fn set_lcd_image(&self, dev: DeviceId, jpeg: Vec<u8>) -> Result<()>;
    async fn clear_lcd(&self, dev: DeviceId) -> Result<()>;

    // Maintenance
    async fn request_reset_token(&self, dev: DeviceId) -> Result<ResetToken>;
    async fn reset_device(&self, dev: DeviceId, confirm: ResetToken) -> Result<()>;
    async fn raw_send(
        &self,
        dev: DeviceId,
        framing: FramingKind,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>>;
    async fn daemon_stats(&self) -> Result<DaemonStats>;
}
