// RealSense Tools in Rust
// Copyright (C) 2025 Carlos Perez-Lopez
//
// This project is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>
//
// You can contact the author via carlospzlz@gmail.com

use eframe::egui;
use eframe::glow;
use eframe::glow::HasContext;
use std::collections::HashSet;
use std::time::Duration;

const VERTEX_SHADER_SRC: &str = r#"
    #version 330 core
    layout(location = 0) in vec3 position;
    layout(location = 1) in vec2 instanceTranslation;
    layout(location = 2) in float instanceDepth;
    layout(location = 3) in vec4 instanceColor;

    uniform mat4 viewProjection;

    out vec4 fragColor;

    void main() {
        vec3 translation = vec3(instanceTranslation.x, instanceTranslation.y, instanceDepth);
        vec3 worldPosition = position * vec3(1.0, 1.0, 1.0) + translation;
        gl_Position = viewProjection * vec4(worldPosition, 1.0);
        fragColor = instanceColor;
    }
"#;

const FRAGMENT_SHADER_SRC: &str = r#"
    #version 330 core
    in vec4 fragColor;
    out vec4 color;
    void main() {
        color = fragColor;
    }
"#;

const FRAME_SIZE: (usize, usize) = (640, 480);

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().collect();
    let enable_auto_exposure = args.len() > 1 && args[1] == "--auto-exposure";

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([730.0, 550.0]),
        ..Default::default()
    };

    let realsense_ctx =
        realsense_rust::context::Context::new().expect("Failed to create RealSense context");

    eframe::run_native(
        "Realsense 3D Viewer \u{1F980}",
        options,
        Box::new(|cc| {
            Ok(Box::new(MyApp::new(
                cc,
                realsense_ctx,
                enable_auto_exposure,
            )))
        }),
    )
}

struct MyApp {
    pipeline: realsense_rust::pipeline::ActivePipeline,
    program: glow::Program,
    vao: glow::VertexArray,
    instance_depth_vbo: glow::NativeBuffer,
    instance_color_vbo: glow::NativeBuffer,
    depth_frame: Option<realsense_rust::frame::DepthFrame>,
    infrared_frame: Option<realsense_rust::frame::InfraredFrame>,
    translation: glam::Vec3,
    rotation: glam::Vec2,
}

impl MyApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        realsense_ctx: realsense_rust::context::Context,
        enable_auto_exposure: bool,
    ) -> Self {
        // Start pipeline
        let devices = realsense_ctx.query_devices(HashSet::new());
        let pipeline = realsense_rust::pipeline::InactivePipeline::try_from(&realsense_ctx)
            .expect("Failed to create inactive pipeline from context");
        let pipeline = start_pipeline(devices, pipeline, enable_auto_exposure);

        // Prepare GL
        let gl = cc
            .gl
            .as_ref()
            .expect("You need to run eframe with the glow backend");

        // Set up shaders
        let program = create_shader_program(gl);

        // Cube vertices (8 unique vertices for the cube)
        let vertices: [f32; 24] = [
            -0.005, -0.005, -0.005, // 0: Bottom-left-back
            0.005, -0.005, -0.005, // 1: Bottom-right-back
            0.005, 0.005, -0.005, // 2: Top-right-back
            -0.005, 0.005, -0.005, // 3: Top-left-back
            -0.005, -0.005, 0.005, // 4: Bottom-left-front
            0.005, -0.005, 0.005, // 5: Bottom-right-front
            0.005, 0.005, 0.005, // 6: Top-right-front
            -0.005, 0.005, 0.005, // 7: Top-left-front
        ];

        // Define indices (referencing the 8 unique vertices)
        let indices: [u32; 36] = [
            0, 1, 2, 2, 3, 0, // Back face
            4, 5, 6, 6, 7, 4, // Front face
            0, 1, 5, 5, 4, 0, // Bottom face
            2, 3, 7, 7, 6, 2, // Top face
            0, 3, 7, 7, 4, 0, // Left face
            1, 2, 6, 6, 5, 1, // Right face
        ];

        // VAO to store:
        // - position VBO
        // - indexes
        // - instance translation VBO
        // - instance depth VBO
        // - instance color VBO
        // - vertex attrib pointers
        let vao = unsafe { gl.create_vertex_array().unwrap() };

        // Unique cube
        unsafe {
            gl.bind_vertex_array(Some(vao));

            // Prepare OpenGL buffers for vertex and index data
            let vertex_buffer = gl.create_buffer().unwrap();
            let index_buffer = gl.create_buffer().unwrap();

            // Load the vertex data into the buffer
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &bytemuck::cast_slice(&vertices),
                glow::STATIC_DRAW,
            );

            // Load the index data into the index buffer
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                &bytemuck::cast_slice(&indices),
                glow::STATIC_DRAW,
            );

            // Set up vertex attribute for position
            let position_location = gl.get_attrib_location(program, "position").unwrap() as u32;
            gl.vertex_attrib_pointer_f32(
                position_location,
                3,
                glow::FLOAT,
                false,
                3 * std::mem::size_of::<f32>() as i32,
                0,
            );
            gl.enable_vertex_attrib_array(position_location);
        }

        let instance_number = FRAME_SIZE.0 * FRAME_SIZE.1;

        // Instance translations
        let mut translation_data: Vec<f32> = vec![0.0; instance_number * 2];
        let (half_width, half_height) = (FRAME_SIZE.0 as f32 / 2.0, FRAME_SIZE.1 as f32 / 2.0);
        for row in 0..FRAME_SIZE.1 {
            for col in 0..FRAME_SIZE.0 {
                let base_index = (row * FRAME_SIZE.0 + col) * 2;
                // First pixel in frame is top-left corner
                translation_data[base_index] = (col as f32 - half_width) / 100.0;
                translation_data[base_index + 1] =
                    ((FRAME_SIZE.1 - row) as f32 - half_height) / 100.0;
            }
        }
        let instance_translation_vbo = unsafe { gl.create_buffer().unwrap() };
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_translation_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &bytemuck::cast_slice(&translation_data),
                glow::STATIC_DRAW,
            );

            let location = gl
                .get_attrib_location(program, "instanceTranslation")
                .unwrap() as u32;
            gl.vertex_attrib_pointer_f32(
                location,
                2,
                glow::FLOAT,
                false,
                2 * std::mem::size_of::<f32>() as i32,
                0,
            );
            gl.enable_vertex_attrib_array(location);

            // Important! translation is per-instance, not per vertex
            gl.vertex_attrib_divisor(location, 1);
        }

        // Initialize instance depths
        let depth_data: Vec<f32> = vec![1.0; instance_number];
        let instance_depth_vbo = unsafe { gl.create_buffer().unwrap() };
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_depth_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &bytemuck::cast_slice(&depth_data),
                glow::DYNAMIC_DRAW,
            );

            let location = gl.get_attrib_location(program, "instanceDepth").unwrap() as u32;
            gl.vertex_attrib_pointer_f32(
                location,
                1,
                glow::FLOAT,
                false,
                std::mem::size_of::<f32>() as i32,
                0,
            );
            gl.enable_vertex_attrib_array(location);

            gl.vertex_attrib_divisor(location, 1);
        }

        // Initialize instance colors
        let color_data: Vec<f32> = vec![0.0; instance_number * 4];
        let instance_color_vbo = unsafe { gl.create_buffer().unwrap() };
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_color_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &bytemuck::cast_slice(&color_data),
                glow::DYNAMIC_DRAW,
            );

            let location = gl.get_attrib_location(program, "instanceColor").unwrap() as u32;
            gl.vertex_attrib_pointer_f32(
                location,
                4,
                glow::FLOAT,
                false,
                4 * std::mem::size_of::<f32>() as i32,
                0,
            );
            gl.enable_vertex_attrib_array(location);

            gl.vertex_attrib_divisor(location, 1);
        }

        // Unbind VAO
        unsafe {
            gl.bind_vertex_array(None);
        }

        Self {
            pipeline,
            program,
            vao,
            instance_depth_vbo,
            instance_color_vbo,
            depth_frame: None,
            infrared_frame: None,
            translation: glam::Vec3::new(0.0, 0.0, -15.0),
            rotation: glam::Vec2::new(0.0, 0.0),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Get frames
        let timeout = Duration::from_millis(100);
        let frames = match self.pipeline.wait(Some(timeout)) {
            Ok(frames) => Some(frames),
            Err(e) => {
                println!("{e}");
                None
            }
        };

        if let Some(ref frames) = frames {
            // Get a pair of:
            //  - Depth frame with emitter on
            //  - IR1 frame with emitter off
            // For some reason 0 is on (maybe the depth was computer from the
            // previous two infrared with emitter 1?). However, in the
            // infrared, 1 gives the frames with no emitter's pattern.
            if self.depth_frame.is_none() {
                let depth_frames = frames.frames_of_type::<realsense_rust::frame::DepthFrame>();
                self.depth_frame = frame_of_type_with_emitter(depth_frames, 0);
            }
            if self.infrared_frame.is_none() {
                let infrared_frames =
                    frames.frames_of_type::<realsense_rust::frame::InfraredFrame>();
                self.infrared_frame = frame_of_type_with_emitter(infrared_frames, 1);
            }
        }

        if self.depth_frame.is_some() && self.infrared_frame.is_some() {
            let depth_frame = self.depth_frame.take().unwrap();
            let infrared_frame = self.infrared_frame.take().unwrap();
            if depth_frame.width() != infrared_frame.width()
                || depth_frame.height() != infrared_frame.height()
            {
                panic!("Make sure depth and infrared frames are the same size");
            }

            let (depth_data, infrared_data) = get_buffers_data(depth_frame, infrared_frame);

            // Get the OpenGL context from the frame
            let gl = frame.gl().expect("Can't get GL from frame");

            // Update instances depth
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instance_depth_vbo));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    &bytemuck::cast_slice(&depth_data),
                    glow::DYNAMIC_DRAW,
                );
            }

            // Update instances color
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instance_color_vbo));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    &bytemuck::cast_slice(&infrared_data),
                    glow::DYNAMIC_DRAW,
                );
            }
        }

        // Compute View Projection matrix
        let projection = glam::Mat4::perspective_rh_gl(45.0_f32.to_radians(), 1.0, 0.1, 100.0);
        let input = egui_ctx.input(|i| i.clone());
        self.translation += get_translation(&input);
        self.rotation += get_rotation(&input);
        let translation = glam::Mat4::from_translation(self.translation);
        let rotation =
            glam::Mat4::from_euler(glam::EulerRot::XYZ, -self.rotation.y, self.rotation.x, 0.0);
        let view = translation * rotation;
        let view_projection = projection * view;

        unsafe {
            // Get the OpenGL context from the frame
            let gl = frame.gl().expect("Can't get GL from frame");

            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            // Enable depth testing
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS); // Default: Pass if fragment is closer

            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vao));

            // Apply view projection matrix
            let uniform_location = gl
                .get_uniform_location(self.program, "viewProjection")
                .unwrap();
            gl.uniform_matrix_4_f32_slice(
                Some(&uniform_location),
                false,
                view_projection.to_cols_array().as_slice(),
            );

            // Draw the cube
            gl.draw_elements_instanced(glow::TRIANGLES, 36, glow::UNSIGNED_INT, 0, 640 * 640);
        }

        egui_ctx.request_repaint();
    }
}

/// Starts RealSense pipeline
fn start_pipeline(
    devices: Vec<realsense_rust::device::Device>,
    pipeline: realsense_rust::pipeline::InactivePipeline,
    enable_auto_exposure: bool,
) -> realsense_rust::pipeline::ActivePipeline {
    let realsense_device = find_realsense(devices);

    if realsense_device.is_none() {
        eprintln!("No RealSense device found!");
        std::process::exit(-1);
    }

    // We want depth and color
    let mut config = realsense_rust::config::Config::new();
    let realsense_device = realsense_device.unwrap();
    let serial_number = realsense_device
        .info(realsense_rust::kind::Rs2CameraInfo::SerialNumber)
        .unwrap();
    config
        .enable_device_from_serial(serial_number)
        .expect("Failed to enable device")
        .disable_all_streams()
        .expect("Failed to disable all streams")
        .enable_stream(
            realsense_rust::kind::Rs2StreamKind::Depth,
            None,
            FRAME_SIZE.0,
            FRAME_SIZE.1,
            realsense_rust::kind::Rs2Format::Z16,
            30,
        )
        .expect("Failed to enable depth stream")
        .enable_stream(
            realsense_rust::kind::Rs2StreamKind::Infrared,
            Some(1),
            FRAME_SIZE.0,
            FRAME_SIZE.1,
            realsense_rust::kind::Rs2Format::Y8,
            30,
        )
        .expect("Failed to enable infrared stream");

    let pipeline = pipeline
        .start(Some(config))
        .expect("Failed to start pipeline");

    for mut sensor in pipeline.profile().device().sensors() {
        // Enable emitter
        if sensor.supports_option(realsense_rust::kind::Rs2Option::EmitterEnabled) {
            sensor
                .set_option(realsense_rust::kind::Rs2Option::EmitterEnabled, 1.0)
                .expect("Failed to set option: EmitterEnabled");
        }
        // Interleave mode, so we have depth and we can overlay IR1
        if sensor.supports_option(realsense_rust::kind::Rs2Option::EmitterOnOff) {
            sensor
                .set_option(realsense_rust::kind::Rs2Option::EmitterOnOff, 1.0)
                .expect("Failed to set option: EmitterOnOff");
        }
        // Enable Auto Exposure
        if sensor.supports_option(realsense_rust::kind::Rs2Option::EnableAutoExposure) {
            let val = if enable_auto_exposure { 1.0 } else { 0.0 };
            sensor
                .set_option(realsense_rust::kind::Rs2Option::EnableAutoExposure, val)
                .expect("Failed to set option: EnableAutoExposure");
        }
    }

    pipeline
}

/// Finds first Real Sense device available
fn find_realsense(
    devices: Vec<realsense_rust::device::Device>,
) -> Option<realsense_rust::device::Device> {
    for device in devices {
        let name = match_info(&device, realsense_rust::kind::Rs2CameraInfo::Name);
        if name.starts_with("Intel RealSense") {
            return Some(device);
        }
    }
    None
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

/// Creates shader program to draw cubes with depth translation
fn create_shader_program(gl: &glow::Context) -> glow::NativeProgram {
    unsafe {
        // Vertex shader
        let vertex_shader = compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER_SRC);

        // Fragment shader
        let fragment_shader = compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER_SRC);

        // Shader program
        let program = gl.create_program().unwrap();
        gl.attach_shader(program, vertex_shader);
        gl.attach_shader(program, fragment_shader);
        gl.link_program(program);
        gl.use_program(Some(program));

        program
    }
}

fn compile_shader(gl: &glow::Context, shader_type: u32, src: &str) -> glow::NativeShader {
    unsafe {
        let shader = gl.create_shader(shader_type).unwrap();
        gl.shader_source(shader, src);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            panic!(
                "Shader compilation failed: {}",
                gl.get_shader_info_log(shader)
            );
        }
        shader
    }
}

fn get_translation(input: &egui::InputState) -> glam::Vec3 {
    if input.pointer.secondary_down() {
        glam::Vec3::new(
            input.pointer.delta().x * 0.01,
            -input.pointer.delta().y * 0.01,
            input.smooth_scroll_delta.y * 0.01,
        )
    } else {
        glam::Vec3::new(0.0, 0.0, input.smooth_scroll_delta.y * 0.01)
    }
}

fn get_rotation(input: &egui::InputState) -> glam::Vec2 {
    if input.pointer.primary_down() {
        glam::Vec2::new(
            input.pointer.delta().x * 0.01,
            -input.pointer.delta().y * 0.01,
        )
    } else {
        glam::Vec2::ZERO
    }
}

fn frame_of_type_with_emitter<T: realsense_rust::frame::FrameEx>(
    mut frames: Vec<T>,
    emitter_mode: i64,
) -> Option<T> {
    if frames.is_empty() {
        return None;
    }

    let frame = &frames[0];
    let mode = frame.metadata(realsense_rust::kind::Rs2FrameMetadata::FrameEmitterMode);

    if mode.is_none() {
        return None;
    }

    if mode.unwrap() == emitter_mode {
        Some(frames.remove(0))
    } else {
        None
    }
}

fn get_buffers_data(
    depth_frame: realsense_rust::frame::DepthFrame,
    infrared_frame: realsense_rust::frame::InfraredFrame,
) -> (Vec<f32>, Vec<f32>) {
    let (width, height) = (depth_frame.width(), depth_frame.height());
    let instance_number = width * height;
    let mut infrared_data: Vec<f32> = vec![0.0; instance_number * 4];
    let mut depth_data: Vec<f32> = vec![0.0; instance_number];
    let max_depth = 4000.0; // 4m
    for col in 0..width {
        for row in 0..height {
            match depth_frame.get_unchecked(col, row) {
                realsense_rust::frame::PixelKind::Z16 { depth } => {
                    let normalized = (*depth as f32 / max_depth).clamp(0.0, 1.0);
                    if normalized > 0.05 {
                        depth_data[row * width + col] = (1.0 - normalized) * 4.0;
                        match infrared_frame.get_unchecked(col, row) {
                            realsense_rust::frame::PixelKind::Y8 { y } => {
                                let base_index = (row * width + col) * 4;
                                infrared_data[base_index] = *y as f32 / 255.0;
                                infrared_data[base_index + 1] = *y as f32 / 255.0;
                                infrared_data[base_index + 2] = *y as f32 / 255.0;
                                infrared_data[base_index + 3] = 1.0;
                            }
                            _ => panic!("Color type is wrong!"),
                        }
                    }
                }
                _ => panic!("Depth type is wrong!"),
            }
        }
    }
    (depth_data, infrared_data)
}
