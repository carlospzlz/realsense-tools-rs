use eframe::egui;
use eframe::glow;
use eframe::glow::HasContext;
use realsense_rust::frame::FrameEx;
use std::collections::HashSet;
use std::time::Duration;

const VERTEX_SHADER_SRC: &str = r#"
    #version 330 core
    layout(location = 0) in vec3 position;
    layout(location = 1) in vec2 instanceTranslation;
    layout(location = 2) in float instanceHeight;
    layout(location = 3) in vec4 instanceColor;

    uniform mat4 viewProjection;

    out vec4 fragColor;

    void main() {
        //float baseCorrection = (1.0 - instanceHeight) / 2.0;
        //float baseCorrection = 0.0;
        vec3 translation = vec3(instanceTranslation.x, instanceTranslation.y, 1.0);
        vec3 worldPosition = position * vec3(1.0, 1.0, instanceHeight * 100.0) + translation;
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

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        depth_buffer: 1, // Important for 3D rendering
        ..Default::default()
    };

    let realsense_ctx =
        realsense_rust::context::Context::new().expect("Failed to create RealSense context");

    eframe::run_native(
        "Glow Example",
        options,
        Box::new(|cc| Ok(Box::new(MyApp::new(cc, realsense_ctx)))),
    )
}

struct MyApp {
    realsense_ctx: realsense_rust::context::Context,
    pipeline: realsense_rust::pipeline::ActivePipeline,
    program: glow::Program,
    vao: glow::VertexArray,
    angle: f32,
    translation: glam::Vec3,
    rotation: glam::Vec2,
    last_mouse_pos: Option<egui::Pos2>,
    instance_height_vbo: glow::NativeBuffer,
    instance_color_vbo: glow::NativeBuffer,
    previous_frames: Option<realsense_rust::frame::CompositeFrame>,
    depth_frame: Option<realsense_rust::frame::DepthFrame>,
    infrared_frame: Option<realsense_rust::frame::InfraredFrame>,
}

impl MyApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        realsense_ctx: realsense_rust::context::Context,
    ) -> Self {
        // Start pipeline
        let devices = realsense_ctx.query_devices(HashSet::new());
        let pipeline = realsense_rust::pipeline::InactivePipeline::try_from(&realsense_ctx)
            .expect("Failed to create inactive pipeline from context");
        let pipeline = start_pipeline(devices, pipeline);

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
        // - instance height VBO
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

        // Number of vertices
        let instance_number = 640 * 640;

        // Instance translations
        let mut translation_data: Vec<f32> = vec![0.0; instance_number * 2];
        for x in 0..640 {
            for y in 0..640 {
                let base_index = (x * 640 + y) * 2;
                translation_data[base_index] = (x as f32 - 320.0) / 100.0;
                translation_data[base_index + 1] = (y as f32 - 320.0) / 100.0;
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

        // Initialize instance heights
        let height_data: Vec<f32> = vec![1.0; instance_number];
        let instance_height_vbo = unsafe { gl.create_buffer().unwrap() };
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_height_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &bytemuck::cast_slice(&height_data),
                glow::DYNAMIC_DRAW,
            );

            let location = gl.get_attrib_location(program, "instanceHeight").unwrap() as u32;
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
            realsense_ctx,
            pipeline: pipeline,
            program: program,
            vao: vao,
            angle: 0.0,
            translation: glam::Vec3::new(0.0, 0.0, -15.0),
            rotation: glam::Vec2::new(0.0, 0.0),
            last_mouse_pos: None,
            instance_height_vbo,
            instance_color_vbo,
            previous_frames: None,
            depth_frame: None,
            infrared_frame: None,
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
            // Sync
            if self.depth_frame.is_none() {
                let depth_frames = frames.frames_of_type::<realsense_rust::frame::DepthFrame>();
                self.depth_frame = frame_of_type_with_emitter(depth_frames, 1);
            }
            if self.infrared_frame.is_none() {
                let infrared_frames = frames.frames_of_type::<realsense_rust::frame::InfraredFrame>();
                self.infrared_frame = frame_of_type_with_emitter(infrared_frames, 1);
            }
        }

        // Create buffer of infrared depth 3D bars
        //if let Some(ref frames) = frames {
        //    let depth_frames = frames.frames_of_type::<realsense_rust::frame::DepthFrame>();
        //    let depth_frame = &depth_frames[0];
        //    let infrared_frames = frames.frames_of_type::<realsense_rust::frame::InfraredFrame>();
        //    let infrared_frame = &infrared_frames[0];
        //    let depth_emitter =
        //        depth_frame.metadata(realsense_rust::kind::Rs2FrameMetadata::FrameEmitterMode);
        //    let infrared_emitter =
        //        infrared_frame.metadata(realsense_rust::kind::Rs2FrameMetadata::FrameEmitterMode);
        //    //println!("{} / {}", depth_frames.len(), infrared_frames.len());
        //    //println!("{} | {}", depth_emitter.unwrap(), infrared_emitter.unwrap());
        //    if !depth_frames.is_empty() {
        //        let depth_frame = &depth_frames[0];
        //        if let Some(emitter_mode) =
        //            depth_frame.metadata(realsense_rust::kind::Rs2FrameMetadata::FrameEmitterMode)
        //        {
        //            // It seems that when the emitter is off is when depth is sane,
        //            // I guess that's because it's computed from the previous infrared
        //            // frames, where the emitter was on.
        //            if emitter_mode == 1 {
        //                if let Some(previous_frames) = self.previous_frames.take() {
        //                    println!("{} | {}", depth_emitter.unwrap(), infrared_emitter.unwrap());
        //                    let infrared_frames =
        //                        frames.frames_of_type::<realsense_rust::frame::InfraredFrame>();
        //                    let infrared_frame = &infrared_frames[0];
        //                    let emitter = infrared_frame
        //                        .metadata(realsense_rust::kind::Rs2FrameMetadata::FrameEmitterMode);
        //                    println!("{}", emitter.unwrap());

        if self.depth_frame.is_some() && self.infrared_frame.is_some() {
            let depth_frame = self.depth_frame.take().unwrap();
            let infrared_frame = self.infrared_frame.take().unwrap();
            if depth_frame.width() != infrared_frame.width()
                || depth_frame.height() != infrared_frame.height()
            {
                panic!("Make sure depth and infrared frames are the same size");
            }

            let instance_number = 640 * 640;
            let mut infrared_data: Vec<f32> = vec![0.0; instance_number * 4];
            let mut depth_data: Vec<f32> = vec![0.0; instance_number];
            let max_depth = 4000.0; // 4m
            for x in 0..depth_frame.width() {
                for yy in 0..depth_frame.height() {
                    match depth_frame.get_unchecked(x, yy) {
                        realsense_rust::frame::PixelKind::Z16 { depth } => {
                            let normalized = (*depth as f32 / max_depth).clamp(0.0, 1.0);
                            if normalized > 0.05 {
                                depth_data[x * 640 + yy] = (1.0 - normalized) * 4.0;
                                match infrared_frame.get_unchecked(x, yy) {
                                    realsense_rust::frame::PixelKind::Y8 { y } => {
                                        let base_index = (x * 640 + yy) * 4;
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

            // Make this better
            // What happens if there no frames?
            let gl = frame.gl().expect("Can't get GL from frame");

            // Update instances height
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instance_height_vbo));
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

        // Get the OpenGL context from the frame
        let gl = frame.gl().expect("Can't get GL from frame");

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
            gl.clear_color(1.0, 0.3, 0.3, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            // Enable depth testing
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS); // Default: Pass if fragment is closer

            // Enable face culling
            //gl.enable(glow::CULL_FACE);
            //gl.cull_face(glow::BACK); // Cull back faces (default)
            //gl.front_face(glow::CW); // Counter-clockwise faces are "front"

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

        self.previous_frames = frames;

        egui_ctx.request_repaint();
    }
}

///
fn start_pipeline(
    devices: Vec<realsense_rust::device::Device>,
    pipeline: realsense_rust::pipeline::InactivePipeline,
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
            640,
            0,
            realsense_rust::kind::Rs2Format::Z16,
            30,
        )
        .expect("Failed to enable depth stream")
        .enable_stream(
            realsense_rust::kind::Rs2StreamKind::Infrared,
            Some(1),
            640,
            0,
            realsense_rust::kind::Rs2Format::Y8,
            30,
        )
        .expect("Failed to enable color stream");

    let pipeline = pipeline
        .start(Some(config))
        .expect("Failed to start pipeline");

    for mut sensor in pipeline.profile().device().sensors() {
        if sensor.supports_option(realsense_rust::kind::Rs2Option::EmitterOnOff) {
            sensor
                .set_option(realsense_rust::kind::Rs2Option::EmitterOnOff, 1.0)
                .expect("Failed to set option: EmitterOnOff");
        }
        if sensor.supports_option(realsense_rust::kind::Rs2Option::EnableAutoExposure) {
            sensor
                .set_option(realsense_rust::kind::Rs2Option::EnableAutoExposure, 0.0)
                .expect("Failed to set option: EnableAutoExposure");
        }
        //if sensor.supports_option(realsense_rust::kind::Rs2Option::EmitterEnabled) {
        //    sensor
        //        .set_option(realsense_rust::kind::Rs2Option::EmitterEnabled, 0.0)
        //        .expect("Failed to set option: EmitterEnabled");
        //}
    }

    pipeline
}

///
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
