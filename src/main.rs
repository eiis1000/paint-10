#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
mod app;
#[cfg(not(target_arch = "wasm32"))]
mod file_dialogs;
#[cfg(not(target_arch = "wasm32"))]
mod icons;
#[cfg(not(target_arch = "wasm32"))]
mod native_options;
#[cfg(not(target_arch = "wasm32"))]
mod print_preview;
#[cfg(not(target_arch = "wasm32"))]
use paint_10::{document, integration, preferences, printing, project, raster_io, text};

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    if let Some(status) = file_dialogs::run_helper_if_requested() {
        std::process::exit(status);
    }
    let options = native_options::with_viewport(
        eframe::egui::ViewportBuilder::default()
            .with_title("Untitled - Paint 10")
            .with_app_id("paint-10")
            .with_inner_size([1180.0, 800.0])
            .with_min_inner_size([500.0, 400.0])
            .with_decorations(false),
    );
    eframe::run_native(
        "Paint 10",
        options,
        Box::new(|cc| Ok(Box::new(app::PaintApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
