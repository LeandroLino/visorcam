#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod album;
mod app;
mod devices;
mod live;
mod messages;
mod model;
mod net;
mod scene;
mod settings;
mod stream;
mod ui;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let screenshot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let check = screenshot.is_some();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("VisorCam")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([960.0, 640.0]);
    if check {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if check {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native("VisorCam", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot)))))
}
