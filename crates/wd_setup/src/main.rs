//! Setup and calibration app. Runs outside the game and sends nothing to the wheel.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod app;

#[cfg(windows)]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Wheel setup")
            .with_inner_size([860.0, 600.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Wheel setup",
        options,
        Box::new(|creation| Ok(Box::new(app::SetupApp::new(creation.egui_ctx.clone())))),
    )
}

#[cfg(not(windows))]
fn main() {
    eprintln!("this app only runs on Windows");
}
