use egui::{Layout, RichText, ScrollArea, Ui};

use crate::controller::{AppCommand, AppController, UiSnapshot};
use crate::models::{ConnectionPriority, EqualizerPreset};
use crate::ui::theme::MurmurPalette;
use crate::ui::widgets::{
    bipolar_studio_bar, eq_studio_curve, font_bold, murmur_card, pill_button, section_eyebrow,
    setting_toggle_row, vertical_studio_fader,
};

pub fn render_sound_room(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
    open_save_eq_modal: &mut bool,
) {
    let caps = snap.headphone.capabilities();
    let eq = &snap.headphone.equalizer;

    if caps.supports_equalizer {
        let matching_profile = snap.custom_eq_profiles.iter().find(|prof| {
            eq.preset() == Some(EqualizerPreset::Manual)
                && eq.clear_bass == prof.clear_bass
                && eq.bands == adapt_bands(&prof.bands, eq.bands.len())
        });
        let active_label = matching_profile
            .map(|pr| pr.name.as_str())
            .or_else(|| eq.preset().map(|pr| pr.label()))
            .unwrap_or("Custom");

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                section_eyebrow(ui, p, "Equalizer");
                ui.label(
                    RichText::new(active_label)
                        .font(font_bold(20.0))
                        .color(p.text_primary),
                );
            });
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if pill_button(
                    ui,
                    p,
                    if matching_profile.is_none() {
                        "+ Save Profile"
                    } else {
                        "+ Save New"
                    },
                    matching_profile.is_none(),
                    p.accent_anc,
                ) {
                    *open_save_eq_modal = true;
                }
            });
        });

        ui.add_space(6.0);

        section_eyebrow(ui, p, "Presets");
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for prof in &snap.custom_eq_profiles {
                let adapted = adapt_bands(&prof.bands, eq.bands.len());
                let is_active = matching_profile.is_some_and(|m| m.id == prof.id);
                if pill_button(ui, p, &prof.name, is_active, p.accent_anc) {
                    controller.send(AppCommand::SetEqCustom {
                        clear_bass: prof.clear_bass,
                        bands: adapted,
                        dragging: false,
                    });
                }
                if !matches!(
                    prof.id.as_str(),
                    "sony_balanced" | "warm_studio" | "podcast_vocal" | "sub_bass"
                ) && pill_button(ui, p, "×", false, p.danger)
                {
                    controller.send(AppCommand::DeleteCustomEqProfile(prof.id.clone()));
                }
            }

            let builtins: Vec<EqualizerPreset> = if eq.available_preset_codes.is_empty() {
                EqualizerPreset::ALL.to_vec()
            } else {
                eq.available_preset_codes
                    .iter()
                    .filter_map(|&c| EqualizerPreset::from_byte((c & 0xFF) as u8))
                    .filter(|pr| !pr.is_customizable())
                    .collect()
            };

            for preset in builtins {
                let selected =
                    matching_profile.is_none() && eq.preset_code == preset.wire_byte() as i32;
                if pill_button(ui, p, preset.label(), selected, p.accent_anc) {
                    controller.send(AppCommand::SetEqPreset(preset));
                }
            }
        });

        ui.add_space(8.0);

        murmur_card(p, None).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Frequency Console")
                        .font(font_bold(15.0))
                        .color(p.text_primary),
                );
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if pill_button(ui, p, "Reset Flat", false, p.text_secondary) {
                        controller.send(AppCommand::SetEqCustom {
                            clear_bass: 0,
                            bands: vec![0; eq.bands.len()],
                            dragging: false,
                        });
                    }
                });
            });

            ui.add_space(8.0);
            eq_studio_curve(
                ui,
                p,
                eq.clear_bass,
                &eq.bands,
                eq.has_clear_bass,
                eq.min_gain(),
                eq.max_gain(),
            );
            ui.add_space(8.0);

            let mut bands = eq.bands.clone();
            let min_g = eq.min_gain();
            let max_g = eq.max_gain();
            let labels = band_labels_for(bands.len());

            if bands.len() <= 6 {
                let n = bands.len().max(1);
                ui.columns(n, |cols| {
                    for (i, col) in cols.iter_mut().enumerate() {
                        let lbl = labels.get(i).copied().unwrap_or("Band");
                        if let Some((next_val, dragging)) = vertical_studio_fader(
                            col,
                            p,
                            lbl,
                            bands[i],
                            min_g,
                            max_g,
                            p.accent_anc,
                        ) {
                            bands[i] = next_val;
                            controller.send(AppCommand::SetEqCustom {
                                clear_bass: eq.clear_bass,
                                bands: bands.clone(),
                                dragging,
                            });
                        }
                    }
                });
            } else {
                ScrollArea::horizontal().id_salt("faders_h").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for i in 0..bands.len() {
                            let lbl = labels.get(i).copied().unwrap_or("Band");
                            ui.allocate_ui(egui::vec2(54.0, 190.0), |ui| {
                                if let Some((next_val, dragging)) = vertical_studio_fader(
                                    ui,
                                    p,
                                    lbl,
                                    bands[i],
                                    min_g,
                                    max_g,
                                    p.accent_anc,
                                ) {
                                    bands[i] = next_val;
                                    controller.send(AppCommand::SetEqCustom {
                                        clear_bass: eq.clear_bass,
                                        bands: bands.clone(),
                                        dragging,
                                    });
                                }
                            });
                        }
                    });
                });
            }

            if eq.has_clear_bass {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Clear Bass")
                            .font(font_bold(13.0))
                            .color(p.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(if eq.clear_bass > 0 {
                                format!("+{}", eq.clear_bass)
                            } else {
                                format!("{}", eq.clear_bass)
                            })
                            .font(font_bold(13.0))
                            .color(p.accent_ambient),
                        );
                    });
                });
                ui.add_space(4.0);
                if let Some((cb, dragging)) = bipolar_studio_bar(
                    ui,
                    p,
                    eq.clear_bass,
                    min_g,
                    max_g,
                    p.accent_ambient,
                ) {
                    controller.send(AppCommand::SetEqCustom {
                        clear_bass: cb,
                        bands: eq.bands.clone(),
                        dragging,
                    });
                }
            }
        });
    } else {
        murmur_card(p, None).show(ui, |ui| {
            section_eyebrow(ui, p, "Equalizer");
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "This headphone model does not expose a hardware equalizer over Bluetooth.",
                )
                .size(13.0)
                .color(p.text_secondary),
            );
        });
    }

    if caps.supports_dsee || caps.supports_connection_priority {
        ui.add_space(12.0);
        let wide = ui.available_width() >= 680.0;
        if wide && caps.supports_dsee && caps.supports_connection_priority {
            ui.columns(2, |cols| {
                render_dsee_card(&mut cols[0], p, snap, controller);
                render_priority_card(&mut cols[1], p, snap, controller);
            });
        } else {
            if caps.supports_dsee {
                render_dsee_card(ui, p, snap, controller);
                ui.add_space(10.0);
            }
            if caps.supports_connection_priority {
                render_priority_card(ui, p, snap, controller);
            }
        }
    }
}

fn render_dsee_card(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
) {
    murmur_card(p, None).show(ui, |ui| {
        section_eyebrow(ui, p, "Audio Upscaling");
        ui.add_space(6.0);
        if let Some(v) = setting_toggle_row(
            ui,
            p,
            "DSEE Extreme",
            "Restore high-frequency detail on compressed audio",
            snap.headphone.dsee_extreme,
            p.accent_anc,
        ) {
            controller.send(AppCommand::SetDsee(v));
        }
    });
}

fn render_priority_card(
    ui: &mut Ui,
    p: &MurmurPalette,
    snap: &UiSnapshot,
    controller: &AppController,
) {
    murmur_card(p, None).show(ui, |ui| {
        section_eyebrow(ui, p, "Bluetooth Audio Link");
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for prio in ConnectionPriority::ALL {
                let sel = snap.headphone.connection_priority == prio;
                if pill_button(ui, p, prio.label(), sel, p.accent_anc) {
                    controller.send(AppCommand::SetConnectionPriority(prio));
                }
            }
        });
    });
}

fn band_labels_for(count: usize) -> Vec<&'static str> {
    match count {
        5 => vec!["400", "1k", "2.5k", "6.3k", "16k"],
        8 => vec!["100", "200", "400", "800", "1.6k", "3.2k", "6.4k", "12.8k"],
        10 => vec!["31", "63", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"],
        _ => vec!["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"],
    }
}

pub fn adapt_bands(bands: &[i32], target_len: usize) -> Vec<i32> {
    if bands.len() == target_len || target_len == 0 {
        return bands.to_vec();
    }
    if bands.is_empty() {
        return vec![0; target_len];
    }
    (0..target_len)
        .map(|i| {
            let idx = (i * (bands.len() - 1)) / (target_len - 1).max(1);
            bands[idx]
        })
        .collect()
}
