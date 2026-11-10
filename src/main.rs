#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod engine;
mod fonts;
mod i18n;
mod model;
mod ui;

fn main() -> eframe::Result<()> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .expect("the bundled app icon must be a PNG");
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_icon(icon)
        .with_inner_size([1160.0, 800.0])
        .with_min_inner_size([800.0, 560.0])
        .with_decorations(false)
        .with_transparent(cfg!(target_os = "macos"));
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "ESP32 SuperFlash",
        options,
        Box::new(|creation| Ok(Box::new(app::App::new(&creation.egui_ctx)))),
    )
}

#[cfg(test)]
mod verification;
