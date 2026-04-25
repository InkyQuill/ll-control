use crate::device::galahad::GalahadDevice;
use eframe::egui;
use std::time::Duration;

pub struct LConnectApp {
    // Device connection
    device: Option<GalahadDevice>,

    // Sensor data
    temperature: f32,
    fan_rpm: u16,
    pump_rpm: u16,

    // Control
    fan_pwm: f32,
    pump_pwm: f32,

    // State
    error_message: Option<String>,
}

impl LConnectApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Try to connect to device
        let device = GalahadDevice::connect().ok();
        let connected = device.is_some();

        Self {
            device,
            temperature: 0.0,
            fan_rpm: 0,
            pump_rpm: 0,
            fan_pwm: 50.0,
            pump_pwm: 50.0,
            error_message: if connected {
                None
            } else {
                Some("Device not found. Make sure your GALAHAD II is connected.".to_string())
            },
        }
    }

    fn update_device_data(&mut self) {
        if let Some(device) = &self.device {
            match device.read_handshake() {
                Ok(data) => {
                    self.temperature = data.temperature;
                    self.fan_rpm = data.fan_rpm;
                    self.pump_rpm = data.pump_rpm;
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Device error: {}", e));
                }
            }
        }
    }
}

impl eframe::App for LConnectApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update device data every 1 second
        ctx.request_repaint_after(Duration::from_secs(1));

        if self.device.is_some() {
            self.update_device_data();
        }

        // Main menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Connect Device").clicked() {
                        self.device = GalahadDevice::connect().ok();
                    }
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Settings").clicked() {
                        // TODO: Open settings window
                    }
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.device.is_some() {
                    ui.label(format!("✓ Connected"));
                    ui.label(format!("| Temp: {:.1}°C", self.temperature));
                    ui.label(format!("| Fan: {} RPM", self.fan_rpm));
                    ui.label(format!("| Pump: {} RPM", self.pump_rpm));
                } else {
                    ui.label("✗ No device connected");
                }
            });
        });

        // Main panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("GALAHAD II LCD Control");

            if self.device.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.label("No device connected");
                    if ui.button("Connect to Device").clicked() {
                        self.device = GalahadDevice::connect().ok();
                    }
                });
                return;
            }

            ui.separator();

            // Sensor readings
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Temperature:");
                    ui.label(format!("{:.1}°C", self.temperature));
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.label("Fan Speed:");
                    ui.label(format!("{} RPM", self.fan_rpm));
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.label("Pump Speed:");
                    ui.label(format!("{} RPM", self.pump_rpm));
                });
            });

            ui.separator();

            // PWM controls
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Fan PWM:");
                    ui.add(egui::Slider::new(&mut self.fan_pwm, 0.0..=100.0).text("%"));
                    if ui.button("Apply").clicked() {
                        if let Some(device) = &self.device {
                            if let Err(e) = device.set_fan_pwm(self.fan_pwm as u8) {
                                self.error_message = Some(format!("Failed to set fan: {}", e));
                            }
                        }
                    }
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.label("Pump PWM:");
                    ui.add(egui::Slider::new(&mut self.pump_pwm, 0.0..=100.0).text("%"));
                    if ui.button("Apply").clicked() {
                        if let Some(device) = &self.device {
                            if let Err(e) = device.set_pump_pwm(self.pump_pwm as u8) {
                                self.error_message = Some(format!("Failed to set pump: {}", e));
                            }
                        }
                    }
                });
            });

            ui.separator();

            // Fan curve visualization (simplified - TODO: make interactive)
            ui.label("Fan Curve:");
            ui.label("(Points: 30°C→800RPM, 50°C→1400RPM, 70°C→2200RPM)");

            // Error display
            if let Some(error) = &self.error_message {
                ui.colored_label(egui::Color32::RED, error);
            }
        });
    }
}
