use eframe::egui;
use realsense_rust::frame::FrameEx;
use std::collections::HashSet;
use std::ffi::CStr;
use std::time::Duration;

/// Gets info from a device or returns "N/A"
fn match_info(
    device: &realsense_rust::device::Device,
    info_param: realsense_rust::kind::Rs2CameraInfo,
) -> String {
    match device.info(info_param) {
        Some(s) => String::from(s.to_str().unwrap()),
        None => String::from("N/A"),
    }
}

///
fn get_dev_repr(index: u8, dev: &realsense_rust::device::Device) -> String {
    let name = match_info(dev, realsense_rust::kind::Rs2CameraInfo::Name);
    let serial_number = match_info(&dev, realsense_rust::kind::Rs2CameraInfo::SerialNumber);
    format!("{index}: {name} ({serial_number})")
}

///
fn color_frame_to_rgb_img(color_frame: &realsense_rust::frame::ColorFrame) -> image::RgbImage {
    let mut img = image::RgbImage::new(color_frame.width() as u32, color_frame.height() as u32);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        match color_frame.get_unchecked(x as usize, y as usize) {
            realsense_rust::frame::PixelKind::Bgr8 { b, g, r } => {
                *pixel = image::Rgb([*r, *g, *b]);
            }
            _ => panic!("Color type is wrong!"),
        }
    }
    img
}

fn infrared_frame_to_rgb_img(frame: &realsense_rust::frame::InfraredFrame) -> image::RgbImage {
    let mut img = image::RgbImage::new(frame.width() as u32, frame.height() as u32);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        match frame.get_unchecked(x as usize, y as usize) {
            realsense_rust::frame::PixelKind::Y8 { y } => {
                *pixel = image::Rgb([*y, *y, *y]);
            }
            _ => panic!("Color type is wrong!"),
        }
    }
    img
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

fn create_pipeline(
    ctx: &realsense_rust::context::Context,
    sn: &CStr,
) -> Option<realsense_rust::pipeline::ActivePipeline> {
    let pipeline = realsense_rust::pipeline::InactivePipeline::try_from(ctx)
        .expect("Failed to create pipeline from context");
    let mut config = realsense_rust::config::Config::new();
    config
        .enable_device_from_serial(sn)
        .expect("Failed to enable device")
        .enable_all_streams()
        .expect("Failed to disable all streams");
    //.enable_stream(
    //    realsense_rust::kind::Rs2StreamKind::Color,
    //    None,
    //    640,
    //    0,
    //    realsense_rust::kind::Rs2Format::Bgr8,
    //    30,
    //)
    //.expect("Failed to enable the color stream")
    //.enable_stream(
    //    realsense_rust::kind::Rs2StreamKind::Depth,
    //    None,
    //    0,
    //    240,
    //    realsense_rust::kind::Rs2Format::Z16,
    //    30,
    //)
    //.expect("Failed to enable the depth stream");

    // Change pipeline's type from InactivePipeline -> ActivePipeline
    let pipeline = pipeline
        .start(Some(config))
        .expect("Failed to start pipeline");
    Some(pipeline)
}

struct MyApp {
    realsense_ctx: realsense_rust::context::Context,
    dev_index: u8,
    warning: Option<String>,
    pipeline: Option<realsense_rust::pipeline::ActivePipeline>,
    depth_stream_enabled: bool,
    color_stream_enabled: bool,
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
            pipeline: None,
            depth_stream_enabled: false,
            color_stream_enabled: false,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, egui_ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update state
        let devices = self.realsense_ctx.query_devices(HashSet::new());
        self.update_pipeline(&devices);
        let frames = self.get_frames();

        // Update GUI
        self.left_panel(egui_ctx);
        self.right_panel(egui_ctx);
        self.bottom_panel(egui_ctx, devices);
        self.central_panel(egui_ctx, frames);

        egui_ctx.request_repaint();
    }
}

impl MyApp {
    fn update_pipeline(&mut self, devices: &Vec<realsense_rust::device::Device>) {
        if devices.len() > 0 {
            if usize::from(self.dev_index) < devices.len() {
                let name = match_info(
                    &devices[self.dev_index as usize],
                    realsense_rust::kind::Rs2CameraInfo::Name,
                );
                if name.starts_with("Intel RealSense") {
                    if !self.pipeline.is_some() {
                        let serial_number = devices[self.dev_index as usize]
                            .info(realsense_rust::kind::Rs2CameraInfo::SerialNumber)
                            .expect("Failed to get serial number");
                        self.pipeline = create_pipeline(&self.realsense_ctx, serial_number);
                    }
                    self.warning = None;
                } else {
                    self.pipeline = None;
                    self.warning = Some(format!(
                        "Device {0} is not an Intel RealSense",
                        self.dev_index
                    ));
                }
            } else {
                self.pipeline = None;
                self.warning = Some(format!("Device {0} is gone", self.dev_index));
            }
        } else {
            self.pipeline = None;
            self.warning = Some("No devices!".to_string());
        }
    }

    fn get_frames(&mut self) -> Option<realsense_rust::frame::CompositeFrame> {
        if let Some(pipeline) = &mut self.pipeline {
            let timeout = Duration::from_millis(200);
            match pipeline.wait(Some(timeout)) {
                Ok(frames) => Some(frames),
                Err(e) => {
                    self.warning = Some(format!("{e}"));
                    None
                }
            }
        } else {
            self.warning = None;
            None
        }
    }

    fn central_panel(
        &mut self,
        egui_ctx: &egui::Context,
        frames: Option<realsense_rust::frame::CompositeFrame>,
    ) {
        egui::CentralPanel::default().show(egui_ctx, |ui| {
            // Draw all frames
            if let Some(frames) = frames {
                // Distribute all available space
                let asize = ui.available_size();
                let (width, height) = (asize[0].round(), asize[1].round());
                let width = if frames.count() > 4 {
                    width / 3.0
                } else if frames.count() > 1 {
                    width / 2.0
                } else {
                    width
                } as u32;
                let height = if frames.count() > 2 {
                    height / 2.0
                } else {
                    height
                } as u32;

                egui::Grid::new("frames").show(ui, |ui| {
                    // Color frames (single one)
                    let color_frames = frames.frames_of_type::<realsense_rust::frame::ColorFrame>();
                    let color_frame = &color_frames[0];
                    let img = color_frame_to_rgb_img(color_frame);
                    let img = image::DynamicImage::ImageRgb8(img);
                    let img = img
                        .resize_exact(width, height, image::imageops::FilterType::Lanczos3)
                        .to_rgb8();
                    let img = egui::ColorImage::from_rgb(
                        [width as usize, height as usize],
                        img.as_raw(),
                    );
                    ui.vertical(|ui| {
                        let texture = egui_ctx.load_texture("color_frame", img, Default::default());
                        ui.image(&texture);
                        let ts = color_frame.timestamp();
                        let ts_domain = color_frame.timestamp_domain().as_str();
                        ui.label(format!("ts ({ts_domain}): {ts}"));
                    });

                    // IR frame
                    let ir_frames = frames.frames_of_type::<realsense_rust::frame::InfraredFrame>();
                    for ir_frame in ir_frames {
                        let img = infrared_frame_to_rgb_img(&ir_frame);
                        let img = image::DynamicImage::ImageRgb8(img);
                        let img = img
                            .resize_exact(width, height, image::imageops::FilterType::Lanczos3)
                            .to_rgb8();
                        let img = egui::ColorImage::from_rgb(
                            [width as usize, height as usize],
                            img.as_raw(),
                        );
                        ui.vertical(|ui| {
                            let texture = egui_ctx.load_texture("color_frame", img, Default::default());
                            ui.image(&texture);
                            let ts = ir_frame.timestamp();
                            let ts_domain = ir_frame.timestamp_domain().as_str();
                            ui.label(format!("ts ({ts_domain}): {ts}"));
                        });
                    }

                });
            }
        });
    }

    fn left_panel(&mut self, egui_ctx: &egui::Context) {
        egui::SidePanel::left("left_panel").show(egui_ctx, |ui| {
            egui::Grid::new("streams").show(ui, |ui| {
                ui.label("Streams");
                ui.end_row();
                ui.label("Depth");
                ui.checkbox(&mut self.depth_stream_enabled, "");
                ui.end_row();
                ui.label("Color");
                ui.checkbox(&mut self.color_stream_enabled, "");
                ui.end_row();
                ui.label("Infrared");
                ui.checkbox(&mut self.color_stream_enabled, "");
                ui.end_row();
                ui.label("Fisheye");
                ui.checkbox(&mut self.color_stream_enabled, "");
                ui.end_row();
                ui.label("Gyro");
                ui.checkbox(&mut self.color_stream_enabled, "");
                ui.end_row();
                ui.label("Accel");
                ui.checkbox(&mut self.color_stream_enabled, "");
                ui.end_row();
                ui.label("Gpio");
                ui.checkbox(&mut self.color_stream_enabled, "");
                ui.end_row();
                ui.label("Pose");
                ui.checkbox(&mut self.color_stream_enabled, "");
                ui.end_row();
                ui.label("Confidence");
                ui.checkbox(&mut self.color_stream_enabled, "");
                ui.end_row();
            });
        });
    }

    fn right_panel(&mut self, egui_ctx: &egui::Context) {
        egui::SidePanel::right("right_panel").show(egui_ctx, |ui| {
            ui.label("Streams");
            ui.label("Depth");
            //ui.checkbox(&mut self.split_view, "");
        });
    }

    fn bottom_panel(
        &mut self,
        egui_ctx: &egui::Context,
        devices: Vec<realsense_rust::device::Device>,
    ) {
        egui::TopBottomPanel::bottom("bottom_panel").show(egui_ctx, |ui| {
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
            } else {
                ui.label("");
            }
        });
    }
}
