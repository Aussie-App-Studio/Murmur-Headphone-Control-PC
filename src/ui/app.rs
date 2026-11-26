use egui::{
    Align, Area, CentralPanel, Color32, CornerRadius, Frame, Layout, Margin, Mesh, Order, Pos2,
    RichText, ScrollArea, Stroke, TopBottomPanel, ViewportCommand,
};

use crate::controller::{AppCommand, AppController};
use crate::models::{AppThemeMode, ConnectionStatus, NoiseControlMode};
use crate::platform::apply_windows_titlebar_theme;
use crate::tray::MurmurTray;
use crate::ui::control_view::render_studio_room;
use crate::ui::device_view::render_settings_room;
use crate::ui::gl_stage::HeadphoneGlStage;
use crate::ui::sound_view::render_sound_room;
use crate::ui::theme::{apply_theme, install_windows_fonts, resolve_palette, MurmurPalette};
use crate::ui::widgets::{color_alpha, font_bold, pill_button, segmented_nav};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Room {
    Control = 0,
    Sound = 1,
    Settings = 2,
}

impl Room {
    fn from_index(i: usize) -> Self {
        match i {
            1 => Self::Sound,
            2 => Self::Settings,
            _ => Self::Control,
        }
    }
}

enum SaveModalKind {
    None,
    AmbientScene,
    EqProfile,
}

pub struct MurmurApp {
    controller: AppController,
    tray: Option<MurmurTray>,
    gl_stage: HeadphoneGlStage,
    active_room: Room,
    save_modal: SaveModalKind,
    modal_input: String,
    last_theme_check: std::time::Instant,
    cached_palette: MurmurPalette,
    applied_titlebar_dark: Option<bool>,
    window_hidden: bool,
    cached_snap: crate::controller::UiSnapshot,
    cached_snap_version: u64,
    screenshot_dir: Option<std::path::PathBuf>,
    screenshot_step: u32,
}

impl MurmurApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        controller: AppController,
        tray: Option<MurmurTray>,
    ) -> Self {
        install_windows_fonts(&cc.egui_ctx);
        controller.set_egui_ctx(cc.egui_ctx.clone());
        if let Some(t) = &tray {
            t.attach_egui_ctx(cc.egui_ctx.clone());
        }
        let cached_snap = controller.snapshot();
        let cached_snap_version = controller.snapshot_version();
        let palette = resolve_palette(cached_snap.theme_mode);
        apply_theme(&cc.egui_ctx, &palette);
        let gl_stage = HeadphoneGlStage::new(cc.gl.as_ref());

        apply_windows_titlebar_theme(
            palette.is_dark,
            (palette.canvas.r(), palette.canvas.g(), palette.canvas.b()),
        );

        Self {
            controller,
            tray,
            gl_stage,
            active_room: Room::Control,
            save_modal: SaveModalKind::None,
            modal_input: String::new(),
            last_theme_check: std::time::Instant::now(),
            cached_palette: palette,
            applied_titlebar_dark: None,
            window_hidden: false,
            cached_snap,
            cached_snap_version,
            screenshot_dir: std::env::var_os("MURMUR_SCREENSHOT_DIR")
                .map(std::path::PathBuf::from),
            screenshot_step: 0,
        }
    }
}

impl eframe::App for MurmurApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = self.cached_palette.canvas;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(tray) = &mut self.tray {
            if tray.poll_events(&self.controller, ctx) {
                self.window_hidden = false;
            }
        }

        let ver = self.controller.snapshot_version();
        if ver != self.cached_snap_version {
            self.cached_snap = self.controller.snapshot();
            self.cached_snap_version = ver;
            if let Some(tray) = &mut self.tray {
                tray.update_tooltip(&self.cached_snap);
            }
        }
        let snap = &self.cached_snap;

        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested && snap.minimize_to_tray {
            let should_quit = self.tray.as_ref().is_some_and(|t| t.should_quit);
            if !should_quit && self.tray.is_some() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                self.window_hidden = true;
                return;
            }
        }
        if self.window_hidden || ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
            return;
        }

        if self.last_theme_check.elapsed().as_secs_f32() >= 2.0 {
            self.last_theme_check = std::time::Instant::now();
        }
        let forced_dark = self.screenshot_dir.is_some()
            && std::env::var_os("MURMUR_SCREENSHOT_DARK").is_some();
        let next_palette = if forced_dark {
            resolve_palette(AppThemeMode::StudioDark)
        } else {
            resolve_palette(snap.theme_mode)
        };
        if next_palette.is_dark != self.cached_palette.is_dark
            || self.applied_titlebar_dark != Some(next_palette.is_dark)
        {
            self.cached_palette = next_palette;
            apply_theme(ctx, &self.cached_palette);
            apply_windows_titlebar_theme(
                next_palette.is_dark,
                (
                    next_palette.canvas.r(),
                    next_palette.canvas.g(),
                    next_palette.canvas.b(),
                ),
            );
            self.applied_titlebar_dark = Some(next_palette.is_dark);
        }
        let p = self.cached_palette;

        // Solid canvas background + subtle ambient top aura across the room
        let mode_accent = match snap.headphone.noise_control.mode {
            NoiseControlMode::NoiseCancelling => p.accent_anc,
            NoiseControlMode::AmbientSound => p.accent_ambient,
            NoiseControlMode::Off => p.text_secondary,
        };
        let aura_alpha = if snap.connection_status == ConnectionStatus::Offline {
            0.04
        } else if p.is_dark {
            0.13
        } else {
            0.09
        };
        let screen = ctx.screen_rect();
        let bg_painter = ctx.layer_painter(egui::LayerId::background());
        bg_painter.rect_filled(screen, 0.0, p.canvas);
        draw_radial_top_aura(&bg_painter, screen, color_alpha(mode_accent, aura_alpha));

        // Floating Top Bar
        TopBottomPanel::top("studio_header")
            .frame(
                Frame::NONE
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(Margin::symmetric(22, 12)),
            )
            .show(ctx, |ui| {
                let wide = ui.available_width() >= 600.0;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Murmur")
                            .font(font_bold(18.0))
                            .color(p.text_primary),
                    );
                    let (dot_col, status_txt) = match snap.connection_status {
                        ConnectionStatus::Connected => (p.accent_anc, "Live"),
                        ConnectionStatus::Connecting => (p.accent_ambient, "Connecting"),
                        ConnectionStatus::Offline => (p.danger, "Offline"),
                    };
                    Frame::NONE
                        .fill(p.surface_recessed)
                        .stroke(Stroke::new(1.0_f32, p.hairline))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(9, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                let (r, _) =
                                    ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
                                ui.painter().circle_filled(r.center(), 3.5, dot_col);
                                ui.label(
                                    RichText::new(status_txt)
                                        .font(font_bold(11.0))
                                        .color(p.text_secondary),
                                );
                            });
                        });

                    if wide {
                        ui.add_space(16.0);
                        if let Some(idx) = segmented_nav(
                            ui,
                            &p,
                            &["Control", "Sound", "Settings"],
                            self.active_room as usize,
                        ) {
                            self.active_room = Room::from_index(idx);
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let next_mode = match snap.theme_mode {
                            AppThemeMode::System => AppThemeMode::StudioDark,
                            AppThemeMode::StudioDark => AppThemeMode::Light,
                            AppThemeMode::Light => AppThemeMode::System,
                        };
                        let theme_lbl = match snap.theme_mode {
                            AppThemeMode::System => "Auto Theme",
                            AppThemeMode::StudioDark => "Dark",
                            AppThemeMode::Light => "Light",
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(theme_lbl)
                                        .size(11.5)
                                        .color(p.text_secondary),
                                )
                                .fill(p.surface_recessed)
                                .stroke(Stroke::new(1.0_f32, p.hairline))
                                .corner_radius(CornerRadius::same(10)),
                            )
                            .clicked()
                        {
                            self.controller.send(AppCommand::SetThemeMode(next_mode));
                        }

                        if let Some(pending) = &snap.pending_changes {
                            let count = pending.pending_count();
                            if count > 0 {
                                Frame::NONE
                                    .fill(p.warning_bg)
                                    .stroke(Stroke::new(
                                        1.0_f32,
                                        color_alpha(p.warning_border, 0.6),
                                    ))
                                    .corner_radius(CornerRadius::same(10))
                                    .inner_margin(Margin::symmetric(9, 4))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!("{} queued", count))
                                                .font(font_bold(11.0))
                                                .color(p.accent_ambient),
                                        );
                                    });
                            }
                        }
                    });
                });

                if !wide {
                    ui.add_space(8.0);
                    if let Some(idx) = segmented_nav(
                        ui,
                        &p,
                        &["Control", "Sound", "Settings"],
                        self.active_room as usize,
                    ) {
                        self.active_room = Room::from_index(idx);
                    }
                }
            });

        if let Some(msg) = &snap.status_message {
            TopBottomPanel::bottom("status_bar")
                .frame(
                    Frame::NONE
                        .fill(p.surface_card)
                        .stroke(Stroke::new(1.0_f32, p.hairline))
                        .inner_margin(Margin::symmetric(16, 7)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(msg).size(12.0).color(p.text_primary));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if pill_button(ui, &p, "Dismiss", false, p.text_secondary) {
                                self.controller.send(AppCommand::ClearStatusMessage);
                            }
                        });
                    });
                });
        }

        // Main Room Canvas
        CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(Margin::symmetric(22, 6)),
            )
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    match self.active_room {
                        Room::Control => {
                            let mut open_scene = false;
                            render_studio_room(
                                ui,
                                &p,
                                &snap,
                                &self.controller,
                                &self.gl_stage,
                                &mut open_scene,
                            );
                            if open_scene {
                                self.save_modal = SaveModalKind::AmbientScene;
                                self.modal_input.clear();
                            }
                        }
                        Room::Sound => {
                            let mut open_eq = false;
                            render_sound_room(ui, &p, &snap, &self.controller, &mut open_eq);
                            if open_eq {
                                self.save_modal = SaveModalKind::EqProfile;
                                self.modal_input.clear();
                            }
                        }
                        Room::Settings => {
                            render_settings_room(ui, &p, &snap, &self.controller)
                        }
                    }
                    ui.add_space(14.0);
                });
            });

        // Clean Modal Dialog for saving scenes / EQ profiles (instead of raw inline TextEdit)
        if !matches!(self.save_modal, SaveModalKind::None) {
            let scrim = ctx.layer_painter(egui::LayerId::new(Order::Middle, "modal_scrim".into()));
            scrim.rect_filled(screen, 0.0, Color32::from_black_alpha(140));

            let title = match self.save_modal {
                SaveModalKind::AmbientScene => "Save Ambient Scene",
                SaveModalKind::EqProfile => "Save EQ Profile",
                SaveModalKind::None => "",
            };
            Area::new("save_modal_area".into())
                .order(Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    Frame::NONE
                        .fill(p.surface_card)
                        .stroke(Stroke::new(1.0_f32, p.hairline))
                        .corner_radius(CornerRadius::same(20))
                        .inner_margin(Margin::same(20))
                        .show(ui, |ui| {
                            ui.set_width(300.0);
                            ui.label(RichText::new(title).font(font_bold(16.0)).color(p.text_primary));
                            ui.add_space(8.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.modal_input)
                                    .hint_text("Enter a name…")
                                    .desired_width(290.0),
                            );
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                if pill_button(ui, &p, "Save", true, p.accent_anc)
                                    && !self.modal_input.trim().is_empty()
                                {
                                    match self.save_modal {
                                        SaveModalKind::AmbientScene => {
                                            self.controller.send(AppCommand::SaveAmbientScene {
                                                name: self.modal_input.clone(),
                                                level: snap.headphone.noise_control.ambient_level,
                                                focus_on_voice: snap
                                                    .headphone
                                                    .noise_control
                                                    .focus_on_voice,
                                            });
                                        }
                                        SaveModalKind::EqProfile => {
                                            self.controller.send(AppCommand::SaveCustomEqProfile {
                                                name: self.modal_input.clone(),
                                                clear_bass: snap.headphone.equalizer.clear_bass,
                                                bands: snap.headphone.equalizer.bands.clone(),
                                            });
                                        }
                                        SaveModalKind::None => {}
                                    }
                                    self.save_modal = SaveModalKind::None;
                                    self.modal_input.clear();
                                }
                                if pill_button(ui, &p, "Cancel", false, p.text_secondary) {
                                    self.save_modal = SaveModalKind::None;
                                    self.modal_input.clear();
                                }
                            });
                        });
                });
        }

        // Optional automated screenshot capture when MURMUR_SCREENSHOT_DIR is set
        if let Some(dir) = self.screenshot_dir.clone() {
            let captured = ctx.input(|i| {
                i.events.iter().find_map(|ev| match ev {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = captured {
                let name = match self.screenshot_step {
                    0 => "room_control.bmp",
                    1 => "room_sound.bmp",
                    _ => "room_settings.bmp",
                };
                write_color_image_bmp(&dir.join(name), &img);
                self.screenshot_step += 1;
                match self.screenshot_step {
                    1 => self.active_room = Room::Sound,
                    2 => self.active_room = Room::Settings,
                    _ => {
                        if let Some(tray) = &mut self.tray {
                            tray.should_quit = true;
                        }
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                        return;
                    }
                }
                ctx.request_repaint();
            } else {
                // Wait a few frames for fonts/GL/layout to settle before grabbing
                static FRAME_COUNTER: std::sync::atomic::AtomicU32 =
                    std::sync::atomic::AtomicU32::new(0);
                let fc = FRAME_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if fc % 6 == 4 {
                    ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
                } else {
                    ctx.request_repaint();
                }
            }
        }
    }
}

fn write_color_image_bmp(path: &std::path::Path, img: &egui::ColorImage) {
    let w = img.size[0] as u32;
    let h = img.size[1] as u32;
    let row_bytes = w * 4;
    let pixel_bytes = row_bytes * h;
    let file_size = 54 + pixel_bytes;

    let mut buf = Vec::with_capacity(file_size as usize);
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&54u32.to_le_bytes());
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&w.to_le_bytes());
    buf.extend_from_slice(&(-(h as i32)).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&32u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&pixel_bytes.to_le_bytes());
    buf.extend_from_slice(&[0u8; 16]);
    for px in &img.pixels {
        buf.extend_from_slice(&[px.b(), px.g(), px.r(), px.a()]);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, buf);
}

fn draw_radial_top_aura(painter: &egui::Painter, rect: egui::Rect, center_color: Color32) {
    let cx = rect.center().x;
    let cy = rect.top();
    let radius = (rect.width() * 0.65).max(300.0);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(Pos2::new(cx, cy), center_color);
    let edge_col = Color32::TRANSPARENT;
    let segs = 24usize;
    for i in 0..=segs {
        let angle = (i as f32 / segs as f32) * std::f32::consts::PI;
        let px = cx + radius * angle.cos();
        let py = cy + radius * 0.45 * angle.sin();
        mesh.colored_vertex(Pos2::new(px, py), edge_col);
        if i > 0 {
            mesh.add_triangle(0, i as u32, (i + 1) as u32);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}
