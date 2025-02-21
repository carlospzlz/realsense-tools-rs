use eframe::egui;
use eframe::glow;
use eframe::glow::HasContext;

struct MyApp;

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Get the OpenGL context from the frame
        if let Some(gl) = frame.gl() {
            unsafe {
                gl.clear_color(1.0, 0.3, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            }
        }
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.label("There we go");
        });
        egui::Window::new("Floating Window")
            .resizable(true) // Allows resizing
            .collapsible(true) // Can collapse
            .show(ctx, |ui| {
                ui.label("This is a floating window!");
            });
    }
}

fn main() {
    let options = eframe::NativeOptions {
        depth_buffer: 1, // Important for 3D rendering
        ..Default::default()
    };

    eframe::run_native("Glow Example", options, Box::new(|_cc| Ok(Box::new(MyApp))));
}
