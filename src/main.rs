#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod battery;
mod catalog;
mod controller;
mod models;
mod platform;
mod protocol;
mod storage;
mod tray;
mod ui;

use std::sync::Arc;

use controller::AppController;
use tray::MurmurTray;
use ui::MurmurApp;

fn build_app_icon() -> egui::IconData {
    let size = 64usize;
    let mut rgba = vec![0u8; size * size * 4];
    let cx = 31.5_f32;
    let cy = 31.5_f32;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32;
            let fy = y as f32;
            // Rounded square dark studio backdrop
            let ax = (fx - cx).abs();
            let ay = (fy - cy).abs();
            let qx = (ax - 18.0).max(0.0);
            let qy = (ay - 18.0).max(0.0);
            let dist_box = (qx * qx + qy * qy).sqrt() - 12.0;

            if dist_box <= 0.0 {
                let idx = (y * size + x) * 4;
                rgba[idx] = 0x16;
                rgba[idx + 1] = 0x1B;
                rgba[idx + 2] = 0x22;
                rgba[idx + 3] = 0xFF;

                let dx = fx - cx;
                let dy = fy - (cy + 2.0);
                let r = (dx * dx + dy * dy).sqrt();
                let in_arc = fy <= cy + 4.0 && (14.5..=20.0).contains(&r);
                let in_left = (11.5..=19.0).contains(&fx) && (cy..=48.0).contains(&fy);
                let in_right = (44.0..=51.5).contains(&fx) && (cy..=48.0).contains(&fy);
                if in_arc || in_left || in_right {
                    rgba[idx] = 0x2C;
                    rgba[idx + 1] = 0xB5;
                    rgba[idx + 2] = 0xA6;
                    rgba[idx + 3] = 0xFF;
                }
            }
        }
    }

    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    let controller = AppController::start();
    let tray = MurmurTray::new();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Murmur — Headphone Control")
            .with_icon(Arc::new(build_app_icon()))
            .with_inner_size([960.0, 660.0])
            .with_min_inner_size([460.0, 540.0]),
        depth_buffer: 24,
        multisampling: 4,
        ..Default::default()
    };

    eframe::run_native(
        "Murmur — Headphone Control",
        options,
        Box::new(move |cc| Ok(Box::new(MurmurApp::new(cc, controller, tray)))),
    )
}
