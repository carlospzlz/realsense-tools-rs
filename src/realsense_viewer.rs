use eframe::egui;
use std::collections::HashSet;

/// Gets info from a device or returns "N/A" if a provided info parameter
/// doesn't exist for the provided device.
fn match_info(
    device: &realsense_rust::device::Device,
    info_param: realsense_rust::kind::Rs2CameraInfo,
) -> String {
    match device.info(info_param) {
        Some(s) => String::from(s.to_str().unwrap()),
        None => String::from("N/A"),
    }
}

fn get_dev_repr(index: u8, dev: &realsense_rust::device::Device) -> String {
    let name = match_info(dev, realsense_rust::kind::Rs2CameraInfo::Name);
    let serial_number = match_info(&dev, realsense_rust::kind::Rs2CameraInfo::SerialNumber);
    format!("{index}: {name} ({serial_number})")
}

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let realsense_ctx =
        realsense_rust::context::Context::new().expect("Failed to create RealSense context");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([750.0, 550.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Realsense Viewer \u{1F980}",
        options,
        Box::new(|cc| Ok(Box::new(MyApp::new(cc, realsense_ctx)))),
    )
}

struct MyApp {
    realsense_ctx: realsense_rust::context::Context,
    dev_index: u8,
    warning: Option<String>,
}

impl MyApp {
    fn new(
        _cc: &eframe::CreationContext<'_>,
        realsense_ctx: realsense_rust::context::Context,
    ) -> Self {
        Self {
            realsense_ctx,
            dev_index: 0,
            warning: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, egui_ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update state
        let devices = self.realsense_ctx.query_devices(HashSet::new());
        if devices.len() > 0 {
            if usize::from(self.dev_index) < devices.len() {
                let name = match_info(
                    &devices[self.dev_index as usize],
                    realsense_rust::kind::Rs2CameraInfo::Name,
                );
                if name.starts_with("Intel RealSense") {
                    self.warning = None;
                } else {
                    self.warning = Some(format!(
                        "Device {0} is not an Intel RealSense",
                        self.dev_index
                    ));
                }
            } else {
                self.warning = Some(format!("Device {0} is gone", self.dev_index));
            }
        } else {
            self.warning = Some("No devices!".to_string());
        }

        // Update GUI
        egui::CentralPanel::default().show(egui_ctx, |ui| {
            // Select device
            ui.horizontal(|ui| {
                ui.label("Select device: ");
                let selected_dev_repr = if usize::from(self.dev_index) < devices.len() {
                    get_dev_repr(self.dev_index, &devices[self.dev_index as usize])
                } else {
                    String::default()
                };
                egui::ComboBox::from_label("")
                    .selected_text(&selected_dev_repr)
                    .show_ui(ui, |ui| {
                        for (i, dev) in devices.iter().enumerate() {
                            let dev_repr = get_dev_repr(i as u8, dev);
                            if ui
                                .selectable_label(dev_repr == selected_dev_repr, dev_repr)
                                .clicked()
                            {
                                self.dev_index = i as u8;
                            }
                        }
                    });
            });

            // Devices table
            egui::Grid::new("devices").striped(true).show(ui, |ui| {
                // Header
                ui.label(egui::RichText::new("Index").strong());
                ui.label(egui::RichText::new("Name").strong());
                ui.label(egui::RichText::new("Serial Number").strong());
                ui.label(egui::RichText::new("Firmware Version").strong());
                ui.label(egui::RichText::new("Recommended").strong());
                ui.end_row();

                for (index, device) in devices.iter().enumerate() {
                    ui.label(index.to_string());
                    ui.label(match_info(
                        &device,
                        realsense_rust::kind::Rs2CameraInfo::Name,
                    ));
                    ui.label(match_info(
                        &device,
                        realsense_rust::kind::Rs2CameraInfo::SerialNumber,
                    ));
                    ui.label(match_info(
                        &device,
                        realsense_rust::kind::Rs2CameraInfo::FirmwareVersion,
                    ));
                    ui.label(match_info(
                        &device,
                        realsense_rust::kind::Rs2CameraInfo::RecommendedFirmwareVersion,
                    ));
                    ui.end_row();
                }
            });

            if let Some(msg) = &self.warning {
                ui.colored_label(egui::Color32::YELLOW, msg);
            }
        });

        egui_ctx.request_repaint();
    }
}
