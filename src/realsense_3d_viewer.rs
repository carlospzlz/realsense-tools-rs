use eframe::egui;
use eframe::glow;
use eframe::glow::HasContext;
use std::collections::HashSet;
use std::time::Duration;

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

        let vertices: &[f32] = &[
            // Positions        // Colors
            -0.5, -0.5, -0.5, 1.0, 0.0, 0.0, // Red
            0.5, -0.5, -0.5, 0.0, 1.0, 0.0, // Green
            0.5, 0.5, -0.5, 0.0, 0.0, 1.0, // Blue
            -0.5, 0.5, -0.5, 1.0, 1.0, 0.0, // Yellow
            -0.5, -0.5, 0.5, 1.0, 0.0, 1.0, // Magenta
            0.5, -0.5, 0.5, 0.0, 1.0, 1.0, // Cyan
            0.5, 0.5, 0.5, 1.0, 1.0, 1.0, // White
            -0.5, 0.5, 0.5, 0.5, 0.5, 0.5, // Gray
        ];

        let indices: &[u32] = &[
            0, 1, 2, 2, 3, 0, // Front face
            4, 5, 6, 6, 7, 4, // Back face
            4, 5, 1, 1, 0, 4, // Bottom face
            7, 6, 2, 2, 3, 7, // Top face
            4, 0, 3, 3, 7, 4, // Left face
            5, 1, 2, 2, 6, 5, // Right face
        ];

        unsafe {
            // VAO to store: vertex attrib pointers, VBOs and indexes
            let vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(vao));

            // Prepare OpenGL buffers for vertex and index data
            let vertex_buffer = gl.create_buffer().unwrap();
            let index_buffer = gl.create_buffer().unwrap();

            // Load the vertex data into the buffer
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &bytemuck::cast_slice(vertices),
                glow::STATIC_DRAW,
            );

            // Load the index data into the index buffer
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                &bytemuck::cast_slice(indices),
                glow::STATIC_DRAW,
            );

            // Set up shaders
            let program = create_shader_program(gl);

            // Set up vertex attributes (position, color)
            let position_location = gl.get_attrib_location(program, "position").unwrap() as u32;
            gl.vertex_attrib_pointer_f32(
                position_location,
                3,
                glow::FLOAT,
                false,
                6 * std::mem::size_of::<f32>() as i32,
                0,
            );
            gl.enable_vertex_attrib_array(position_location);

            let color_location = gl.get_attrib_location(program, "color").unwrap() as u32;
            gl.vertex_attrib_pointer_f32(
                color_location,
                3,
                glow::FLOAT,
                false,
                6 * std::mem::size_of::<f32>() as i32,
                3 * std::mem::size_of::<f32>() as i32,
            );
            gl.enable_vertex_attrib_array(color_location);

            // Apply rotation using a transformation matrix
            //let rotation_matrix = nalgebra::Matrix4::from_euler_angles(PI / 180.0 * 45.0, PI / 180.0 * 45.0, 0.0); // Rotating 45 degrees
            //let rotation_uniform = gl.get_uniform_location(program, "rotation").unwrap();
            //gl.uniform_matrix4_f32_slice(Some(&rotation_uniform), false, rotation_matrix.as_slice());

            // Unbind VAO
            gl.bind_vertex_array(None);

            Self {
                realsense_ctx,
                pipeline: pipeline,
                program: program,
                vao: vao,
                angle: 0.0,
                translation: glam::Vec3::new(0.0, 0.0, 0.0),
                rotation: glam::Vec2::new(0.0, 0.0),
                last_mouse_pos: None,
            }
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Get frames
        let timeout = Duration::from_millis(50);
        let frames = match self.pipeline.wait(Some(timeout)) {
            Ok(frames) => Some(frames),
            Err(e) => {
                println!("{e}");
                None
            }
        };

        // Create buffer of colored depth 3D bars
        if let Some(frames) = frames {
            let depth_frames = frames.frames_of_type::<realsense_rust::frame::DepthFrame>();
            let color_frames = frames.frames_of_type::<realsense_rust::frame::ColorFrame>();
            if !depth_frames.is_empty() && !color_frames.is_empty() {
                let depth_frame = &depth_frames[0];
                let color_frame = &color_frames[0];
                if depth_frame.width() != color_frame.width()
                    || depth_frame.height() != color_frame.height()
                {
                    panic!("Make sure depth and color frames are the same size");
                }
                let max_value = 4000.0; // 4m
                let mut img =
                    image::RgbImage::new(depth_frame.width() as u32, depth_frame.height() as u32);
                for (x, y, pixel) in img.enumerate_pixels_mut() {
                    match depth_frame.get_unchecked(x as usize, y as usize) {
                        realsense_rust::frame::PixelKind::Z16 { depth } => {
                            let normalized = *depth as f32 / max_value;
                            //println!("{}", normalized);
                        }
                        _ => panic!("Depth type is wrong!"),
                    }
                }
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

        let mut model_matrices = Vec::new();

        for x in 0..4 {
            for z in 0..4 {
                // Scale cube height
                let scale = glam::Mat4::from_scale(glam::Vec3::new(1.0, (x + z) as f32, 1.0));

                // Position the cube in the grid
                let translation = glam::Mat4::from_translation(glam::Vec3::new(x as f32, z as f32, 0.0));

                // Compute final transformation matrix
                let model = translation * scale;
                model_matrices.push(model);
            }
        }

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

            // Create a buffer for instance data
            let instance_vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_vbo));

            // Convert Mat4 data into a flat Vec<f32>
            let matrix_data: Vec<f32> = model_matrices.iter().flat_map(|m| m.to_cols_array()).collect();
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&matrix_data), glow::STATIC_DRAW);

            // Apply view projection matrix
            let uniform_location = gl.get_uniform_location(self.program, "viewProjection").unwrap();
            gl.uniform_matrix_4_f32_slice(
                Some(&uniform_location),
                false,
                view_projection.to_cols_array().as_slice(),
            );

            // Draw the cube
            gl.draw_elements(glow::TRIANGLES, 36, glow::UNSIGNED_INT, 0);
        }

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
        panic!("No RealSense device found!");
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
            realsense_rust::kind::Rs2StreamKind::Color,
            None,
            640,
            0,
            realsense_rust::kind::Rs2Format::Bgr8,
            30,
        )
        .expect("Failed to enable color stream");

    pipeline
        .start(Some(config))
        .expect("Failed to start pipeline")
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

//#layout(location = 2) in mat4 modelMatrix;
// modelMatrix
fn create_shader_program(gl: &glow::Context) -> glow::NativeProgram {
    unsafe {
        // Vertex shader
        let vertex_shader_src = r#"
            #version 330 core
            layout(location = 0) in vec3 position;
            layout(location = 1) in vec3 color;
            uniform mat4 viewProjection;
            out vec3 fragColor;
            void main() {
                gl_Position = viewProjection * vec4(position, 1.0);
                fragColor = color;
            }
        "#;
        let vertex_shader = compile_shader(gl, glow::VERTEX_SHADER, vertex_shader_src);

        // Fragment shader
        let fragment_shader_src = r#"
            #version 330 core
            in vec3 fragColor;
            out vec4 color;
            void main() {
                color = vec4(fragColor, 1.0);
            }
        "#;
        let fragment_shader = compile_shader(gl, glow::FRAGMENT_SHADER, fragment_shader_src);

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
