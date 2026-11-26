use chrono::{DateTime, Local, Utc};
use egui::{Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Ui};

use crate::controller::{AppCommand, AppController, UiSnapshot};
use crate::models::{ConnectionStatus, NoiseControlMode};
use crate::ui::gl_stage::HeadphoneGlStage;
use crate::ui::theme::MurmurPalette;
use crate::ui::widgets::{
    acoustic_mode_dock, ambient_precision_scrubber, battery_telemetry_strip, color_alpha,
    font_bold, murmur_card, pill_button, quick_control_tile, section_eyebrow, setting_toggle_row,
};

pub fn render_studio_room(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
    gl_stage: &HeadphoneGlStage,
    open_save_scene_modal: &mut bool,
) {
    let wide = ui.available_width() >= 700.0;

    if wide {
        let total_w = ui.available_width();
        let left_w = (total_w * 0.44).clamp(290.0, 420.0);
        let right_w = (total_w - left_w - 18.0).max(320.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            ui.allocate_ui_with_layout(
                egui::vec2(left_w, ui.available_height()),
                Layout::top_down(egui::Align::Center),
                |ui| {
                    render_stage_column(ui, p, snap, controller, gl_stage);
                },
            );
            ui.allocate_ui_with_layout(
                egui::vec2(right_w, ui.available_height()),
                Layout::top_down(egui::Align::Min),
                |ui| {
                    render_desk_column(ui, p, snap, controller, open_save_scene_modal);
                },
            );
        });
    } else {
        render_stage_column(ui, p, snap, controller, gl_stage);
        ui.add_space(12.0);
        render_desk_column(ui, p, snap, controller, open_save_scene_modal);
    }
}

fn render_stage_column(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
    gl_stage: &HeadphoneGlStage,
) {
    let is_offline = snap.connection_status == ConnectionStatus::Offline;
    let mode_accent = match snap.headphone.noise_control.mode {
        NoiseControlMode::NoiseCancelling => p.accent_anc,
        NoiseControlMode::AmbientSound => p.accent_ambient,
        NoiseControlMode::Off => p.text_secondary,
    };

    // Frameless 3D Spinning PBR Showcase floating directly in the room
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(&snap.headphone.model_name)
                .font(font_bold(20.0))
                .color(p.text_primary),
        );
        let subtitle = match snap.connection_status {
            ConnectionStatus::Connected => format!(
                "Connected · {} · Drag to spin",
                snap.headphone.protocol_version.label()
            ),
            ConnectionStatus::Connecting => "Connecting over Bluetooth…".to_string(),
            ConnectionStatus::Offline => {
                if let Some(ts) = snap.last_seen_ms {
                    if let Some(dt) = DateTime::<Utc>::from_timestamp_millis(ts) {
                        let local: DateTime<Local> = dt.into();
                        format!("Offline · Last seen {}", local.format("%b %d %H:%M"))
                    } else {
                        "Offline · Cached State".to_string()
                    }
                } else {
                    "Offline · Ready to pair".to_string()
                }
            }
        };
        ui.label(RichText::new(subtitle).size(11.5).color(p.text_secondary));
    });

    gl_stage.draw(
        ui,
        p,
        snap.active_finish(),
        snap.headphone.form_factor(),
        mode_accent,
        is_offline,
        245.0,
    );

    battery_telemetry_strip(
        ui,
        p,
        &snap.headphone.battery,
        snap.battery_projection.remaining_minutes,
        snap.battery_projection.is_calibrated,
        snap.headphone.codec.label(),
    );

    // Offline warning + queued sync card
    if is_offline {
        ui.add_space(12.0);
        Frame::NONE
            .fill(p.warning_bg)
            .stroke(Stroke::new(1.0_f32, color_alpha(p.warning_border, 0.55)))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Headphones Not Connected")
                        .font(font_bold(13.0))
                        .color(p.accent_ambient),
                );
                ui.label(
                    RichText::new(
                        "Any changes you make now will be saved and pushed automatically next time you connect.",
                    )
                    .size(11.5)
                    .color(p.text_primary),
                );
                if let Some(hint) = &snap.connection_hint {
                    ui.add_space(3.0);
                    ui.label(RichText::new(hint).size(11.0).color(p.text_secondary));
                }
                if let Some(pending) = &snap.pending_changes {
                    let labels = pending.pending_labels();
                    if !labels.is_empty() {
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            for lbl in labels {
                                Frame::NONE
                                    .fill(p.surface_card)
                                    .stroke(Stroke::new(1.0_f32, p.hairline))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(Margin::symmetric(7, 3))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(lbl).size(11.0).color(p.text_primary),
                                        );
                                    });
                            }
                            if pill_button(ui, p, "Discard", false, p.danger) {
                                controller.send(AppCommand::DiscardPendingChanges);
                            }
                        });
                    }
                }
            });
    }

    if let Some(notice) = &snap.fallback_notice {
        ui.add_space(10.0);
        murmur_card(p, Some(p.accent_ambient)).show(ui, |ui| {
            ui.label(
                RichText::new(&notice.title)
                    .font(font_bold(12.5))
                    .color(p.accent_ambient),
            );
            ui.label(RichText::new(&notice.reason).size(11.5).color(p.text_secondary));
            if notice.can_retry_control_channel {
                ui.add_space(6.0);
                if pill_button(ui, p, "Retry Link", false, p.accent_ambient) {
                    controller.send(AppCommand::ConnectDevice(
                        snap.headphone.bluetooth_address.clone(),
                    ));
                }
            }
        });
    }

    if !snap.vault_diffs.is_empty() {
        ui.add_space(10.0);
        Frame::NONE
            .fill(color_alpha(p.accent_anc, 0.12))
            .stroke(Stroke::new(1.0_f32, color_alpha(p.accent_anc, 0.4)))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Settings differ from Vault")
                                .font(font_bold(12.5))
                                .color(p.text_primary),
                        );
                        ui.label(
                            RichText::new(snap.vault_diffs.join(" · "))
                                .size(11.0)
                                .color(p.text_secondary),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if pill_button(ui, p, "Restore", true, p.accent_anc) {
                            controller.send(AppCommand::RestoreVault);
                        }
                    });
                });
            });
    }
}

fn render_desk_column(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
    open_save_scene_modal: &mut bool,
) {
    let caps = snap.headphone.capabilities();
    let nc = &snap.headphone.noise_control;

    if let Some(alert) = &snap.headphone.pending_alert {
        murmur_card(p, Some(p.accent_ambient)).show(ui, |ui| {
            ui.label(
                RichText::new("Headphone Confirmation")
                    .font(font_bold(13.0))
                    .color(p.accent_ambient),
            );
            ui.label(RichText::new(alert.message()).size(12.0).color(p.text_primary));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if alert.needs_answer {
                    if pill_button(ui, p, "Confirm", true, p.accent_anc) {
                        controller.send(AppCommand::RespondToAlert {
                            message_type: alert.message_type,
                            accept: true,
                        });
                    }
                    if pill_button(ui, p, "Cancel", false, p.text_secondary) {
                        controller.send(AppCommand::RespondToAlert {
                            message_type: alert.message_type,
                            accept: false,
                        });
                    }
                } else if pill_button(ui, p, "Dismiss", false, p.text_secondary) {
                    controller.send(AppCommand::DismissAlert);
                }
            });
        });
        ui.add_space(10.0);
    }

    if caps.supports_noise_control {
        murmur_card(p, None).show(ui, |ui| {
            section_eyebrow(ui, p, "Acoustic Control");
            ui.add_space(8.0);
            if let Some(next_mode) = acoustic_mode_dock(ui, p, nc.mode) {
                controller.send(AppCommand::SetNoiseControl {
                    mode: next_mode,
                    ambient_level: None,
                    focus_on_voice: None,
                    dragging: false,
                });
            }

            if nc.mode == NoiseControlMode::NoiseCancelling {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Actively cancelling room, fan, and low-frequency rumble.")
                        .size(11.5)
                        .color(p.text_secondary),
                );
            } else if nc.mode == NoiseControlMode::Off {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Acoustic processing paused · passive earcup isolation only.")
                        .size(11.5)
                        .color(p.text_secondary),
                );
            } else if nc.mode == NoiseControlMode::AmbientSound {
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Ambient Passthrough")
                            .font(font_bold(13.0))
                            .color(p.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} / 20", nc.ambient_level))
                                .font(font_bold(13.0))
                                .color(p.accent_ambient),
                        );
                    });
                });
                ui.add_space(4.0);

                if let Some((lvl, dragging)) =
                    ambient_precision_scrubber(ui, p, nc.ambient_level)
                {
                    controller.send(AppCommand::SetNoiseControl {
                        mode: NoiseControlMode::AmbientSound,
                        ambient_level: Some(lvl),
                        focus_on_voice: None,
                        dragging,
                    });
                }

                ui.add_space(10.0);
                if let Some(voice) = setting_toggle_row(
                    ui,
                    p,
                    "Focus on Voice",
                    "Let speech come through clearly while reducing hum",
                    nc.focus_on_voice,
                    p.accent_ambient,
                ) {
                    controller.send(AppCommand::SetNoiseControl {
                        mode: NoiseControlMode::AmbientSound,
                        ambient_level: None,
                        focus_on_voice: Some(voice),
                        dragging: false,
                    });
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    section_eyebrow(ui, p, "Quick Scenes");
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if pill_button(ui, p, "+ Save", false, p.accent_ambient) {
                            *open_save_scene_modal = true;
                        }
                    });
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for scene in &snap.ambient_scenes {
                        let active = nc.ambient_level == scene.level
                            && nc.focus_on_voice == scene.focus_on_voice;
                        if pill_button(ui, p, &scene.name, active, p.accent_ambient) {
                            controller.send(AppCommand::SetNoiseControl {
                                mode: NoiseControlMode::AmbientSound,
                                ambient_level: Some(scene.level),
                                focus_on_voice: Some(scene.focus_on_voice),
                                dragging: false,
                            });
                        }
                        if !matches!(scene.id.as_str(), "street" | "office" | "convo")
                            && pill_button(ui, p, "×", false, p.danger)
                        {
                            controller.send(AppCommand::DeleteAmbientScene(scene.id.clone()));
                        }
                    }
                });
            }
        });
        ui.add_space(12.0);
    }

    // 2x2 Bento Quick Controls
    let mut tiles: Vec<(&str, String, bool, Color32, AppCommand)> = Vec::new();
    if caps.supports_speak_to_chat {
        let on = snap.headphone.speak_to_chat;
        tiles.push((
            "Speak-to-Chat",
            if on { "Active" } else { "Off" }.to_string(),
            on,
            p.accent_anc,
            AppCommand::SetSpeakToChat(!on),
        ));
    }
    if caps.supports_dsee {
        let on = snap.headphone.dsee_extreme;
        tiles.push((
            "DSEE Extreme",
            if on { "Upscaling" } else { "Off" }.to_string(),
            on,
            p.accent_anc,
            AppCommand::SetDsee(!on),
        ));
    }
    if caps.supports_touch_panel {
        let on = snap.headphone.touch_panel_enabled();
        tiles.push((
            "Touch Panel",
            if on { "Enabled" } else { "Locked" }.to_string(),
            on,
            p.accent_anc,
            AppCommand::SetTouchPanel(!on),
        ));
    }
    if caps.supports_wearing_detection {
        let on = snap.headphone.wearing_detection;
        tiles.push((
            "Wear Detection",
            if on { "Auto-Pause" } else { "Off" }.to_string(),
            on,
            p.accent_anc,
            AppCommand::SetWearingDetection(!on),
        ));
    }

    if !tiles.is_empty() {
        section_eyebrow(ui, p, "Quick Controls");
        ui.add_space(6.0);
        for chunk in tiles.chunks(2) {
            ui.columns(2, |cols| {
                for (i, (title, sub, active, accent, cmd)) in chunk.iter().enumerate() {
                    if quick_control_tile(&mut cols[i], p, title, sub, *active, *accent) {
                        controller.send(cmd.clone());
                    }
                }
            });
            ui.add_space(8.0);
        }
    }
}
