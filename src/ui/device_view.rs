use egui::{CornerRadius, Frame, Layout, Margin, Rect, RichText, Stroke, Ui, Vec2};

use crate::controller::{AppCommand, AppController, UiSnapshot};
use crate::models::{
    AppThemeMode, AssignablePreset, AutoPowerOff, ConnectionStatus, HeadphoneAge, HeadphoneFinish,
    HeadphoneFinishOverride, NcAmbButtonCycle, QuickAccessService, SpeakToChatSensitivity,
    SpeakToChatTimeout, VoiceAssistant,
};
use crate::protocol::sony_v2::DeviceAction;
use crate::ui::theme::MurmurPalette;
use crate::ui::widgets::{
    finish_swatch_circle, font_bold, murmur_card, pill_button, section_eyebrow, setting_toggle_row,
};

pub fn render_settings_room(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
) {
    let wide = ui.available_width() >= 700.0;
    if wide {
        ui.columns(2, |cols| {
            render_left_column(&mut cols[0], p, snap, controller);
            render_right_column(&mut cols[1], p, snap, controller);
        });
    } else {
        render_left_column(ui, p, snap, controller);
        ui.add_space(12.0);
        render_right_column(ui, p, snap, controller);
    }
}

fn render_left_column(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
) {
    let caps = snap.headphone.capabilities();

    // Paired Windows Bluetooth Devices
    murmur_card(p, None).show(ui, |ui| {
        ui.horizontal(|ui| {
            section_eyebrow(ui, p, "Your Headphones");
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if pill_button(ui, p, "Refresh", false, p.accent_anc) {
                    controller.send(AppCommand::RefreshDevices);
                }
            });
        });
        ui.add_space(8.0);

        if snap.paired_devices.is_empty() {
            Frame::NONE
                .fill(p.surface_recessed)
                .stroke(Stroke::new(1.0_f32, p.hairline))
                .corner_radius(CornerRadius::same(13))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "No paired Bluetooth headphones found. Pair them in Windows Settings -> Bluetooth & devices, then click Refresh.",
                        )
                        .size(12.0)
                        .color(p.text_secondary),
                    );
                });
        } else {
            for dev in &snap.paired_devices {
                let is_active = snap.connection_status == ConnectionStatus::Connected
                    && snap
                        .headphone
                        .bluetooth_address
                        .eq_ignore_ascii_case(&dev.address);
                Frame::NONE
                    .fill(p.surface_recessed)
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_active { p.accent_anc } else { p.hairline },
                    ))
                    .corner_radius(CornerRadius::same(13))
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&dev.name)
                                        .font(font_bold(13.0))
                                        .color(p.text_primary),
                                );
                                let tag = if is_active {
                                    "Connected"
                                } else if dev.is_connected {
                                    "Ready on Windows"
                                } else {
                                    "Paired"
                                };
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        dev.detected_protocol.label(),
                                        tag
                                    ))
                                    .size(11.0)
                                    .color(if is_active {
                                        p.accent_anc
                                    } else {
                                        p.text_secondary
                                    }),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if is_active {
                                    if pill_button(ui, p, "Disconnect", false, p.danger) {
                                        controller.send(AppCommand::Disconnect);
                                    }
                                } else if pill_button(ui, p, "Connect", true, p.accent_anc) {
                                    controller.send(AppCommand::ConnectDevice(dev.address.clone()));
                                }
                            });
                        });
                    });
                ui.add_space(6.0);
            }
        }
    });

    // Multipoint Sources
    if caps.supports_paired_devices && !snap.headphone.paired_devices.is_empty() {
        ui.add_space(12.0);
        murmur_card(p, None).show(ui, |ui| {
            section_eyebrow(ui, p, "Audio Sources (Multipoint)");
            ui.add_space(8.0);
            for dev in &snap.headphone.paired_devices {
                Frame::NONE
                    .fill(p.surface_recessed)
                    .stroke(Stroke::new(1.0_f32, p.hairline))
                    .corner_radius(CornerRadius::same(13))
                    .inner_margin(Margin::symmetric(12, 9))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                let status = if dev.is_playing {
                                    "Playing Now"
                                } else if dev.is_connected {
                                    "Connected"
                                } else {
                                    "Paired"
                                };
                                ui.label(
                                    RichText::new(&dev.name)
                                        .font(font_bold(13.0))
                                        .color(p.text_primary),
                                );
                                ui.label(
                                    RichText::new(status)
                                        .size(11.0)
                                        .color(if dev.is_playing {
                                            p.accent_anc
                                        } else {
                                            p.text_secondary
                                        }),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if dev.is_connected {
                                    if pill_button(ui, p, "Play", dev.is_playing, p.accent_anc) {
                                        controller.send(AppCommand::SwitchAudioSource(
                                            dev.address.clone(),
                                        ));
                                    }
                                    let locked = snap
                                        .locked_playback_address
                                        .as_ref()
                                        .is_some_and(|m| m.eq_ignore_ascii_case(&dev.address));
                                    if pill_button(
                                        ui,
                                        p,
                                        if locked { "Locked" } else { "Lock" },
                                        locked,
                                        p.accent_ambient,
                                    ) {
                                        controller.send(AppCommand::ToggleLockPlaybackSource(
                                            dev.address.clone(),
                                        ));
                                    }
                                } else if pill_button(ui, p, "Connect", false, p.accent_anc) {
                                    controller.send(AppCommand::PerformDeviceAction(
                                        DeviceAction::Connect,
                                        dev.address.clone(),
                                    ));
                                }
                            });
                        });
                    });
                ui.add_space(6.0);
            }
        });
    }

    // Settings Vault & Backup
    ui.add_space(12.0);
    murmur_card(p, None).show(ui, |ui| {
        section_eyebrow(ui, p, "Settings Backup (Vault)");
        ui.add_space(6.0);
        ui.label(
            RichText::new("Save your favorite setup or share JSON backups with Murmur on Android.")
                .size(12.0)
                .color(p.text_secondary),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if pill_button(ui, p, "Save Vault", true, p.accent_anc) {
                controller.send(AppCommand::SaveVault);
            }
            if snap.vault_snapshot.is_some() {
                if pill_button(ui, p, "Restore", false, p.accent_anc) {
                    controller.send(AppCommand::RestoreVault);
                }
                if pill_button(ui, p, "Export…", false, p.text_secondary) {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("murmur-vault.json")
                        .add_filter("JSON", &["json"])
                        .save_file()
                    {
                        controller.send(AppCommand::ExportVault(path));
                    }
                }
            }
            if pill_button(ui, p, "Import…", false, p.text_secondary) {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    controller.send(AppCommand::ImportVault(path));
                }
            }
            if snap.vault_snapshot.is_some() && pill_button(ui, p, "Clear", false, p.danger) {
                controller.send(AppCommand::ClearVault);
            }
        });

        if snap.vault_snapshot.is_some() {
            ui.add_space(8.0);
            if let Some(auto) = setting_toggle_row(
                ui,
                p,
                "Auto-Restore on Connect",
                "Apply Vault automatically unless offline edits are queued",
                snap.auto_enforce_vault,
                p.accent_anc,
            ) {
                controller.send(AppCommand::SetAutoEnforceVault(auto));
            }
        }
    });
}

fn render_right_column(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
) {
    let caps = snap.headphone.capabilities();

    // Headphone Smart Controls (Zero ComboBoxes!)
    murmur_card(p, None).show(ui, |ui| {
        section_eyebrow(ui, p, "Headphone Features");
        ui.add_space(8.0);

        if caps.supports_speak_to_chat {
            if let Some(v) = setting_toggle_row(
                ui,
                p,
                "Speak-to-Chat",
                "Pause music automatically when you talk",
                snap.headphone.speak_to_chat,
                p.accent_anc,
            ) {
                controller.send(AppCommand::SetSpeakToChat(v));
            }
            if snap.headphone.speak_to_chat {
                ui.add_space(6.0);
                ui.label(RichText::new("Voice Sensitivity").size(11.5).color(p.text_secondary));
                ui.horizontal_wrapped(|ui| {
                    for s in SpeakToChatSensitivity::ALL {
                        if pill_button(
                            ui,
                            p,
                            s.label(),
                            snap.headphone.speak_to_chat_sensitivity == s,
                            p.accent_anc,
                        ) {
                            controller.send(AppCommand::SetSpeakToChatConfig(
                                s,
                                snap.headphone.speak_to_chat_timeout,
                            ));
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Resume After Silence").size(11.5).color(p.text_secondary));
                ui.horizontal_wrapped(|ui| {
                    for t in SpeakToChatTimeout::ALL {
                        let lbl = match t {
                            SpeakToChatTimeout::Short5s => "5s",
                            SpeakToChatTimeout::Standard15s => "15s",
                            SpeakToChatTimeout::Long30s => "30s",
                            SpeakToChatTimeout::Never => "Manual",
                        };
                        if pill_button(
                            ui,
                            p,
                            lbl,
                            snap.headphone.speak_to_chat_timeout == t,
                            p.accent_anc,
                        ) {
                            controller.send(AppCommand::SetSpeakToChatConfig(
                                snap.headphone.speak_to_chat_sensitivity,
                                t,
                            ));
                        }
                    }
                });
            }
            ui.add_space(6.0);
        }

        if caps.supports_wearing_detection {
            if let Some(v) = setting_toggle_row(
                ui,
                p,
                "Wearing Detection",
                "Auto-pause music when headphones come off",
                snap.headphone.wearing_detection,
                p.accent_anc,
            ) {
                controller.send(AppCommand::SetWearingDetection(v));
            }
            ui.add_space(6.0);
        }

        if caps.supports_touch_panel {
            if let Some(v) = setting_toggle_row(
                ui,
                p,
                "Touch Controls",
                "Enable touch gestures on earcup / earbuds",
                snap.headphone.touch_panel_enabled(),
                p.accent_anc,
            ) {
                controller.send(AppCommand::SetTouchPanel(v));
            }
            ui.add_space(6.0);
        }

        if caps.supports_multipoint {
            if let Some(v) = setting_toggle_row(
                ui,
                p,
                "Multipoint",
                "Connect to two devices at the same time",
                snap.headphone.multipoint_enabled(),
                p.accent_anc,
            ) {
                controller.send(AppCommand::SetMultipoint(v));
            }
            ui.add_space(6.0);
        }

        if caps.supports_voice_guidance {
            if let Some(v) = setting_toggle_row(
                ui,
                p,
                "Voice Guidance",
                "Spoken battery and mode announcements",
                snap.headphone.voice_guidance_enabled,
                p.accent_anc,
            ) {
                controller.send(AppCommand::SetVoiceGuidance(v));
            }
            ui.add_space(8.0);
        }

        if caps.supports_nc_amb_button_cycle {
            ui.label(
                RichText::new("NC / AMB Button Cycle")
                    .font(font_bold(12.5))
                    .color(p.text_primary),
            );
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for c in NcAmbButtonCycle::ALL {
                    let lbl = match c {
                        NcAmbButtonCycle::NcAmbientOff => "NC · Amb · Off",
                        NcAmbButtonCycle::NcAmbient => "NC ↔ Amb",
                        NcAmbButtonCycle::NcOff => "NC ↔ Off",
                        NcAmbButtonCycle::AmbientOff => "Amb ↔ Off",
                    };
                    if pill_button(ui, p, lbl, snap.headphone.nc_amb_button_cycle == c, p.accent_anc)
                    {
                        controller.send(AppCommand::SetNcAmbButtonCycle(c));
                    }
                }
            });
            ui.add_space(8.0);
        }

        if caps.supports_auto_power_off {
            ui.label(
                RichText::new("Auto Power-Off")
                    .font(font_bold(12.5))
                    .color(p.text_primary),
            );
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for apo in [
                    AutoPowerOff::WhenRemoved,
                    AutoPowerOff::After5Min,
                    AutoPowerOff::After15Min,
                    AutoPowerOff::After30Min,
                    AutoPowerOff::After1Hour,
                    AutoPowerOff::Never,
                ] {
                    let lbl = match apo {
                        AutoPowerOff::WhenRemoved => "When Removed",
                        AutoPowerOff::After5Min => "5m",
                        AutoPowerOff::After15Min => "15m",
                        AutoPowerOff::After30Min => "30m",
                        AutoPowerOff::After1Hour => "1h",
                        AutoPowerOff::Never => "Never",
                        _ => apo.label(),
                    };
                    if pill_button(ui, p, lbl, snap.headphone.auto_power_off == apo, p.accent_anc) {
                        controller.send(AppCommand::SetAutoPowerOff(apo));
                    }
                }
            });
            ui.add_space(8.0);
        }

        if caps.supports_voice_assistant {
            ui.label(
                RichText::new("Voice Assistant")
                    .font(font_bold(12.5))
                    .color(p.text_primary),
            );
            ui.add_space(4.0);
            let opts = if snap.headphone.voice_assistant_options.is_empty() {
                vec![
                    VoiceAssistant::DeviceDefault,
                    VoiceAssistant::GoogleAssistant,
                    VoiceAssistant::Alexa,
                    VoiceAssistant::Off,
                ]
            } else {
                snap.headphone.voice_assistant_options.clone()
            };
            ui.horizontal_wrapped(|ui| {
                for va in opts {
                    if pill_button(
                        ui,
                        p,
                        va.label(),
                        snap.headphone.voice_assistant == va,
                        p.accent_anc,
                    ) {
                        controller.send(AppCommand::SetVoiceAssistant(va));
                    }
                }
            });
            ui.add_space(8.0);
        }

        if caps.supports_assignable_settings && !snap.headphone.assignable_settings.is_empty() {
            for (idx, setting) in snap.headphone.assignable_settings.iter().enumerate() {
                ui.label(
                    RichText::new(format!("{} Sensor Action", setting.side_label()))
                        .font(font_bold(12.5))
                        .color(p.text_primary),
                );
                ui.add_space(4.0);
                let opts = if setting.available_presets.is_empty() {
                    vec![
                        AssignablePreset::AmbientSoundControl,
                        AssignablePreset::PlaybackControl,
                        AssignablePreset::VolumeControl,
                        AssignablePreset::NoFunction,
                    ]
                } else {
                    setting.available_presets.clone()
                };
                ui.horizontal_wrapped(|ui| {
                    for pr in opts {
                        if pill_button(ui, p, pr.label(), setting.preset == pr, p.accent_anc) {
                            controller.send(AppCommand::SetAssignablePreset(idx, pr));
                        }
                    }
                });
                ui.add_space(6.0);
            }
        }

        if caps.supports_quick_access {
            ui.label(
                RichText::new("Quick Access Tap")
                    .font(font_bold(12.5))
                    .color(p.text_primary),
            );
            ui.add_space(4.0);
            let opts = [QuickAccessService(0x00), QuickAccessService(0x01)];
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("2×").size(12.0).color(p.text_secondary));
                for svc in opts {
                    if pill_button(
                        ui,
                        p,
                        &svc.label(),
                        snap.headphone.quick_access_double_tap == svc,
                        p.accent_anc,
                    ) {
                        controller.send(AppCommand::SetQuickAccess(
                            svc,
                            snap.headphone.quick_access_triple_tap,
                        ));
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("3×").size(12.0).color(p.text_secondary));
                for svc in opts {
                    if pill_button(
                        ui,
                        p,
                        &svc.label(),
                        snap.headphone.quick_access_triple_tap == svc,
                        p.accent_anc,
                    ) {
                        controller.send(AppCommand::SetQuickAccess(
                            snap.headphone.quick_access_double_tap,
                            svc,
                        ));
                    }
                }
            });
            ui.add_space(6.0);
        }

        if caps.supports_power_off && snap.is_live() {
            ui.add_space(4.0);
            if pill_button(ui, p, "Turn Off Headphones", false, p.danger) {
                controller.send(AppCommand::PowerOffHeadphones);
            }
        }
    });

    ui.add_space(12.0);

    // Appearance, Battery Calibration & App Options
    murmur_card(p, None).show(ui, |ui| {
        section_eyebrow(ui, p, "Appearance & Calibration");
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for mode in AppThemeMode::ALL {
                if pill_button(ui, p, mode.label(), snap.theme_mode == mode, p.accent_anc) {
                    controller.send(AppCommand::SetThemeMode(mode));
                }
            }
        });

        ui.add_space(10.0);
        ui.label(
            RichText::new(format!("3D Model Color: {}", snap.finish_override.label()))
                .font(font_bold(12.5))
                .color(p.text_primary),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if pill_button(
                ui,
                p,
                "Auto",
                snap.finish_override == HeadphoneFinishOverride::Auto,
                p.accent_anc,
            ) {
                controller.send(AppCommand::SetFinishOverride(HeadphoneFinishOverride::Auto));
            }
            for (ovr, fin) in [
                (HeadphoneFinishOverride::ObsidianBlack, HeadphoneFinish::ObsidianBlack),
                (HeadphoneFinishOverride::PlatinumSilver, HeadphoneFinish::PlatinumSilver),
                (HeadphoneFinishOverride::MidnightBlue, HeadphoneFinish::MidnightBlue),
                (HeadphoneFinishOverride::SmokyPink, HeadphoneFinish::SmokyPink),
                (HeadphoneFinishOverride::WarmWhite, HeadphoneFinish::WarmWhite),
                (HeadphoneFinishOverride::ForestGray, HeadphoneFinish::ForestGray),
                (HeadphoneFinishOverride::CrimsonRed, HeadphoneFinish::CrimsonRed),
                (HeadphoneFinishOverride::ChampagneGold, HeadphoneFinish::ChampagneGold),
                (HeadphoneFinishOverride::Violet, HeadphoneFinish::Violet),
            ] {
                if finish_swatch_circle(ui, fin, snap.finish_override == ovr, 28.0, p) {
                    controller.send(AppCommand::SetFinishOverride(ovr));
                }
            }
        });

        ui.add_space(10.0);
        ui.label(
            RichText::new("Headphone Battery Age")
                .font(font_bold(12.5))
                .color(p.text_primary),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for age in HeadphoneAge::ALL {
                let lbl = match age {
                    HeadphoneAge::NewUnder1Y => "< 1 yr",
                    HeadphoneAge::Moderate1To2Y => "1–2 yrs",
                    HeadphoneAge::Daily2To3Y => "2–3 yrs",
                    HeadphoneAge::Heavy3YPlus => "3+ yrs",
                };
                if pill_button(ui, p, lbl, snap.headphone_age == age, p.accent_anc) {
                    controller.send(AppCommand::SetHeadphoneAge(age));
                }
            }
        });

        ui.add_space(6.0);
        let prog = snap.battery_projection.calibration_progress.clamp(0.0, 1.0);
        let bar_rect = ui
            .allocate_space(Vec2::new(ui.available_width().max(100.0), 6.0))
            .1;
        ui.painter()
            .rect_filled(bar_rect, CornerRadius::same(3), p.surface_recessed);
        let fill_w = bar_rect.width() * prog;
        if fill_w > 0.5 {
            ui.painter().rect_filled(
                Rect::from_min_size(bar_rect.min, Vec2::new(fill_w, bar_rect.height())),
                CornerRadius::same(3),
                p.accent_anc,
            );
        }
        ui.label(
            RichText::new(format!(
                "Battery calibration: {}/3 steps · {:.1}%/hr est.",
                snap.battery_projection.valid_transitions,
                snap.battery_projection.drain_rate_per_hour
            ))
            .size(11.0)
            .color(p.text_secondary),
        );

        ui.add_space(10.0);
        if let Some(mtt) = setting_toggle_row(
            ui,
            p,
            "Keep in System Tray on Close",
            "Minimize to tray for quick ANC toggles instead of quitting",
            snap.minimize_to_tray,
            p.accent_anc,
        ) {
            controller.send(AppCommand::SetMinimizeToTray(mtt));
        }
    });
}
