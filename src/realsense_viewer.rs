use eframe::egui;
use std::collections::HashSet;
use std::ffi::CString;
use std::time::Duration;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let realsense_ctx =
        realsense_rust::context::Context::new().expect("Failed to create RealSense context");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 550.0]),
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
    pipeline: Option<realsense_rust::pipeline::ActivePipeline>,
    depth_stream_enabled: bool,
    color_stream_enabled: bool,
    infrared_0_stream_enabled: bool,
    infrared_1_stream_enabled: bool,
    accel_stream_enabled: bool,
    gyro_stream_enabled: bool,
    global_time_enabled: bool,
    emitter_enabled: bool,
    auto_exposure_enabled: bool,
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
            depth_stream_enabled: true,
            color_stream_enabled: true,
            infrared_0_stream_enabled: true,
            infrared_1_stream_enabled: true,
            accel_stream_enabled: true,
            gyro_stream_enabled: true,
            global_time_enabled: true,
            emitter_enabled: true,
            auto_exposure_enabled: true,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, egui_ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Reset warning
        self.warning = None;

        // Check selected camera and update pipeline if needed
        let devices = self.realsense_ctx.query_devices(HashSet::new());
        self.update_pipeline_for_selected_device(&devices);

        // Get frames
        let frames = self.get_frames();

        // Update GUI
        self.left_panel(egui_ctx);
        self.right_panel(egui_ctx, &frames);
        self.bottom_panel(egui_ctx, devices);
        self.central_panel(egui_ctx, frames);

        egui_ctx.request_repaint();
    }
}

impl MyApp {
    fn update_pipeline_for_selected_device(
        &mut self,
        devices: &Vec<realsense_rust::device::Device>,
    ) {
        if devices.len() == 0 {
            self.pipeline = None;
            self.warning = Some("No devices!".to_string());
            return;
        }

        if usize::from(self.dev_index) >= devices.len() {
            self.pipeline = None;
            self.warning = Some(format!("Device {0} is gone", self.dev_index));
            return;
        }

        let new_device = &devices[self.dev_index as usize];
        let name = match_info(new_device, realsense_rust::kind::Rs2CameraInfo::Name);
        if !name.starts_with("Intel RealSense") {
            self.pipeline = None;
            self.warning = Some(format!(
                "Device {0} is not an Intel RealSense",
                self.dev_index
            ));
            return;
        }

        let new_serial_number = get_serial_number(new_device);
        if let Some(pipeline) = &self.pipeline {
            let current_device = &pipeline.profile().device();
            if new_serial_number == get_serial_number(current_device) {
                return;
            }
        }

        let pipeline = if let Some(pipeline) = self.pipeline.take() {
            // ActivePipeline -> InactivePipeline
            pipeline.stop()
        } else {
            realsense_rust::pipeline::InactivePipeline::try_from(&self.realsense_ctx)
                .expect("Failed to create inactive pipeline from context")
        };

        let new_serial_number = CString::new(new_serial_number).expect("Failed to create CString");
        self.pipeline = self.start_pipeline(&new_serial_number, pipeline);
    }

    fn update_current_pipeline(&mut self) {
        if let Some(pipeline) = self.pipeline.take() {
            let current_device = pipeline.profile().device();
            let serial_number = get_serial_number(current_device);

            // ActivePipeline -> InactivePipeline
            let pipeline = pipeline.stop();

            let serial_number = CString::new(serial_number).expect("Failed to create CString");
            self.pipeline = self.start_pipeline(&serial_number, pipeline);
        }
    }

    fn start_pipeline(
        &mut self,
        serial_number: &CString,
        pipeline: realsense_rust::pipeline::InactivePipeline,
    ) -> Option<realsense_rust::pipeline::ActivePipeline> {
        if !self.depth_stream_enabled
            && !self.color_stream_enabled
            && !self.infrared_0_stream_enabled
            && !self.infrared_1_stream_enabled
            && !self.accel_stream_enabled
            && !self.gyro_stream_enabled
        {
            self.warning = Some("We need at least one stream to start the pipeline".to_string());
            return None;
        }

        let config = self.create_config(serial_number);
        let pipeline = pipeline
            .start(Some(config))
            .expect("Failed to start pipeline");

        self.update_sensors();

        Some(pipeline)
    }

    /// Config is consumed by start(), we need to create one each time
    fn create_config(&mut self, serial_number: &CString) -> realsense_rust::config::Config {
        let mut config = realsense_rust::config::Config::new();
        config
            .enable_device_from_serial(serial_number)
            .expect("Failed to enable device")
            .disable_all_streams()
            .expect("Failed to disable all streams");

        if self.depth_stream_enabled {
            config
                .enable_stream(
                    realsense_rust::kind::Rs2StreamKind::Depth,
                    None,
                    640,
                    0,
                    realsense_rust::kind::Rs2Format::Z16,
                    30,
                )
                .expect("Failed to enable depth stream");
        } else {
            config
                .disable_stream(realsense_rust::kind::Rs2StreamKind::Depth)
                .expect("Failed to disable depth stream");
        }

        if self.color_stream_enabled {
            config
                .enable_stream(
                    realsense_rust::kind::Rs2StreamKind::Color,
                    None,
                    640,
                    0,
                    realsense_rust::kind::Rs2Format::Bgr8,
                    30,
                )
                .expect("Failed to enable color stream");
        } else {
            config
                .disable_stream(realsense_rust::kind::Rs2StreamKind::Color)
                .expect("Failed to disable depth stream");
        }

        // Index start at 1, madness
        if self.infrared_0_stream_enabled {
            config
                .enable_stream(
                    realsense_rust::kind::Rs2StreamKind::Infrared,
                    Some(1),
                    640,
                    0,
                    realsense_rust::kind::Rs2Format::Y8,
                    30,
                )
                .expect("Failed to enable IR0 stream");
        } else {
            config
                .disable_stream_at_index(realsense_rust::kind::Rs2StreamKind::Infrared, 1)
                .expect("Failed to disable IR0 stream");
        }

        if self.infrared_1_stream_enabled {
            config
                .enable_stream(
                    realsense_rust::kind::Rs2StreamKind::Infrared,
                    Some(2),
                    640,
                    0,
                    realsense_rust::kind::Rs2Format::Y8,
                    30,
                )
                .expect("Failed to enable IR1 stream");
        } else {
            config
                .disable_stream_at_index(realsense_rust::kind::Rs2StreamKind::Infrared, 2)
                .expect("Failed to disable IR1 stream");
        }

        if self.gyro_stream_enabled {
            config
                .enable_stream(
                    realsense_rust::kind::Rs2StreamKind::Gyro,
                    None,
                    0,
                    0,
                    realsense_rust::kind::Rs2Format::Any,
                    0,
                )
                .expect("Failed to enable gyro stream");
        } else {
            config
                .disable_stream(realsense_rust::kind::Rs2StreamKind::Gyro)
                .expect("Failed to disable gyro stream");
        }

        if self.accel_stream_enabled {
            config
                .enable_stream(
                    realsense_rust::kind::Rs2StreamKind::Accel,
                    None,
                    0,
                    0,
                    realsense_rust::kind::Rs2Format::Any,
                    0,
                )
                .expect("Failed to enable accel stream");
        } else {
            config
                .disable_stream(realsense_rust::kind::Rs2StreamKind::Accel)
                .expect("Failed to disable accel stream");
        }

        config
    }

    fn update_sensors(&mut self) {
        if let Some(pipeline) = &self.pipeline {
            for mut sensor in pipeline.profile().device().sensors() {
                if sensor.supports_option(realsense_rust::kind::Rs2Option::GlobalTimeEnabled) {
                    let val = if self.global_time_enabled { 1.0 } else { 0.0 };
                    sensor
                        .set_option(realsense_rust::kind::Rs2Option::GlobalTimeEnabled, val)
                        .expect("Failed to set option: GlobalTimeEnabled");
                }
                if sensor.supports_option(realsense_rust::kind::Rs2Option::EmitterEnabled) {
                    let val = if self.emitter_enabled { 1.0 } else { 0.0 };
                    sensor
                        .set_option(realsense_rust::kind::Rs2Option::EmitterEnabled, val)
                        .expect("Failed to set option: EmitterEnabled");
                }
                if sensor.supports_option(realsense_rust::kind::Rs2Option::EnableAutoExposure) {
                    let val = if self.auto_exposure_enabled { 1.0 } else { 0.0 };
                    sensor
                        .set_option(realsense_rust::kind::Rs2Option::EnableAutoExposure, val)
                        .expect("Failed to set option: EnableAutoExposure");
                }
            }
        }
    }

    fn get_frames(&mut self) -> Option<realsense_rust::frame::CompositeFrame> {
        if let Some(pipeline) = &mut self.pipeline {
            let timeout = Duration::from_millis(20);
            match pipeline.wait(Some(timeout)) {
                Ok(frames) => Some(frames),
                Err(e) => {
                    self.warning = Some(format!("{e}"));
                    None
                }
            }
        } else {
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
                // Account for each frame's margin too
                let width = if frames.count() > 4 {
                    width / 3.0 - 11.0
                } else if frames.count() > 1 {
                    width / 2.0 - 10.0
                } else {
                    width - 5.0
                } as u32;
                let height = (if frames.count() > 2 {
                    height / 2.0
                } else {
                    height
                } - 25.0) as u32;
                let size = (width, height);

                let mut frame_count = 1 as u8;
                let columns = if frames.count() > 4 {
                    3
                } else if frames.count() > 1 {
                    2
                } else {
                    1
                };

                egui::Grid::new("frames").show(ui, |ui| {
                    // Depth frames (either 0 or 1)
                    let depth_frames = frames.frames_of_type::<realsense_rust::frame::DepthFrame>();
                    for depth_frame in depth_frames {
                        let img = depth_frame_to_rgb_img(&depth_frame);
                        self.add_image_frame_item(egui_ctx, ui, img, size, depth_frame);
                        frame_count += 1;
                    }

                    // Color frames (either 0 or 1)
                    let color_frames = frames.frames_of_type::<realsense_rust::frame::ColorFrame>();
                    for color_frame in color_frames {
                        let img = color_frame_to_rgb_img(&color_frame);
                        self.add_image_frame_item(egui_ctx, ui, img, size, color_frame);
                        if frame_count % columns == 0 {
                            ui.end_row();
                        }
                        frame_count += 1;
                    }

                    // IR frames (0, 1 or 2)
                    let ir_frames = frames.frames_of_type::<realsense_rust::frame::InfraredFrame>();
                    for ir_frame in ir_frames {
                        let img = infrared_frame_to_rgb_img(&ir_frame);
                        self.add_image_frame_item(egui_ctx, ui, img, size, ir_frame);
                        if frame_count % columns == 0 {
                            ui.end_row();
                        }
                        frame_count += 1;
                    }

                    // Gyro frames (either 0 or 1)
                    let gyro_frames = frames.frames_of_type::<realsense_rust::frame::GyroFrame>();
                    for gyro_frame in gyro_frames {
                        let rot_velocity = gyro_frame.rotational_velocity();
                        self.add_motion_frame_item(
                            ui,
                            *rot_velocity,
                            size,
                            0.5,
                            gyro_frame,
                            "radians/s",
                        );
                        if frame_count % columns == 0 {
                            ui.end_row();
                        }
                        frame_count += 1;
                    }

                    // Accel frames (either 0 or 1)
                    let accel_frames = frames.frames_of_type::<realsense_rust::frame::AccelFrame>();
                    for accel_frame in accel_frames {
                        let accel = accel_frame.acceleration();
                        self.add_motion_frame_item(ui, *accel, size, 0.1, accel_frame, "m/s²");
                        if frame_count % columns == 0 {
                            ui.end_row();
                        }
                        frame_count += 1;
                    }
                });
            }
        });
    }

    fn add_image_frame_item<T: realsense_rust::frame::FrameEx>(
        &mut self,
        egui_ctx: &egui::Context,
        ui: &mut egui::Ui,
        img: image::RgbImage,
        size: (u32, u32),
        frame: T,
    ) {
        let img = image::DynamicImage::ImageRgb8(img);
        let img = img
            .resize_exact(size.0, size.1, image::imageops::FilterType::Lanczos3)
            .to_rgb8();
        let img = egui::ColorImage::from_rgb([size.0 as usize, size.1 as usize], img.as_raw());
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            ui.vertical(|ui| {
                let texture = egui_ctx.load_texture("unnamed", img, Default::default());
                ui.image(&texture);
                self.add_timestamp_line(ui, size.0 as f32, frame);
            });
        });
    }

    fn add_timestamp_line<T: realsense_rust::frame::FrameEx>(
        &mut self,
        ui: &mut egui::Ui,
        width: f32,
        frame: T,
    ) {
        ui.allocate_ui_with_layout(
            egui::Vec2::new(width, 15.0),
            egui::Layout::left_to_right(egui::Align::Max),
            |ui| {
                let ts = frame.timestamp();
                let ts_domain = frame.timestamp_domain().as_str();
                let label = egui::Label::new(format!("{ts_domain}: {ts:.2}"));
                ui.add(label.wrap_mode(egui::TextWrapMode::Truncate));
            },
        );
    }

    fn add_motion_frame_item<T: realsense_rust::frame::FrameEx>(
        &mut self,
        ui: &mut egui::Ui,
        data: [f32; 3],
        size: (u32, u32),
        scale: f32,
        frame: T,
        units: &str,
    ) {
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            ui.vertical(|ui| {
                // Account for motion values
                let size = (size.0, size.1 - 18);
                let (area, _response) = ui.allocate_at_least(
                    egui::vec2(size.0 as f32, size.1 as f32),
                    egui::Sense::hover(),
                );
                let painter = ui.painter();
                painter.rect_filled(area, 0.0, egui::Color32::BLACK);
                let colors = [
                    egui::Color32::RED,
                    egui::Color32::GREEN,
                    egui::Color32::BLUE,
                ];
                let bar_width = size.0 as f32 / 7.0;
                let bar_max_height = size.1 as f32 / 2.0;
                let mut left_corner =
                    egui::Pos2::new(area.min.x + bar_width, area.min.y + bar_max_height);
                for (component, color) in data.into_iter().zip(colors.into_iter()) {
                    // Positive values grow downwards. Reverse it
                    let height = -component * bar_max_height * scale;
                    // Clamp to limits of area's height
                    let height = height.clamp(-bar_max_height, bar_max_height);
                    let right_corner =
                        egui::Pos2::new(left_corner.x + bar_width, left_corner.y + height);
                    painter.rect_filled(
                        egui::Rect::from_two_pos(left_corner, right_corner),
                        2.0,
                        color,
                    );
                    left_corner.x = left_corner.x + bar_width * 2.0;
                }
                // Horizontal line at origin
                let thickness = 0.5;
                let y = area.min.y + size.1 as f32 / 2.0;
                let left_corner = egui::Pos2::new(area.min.x, y - thickness / 2.0);
                let right_corner = egui::Pos2::new(area.max.x, y + thickness / 2.0);
                painter.rect_filled(
                    egui::Rect::from_two_pos(left_corner, right_corner),
                    0.0,
                    egui::Color32::DARK_GRAY,
                );

                self.add_components_line(ui, size.0 as f32, data, units);
                self.add_timestamp_line(ui, size.0 as f32, frame);
            });
        });
    }

    fn add_components_line(&mut self, ui: &mut egui::Ui, width: f32, data: [f32; 3], units: &str) {
        ui.allocate_ui_with_layout(
            egui::Vec2::new(width, 15.0),
            egui::Layout::left_to_right(egui::Align::Max),
            |ui| {
                let label = egui::Label::new(format!(
                    "X: {:>6.2}  Y: {:>6.2}  Z: {:>6.2}  [{}]",
                    data[0], data[1], data[2], units
                ));
                ui.add(label.wrap_mode(egui::TextWrapMode::Truncate));
            },
        );
    }

    fn left_panel(&mut self, egui_ctx: &egui::Context) {
        egui::SidePanel::left("left_panel")
            .exact_width(130.0)
            .show(egui_ctx, |ui| {
                ui.horizontal(|_ui| {});
                ui.label("Streams");
                ui.horizontal(|ui| {
                    ui.label("Depth");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.checkbox(&mut self.depth_stream_enabled, "").clicked() {
                            self.update_current_pipeline();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Color");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.checkbox(&mut self.color_stream_enabled, "").clicked() {
                            self.update_current_pipeline();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Infrared 0");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui
                            .checkbox(&mut self.infrared_0_stream_enabled, "")
                            .clicked()
                        {
                            self.update_current_pipeline();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Infrared 1");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui
                            .checkbox(&mut self.infrared_1_stream_enabled, "")
                            .clicked()
                        {
                            self.update_current_pipeline();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Gyro");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.checkbox(&mut self.gyro_stream_enabled, "").clicked() {
                            self.update_current_pipeline();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Accel");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.checkbox(&mut self.accel_stream_enabled, "").clicked() {
                            self.update_current_pipeline();
                        }
                    });
                });
                ui.horizontal(|_ui| {});
                ui.horizontal(|_ui| {});
                ui.horizontal(|ui| {
                    ui.label("Sensor Options");
                    let separator = egui::Separator::default();
                    ui.add(separator.horizontal());
                });
                ui.horizontal(|ui| {
                    ui.label("Global Time");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.checkbox(&mut self.global_time_enabled, "").clicked() {
                            self.update_sensors();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Emitter");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.checkbox(&mut self.emitter_enabled, "").clicked() {
                            self.update_sensors();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Auto Exposure");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.checkbox(&mut self.auto_exposure_enabled, "").clicked() {
                            self.update_sensors();
                        }
                    });
                });
            });
    }

    fn right_panel(
        &mut self,
        egui_ctx: &egui::Context,
        frames: &Option<realsense_rust::frame::CompositeFrame>,
    ) {
        egui::SidePanel::right("right_panel")
            .min_width(200.0)
            .max_width(400.0)
            .show(egui_ctx, |ui| {
                ui.horizontal(|_ui| {});

                // General Info
                ui.label("General Info");
                let frames_count = if let Some(frames) = frames {
                    frames.count()
                } else {
                    0
                };
                ui.label(format!("Frames received: {frames_count}"));
                let streams_count = if let Some(pipeline) = &self.pipeline {
                    pipeline.profile().streams().len()
                } else {
                    0
                };
                ui.label(format!("Streams: {streams_count}"));
                let sensors_count = if let Some(pipeline) = &self.pipeline {
                    pipeline.profile().device().sensors().len()
                } else {
                    0
                };
                ui.label(format!("Sensors: {sensors_count}"));
                if let Some(pipeline) = &self.pipeline {
                    for sensor in pipeline.profile().device().sensors() {
                        let name = sensor.info(realsense_rust::kind::Rs2CameraInfo::Name);
                        if let Some(name) = name {
                            let name = String::from(name.to_str().unwrap());
                            ui.label(format!("  •  {name}"));
                        }
                    }
                }
                ui.horizontal(|_ui| {});

                // Streams Info
                ui.horizontal(|ui| {
                    ui.label("Streams Info");
                    let separator = egui::Separator::default();
                    ui.add(separator.horizontal());
                });
                if let Some(pipeline) = &self.pipeline {
                    // Search for IR0 stream as reference stream to get extrinsics to
                    //};
                    let ir0_stream_profile = pipeline
                        .profile()
                        .streams()
                        .iter()
                        .find(|s| {
                            s.kind() == realsense_rust::kind::Rs2StreamKind::Infrared
                                && s.index() == 1
                        })
                        .expect("IR0 stream not found!");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // Print info of all streams
                        for stream_profile in pipeline.profile().streams() {
                            let kind = stream_profile.kind();
                            let index = stream_profile.index();
                            ui.collapsing(format!("{:?}:{index}", kind), |ui| {
                                ui.label(format!("Format: {:?}", stream_profile.format()));
                                ui.label(format!("Unique ID: {}", stream_profile.unique_id()));
                                ui.label(format!("Framerate: {}", stream_profile.framerate()));
                                match stream_profile.intrinsics() {
                                    Ok(intrinsics) => {
                                        ui.label(egui::RichText::new("Intrinsics:").strong());
                                        ui.label(format!(
                                            "Size: {}x{}",
                                            intrinsics.width(),
                                            intrinsics.height()
                                        ));
                                        ui.label(format!(
                                            "Principal Point: {}, {}",
                                            intrinsics.ppx(),
                                            intrinsics.ppy()
                                        ));
                                        ui.label(format!(
                                            "Focal Length: {}, {}",
                                            intrinsics.fx(),
                                            intrinsics.fy()
                                        ));
                                        let distortion = intrinsics.distortion();
                                        ui.label(format!(
                                            "Distortion Model: {:?}",
                                            distortion.model
                                        ));
                                        ui.label(format!(
                                            "Distortion Coeffs: {:?}",
                                            distortion.coeffs
                                        ));
                                    }
                                    Err(_) => (),
                                }
                                match stream_profile.extrinsics(ir0_stream_profile) {
                                    Ok(extrinsics) => {
                                        ui.label(egui::RichText::new("Extrinsics:").strong());
                                        ui.label(format!("To IR1: {:?}", extrinsics.translation()));
                                    }
                                    Err(_) => (),
                                }
                            });
                            ui.horizontal(|_ui| {});
                        }
                    });
                }
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
fn get_serial_number(device: &realsense_rust::device::Device) -> String {
    match_info(&device, realsense_rust::kind::Rs2CameraInfo::SerialNumber)
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

///
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

///
fn depth_frame_to_rgb_img(frame: &realsense_rust::frame::DepthFrame) -> image::RgbImage {
    let max_value = 4000.0; // 4m
    let mut img = image::RgbImage::new(frame.width() as u32, frame.height() as u32);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        match frame.get_unchecked(x as usize, y as usize) {
            realsense_rust::frame::PixelKind::Z16 { depth } => {
                let normalized = *depth as f32 / max_value;
                *pixel = jet_colormap(normalized);
            }
            _ => panic!("Depth type is wrong!"),
        }
    }
    img
}

/// Implement the classic jet color map
/// Blue -> Cyan -> Yellow -> Red -> Black
fn jet_colormap(value: f32) -> image::Rgb<u8> {
    let v = value.clamp(0.0, 1.0);

    let (r, g, b) = if v < 0.25 {
        lerp_color(v, 0.00, (0, 0, 255), 0.25, (0, 255, 255)) // Blue → Cyan
    } else if v < 0.5 {
        lerp_color(v, 0.25, (0, 255, 255), 0.5, (255, 255, 0)) // Cyan → Yellow
    } else if v < 0.75 {
        lerp_color(v, 0.5, (255, 255, 0), 0.75, (255, 0, 0)) // Green → Yellow
    } else {
        lerp_color(v, 0.8, (255, 0, 0), 1.00, (0, 0, 0)) // Dark Red → Black
    };

    image::Rgb([r, g, b])
}

/// Linearly interpolates between two colors based on value position.
fn lerp_color(
    value: f32,
    v_min: f32,
    c_min: (u8, u8, u8),
    v_max: f32,
    c_max: (u8, u8, u8),
) -> (u8, u8, u8) {
    let t = ((value - v_min) / (v_max - v_min)).clamp(0.0, 1.0);
    (
        (c_min.0 as f32 + t * (c_max.0 as f32 - c_min.0 as f32)) as u8,
        (c_min.1 as f32 + t * (c_max.1 as f32 - c_min.1 as f32)) as u8,
        (c_min.2 as f32 + t * (c_max.2 as f32 - c_min.2 as f32)) as u8,
    )
}
