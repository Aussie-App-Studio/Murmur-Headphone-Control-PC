use egui::{
    Align, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, Mesh, Pos2, Rect,
    RichText, Sense, Shape, Stroke, Ui, Vec2,
};

use crate::models::{BatteryState, HeadphoneFinish, NoiseControlMode};
use crate::ui::theme::MurmurPalette;

pub fn rgb_to_color(rgb: u32) -> Color32 {
    Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

pub fn color_alpha(c: Color32, alpha: f32) -> Color32 {
    let a = (c.a() as f32 * alpha.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn mix_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| -> u8 { (x as f32 + (y as f32 - x as f32) * t).round() as u8 };
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

pub fn font_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn font_light(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("light".into()))
}

pub fn murmur_card(p: &MurmurPalette, accent_glow: Option<Color32>) -> Frame {
    let border = match accent_glow {
        Some(a) => color_alpha(a, 0.30),
        None => p.hairline,
    };
    Frame::NONE
        .fill(p.surface_card)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(16))
}

pub fn section_eyebrow(ui: &mut Ui, p: &MurmurPalette, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .font(font_bold(10.5))
            .color(p.text_tertiary),
    );
}

pub fn segmented_nav(ui: &mut Ui, p: &MurmurPalette, items: &[&str], active: usize) -> Option<usize> {
    let mut clicked = None;
    Frame::NONE
        .fill(p.surface_recessed)
        .stroke(Stroke::new(1.0_f32, p.hairline))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (idx, &label) in items.iter().enumerate() {
                    let is_active = idx == active;
                    let fill = if is_active {
                        p.surface_card
                    } else {
                        Color32::TRANSPARENT
                    };
                    let stroke = if is_active {
                        Stroke::new(1.0_f32, p.hairline)
                    } else {
                        Stroke::NONE
                    };
                    let text_col = if is_active {
                        p.text_primary
                    } else {
                        p.text_secondary
                    };
                    let btn = egui::Button::new(
                        RichText::new(label).font(font_bold(12.5)).color(text_col),
                    )
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(CornerRadius::same(10))
                    .min_size(Vec2::new(78.0, 28.0));
                    if ui.add(btn).clicked() {
                        clicked = Some(idx);
                    }
                }
            });
        });
    clicked
}

pub fn pill_button(
    ui: &mut Ui,
    p: &MurmurPalette,
    label: &str,
    selected: bool,
    accent: Color32,
) -> bool {
    let fill = if selected {
        mix_color(p.surface_card, accent, if p.is_dark { 0.24 } else { 0.14 })
    } else {
        p.surface_recessed
    };
    let text_color = if selected {
        if p.is_dark {
            mix_color(accent, Color32::WHITE, 0.28)
        } else {
            accent
        }
    } else {
        p.text_secondary
    };
    let stroke = if selected {
        Stroke::new(1.0_f32, color_alpha(accent, 0.65))
    } else {
        Stroke::new(1.0_f32, p.hairline)
    };
    let btn = egui::Button::new(RichText::new(label).size(12.5).strong().color(text_color))
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(10));
    ui.add(btn).clicked()
}

pub fn toggle_switch(ui: &mut Ui, p: &MurmurPalette, on: &mut bool, accent: Color32) -> bool {
    let desired_size = Vec2::new(42.0, 23.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let bg = if *on { accent } else { p.surface_elevated };
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, bg);
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, if *on { color_alpha(accent, 0.8) } else { p.hairline }),
            egui::StrokeKind::Inside,
        );
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = Pos2::new(circle_x, rect.center().y);
        ui.painter()
            .circle_filled(center, radius - 3.5, Color32::WHITE);
    }
    response.changed()
}

pub fn setting_toggle_row(
    ui: &mut Ui,
    p: &MurmurPalette,
    title: &str,
    subtitle: &str,
    value: bool,
    accent: Color32,
) -> Option<bool> {
    let mut current = value;
    let mut changed = false;
    Frame::NONE
        .fill(p.surface_recessed)
        .stroke(Stroke::new(1.0_f32, p.hairline))
        .corner_radius(CornerRadius::same(13))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(title)
                            .font(font_bold(13.0))
                            .color(p.text_primary),
                    );
                    if !subtitle.is_empty() {
                        ui.label(RichText::new(subtitle).size(11.5).color(p.text_secondary));
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if toggle_switch(ui, p, &mut current, accent) {
                        changed = true;
                    }
                });
            });
        });
    if changed {
        Some(current)
    } else {
        None
    }
}

pub fn quick_control_tile(
    ui: &mut Ui,
    p: &MurmurPalette,
    title: &str,
    subtitle: &str,
    active: bool,
    accent: Color32,
) -> bool {
    let width = ui.available_width().max(110.0);
    let height = 70.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if ui.is_rect_visible(rect) {
        let fill = if active {
            mix_color(p.surface_card, accent, if p.is_dark { 0.20 } else { 0.12 })
        } else if response.hovered() {
            p.surface_elevated
        } else {
            p.surface_card
        };
        let border = if active {
            color_alpha(accent, 0.62)
        } else {
            p.hairline
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(15), fill);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(15),
            Stroke::new(1.0_f32, border),
            egui::StrokeKind::Inside,
        );

        let dot_pos = Pos2::new(rect.left() + 16.0, rect.top() + 22.0);
        let dot_col = if active { accent } else { p.text_tertiary };
        ui.painter().circle_filled(dot_pos, 4.0, dot_col);
        if active {
            ui.painter()
                .circle_filled(dot_pos, 7.5, color_alpha(accent, 0.28));
        }

        ui.painter().text(
            Pos2::new(rect.left() + 28.0, rect.top() + 22.0),
            egui::Align2::LEFT_CENTER,
            title,
            font_bold(13.0),
            p.text_primary,
        );
        let sub_color = if active {
            if p.is_dark {
                mix_color(accent, Color32::WHITE, 0.25)
            } else {
                accent
            }
        } else {
            p.text_secondary
        };
        ui.painter().text(
            Pos2::new(rect.left() + 14.0, rect.top() + 47.0),
            egui::Align2::LEFT_CENTER,
            subtitle,
            FontId::proportional(11.5),
            sub_color,
        );
    }
    response.clicked()
}

pub fn acoustic_mode_dock(
    ui: &mut Ui,
    p: &MurmurPalette,
    selected: NoiseControlMode,
) -> Option<NoiseControlMode> {
    let mut next_mode = None;
    let items = [
        (NoiseControlMode::NoiseCancelling, "Noise Cancelling", p.accent_anc),
        (NoiseControlMode::AmbientSound, "Ambient", p.accent_ambient),
        (NoiseControlMode::Off, "Off", p.text_secondary),
    ];

    Frame::NONE
        .fill(p.surface_recessed)
        .stroke(Stroke::new(1.0_f32, p.hairline))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(5))
        .show(ui, |ui| {
            ui.columns(3, |cols| {
                for (idx, (mode, label, accent)) in items.iter().enumerate() {
                    let ui = &mut cols[idx];
                    let is_sel = selected == *mode;
                    let w = ui.available_width().max(80.0);
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 72.0), Sense::click());
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if ui.is_rect_visible(rect) {
                        let bg = if is_sel {
                            mix_color(p.surface_card, *accent, if p.is_dark { 0.20 } else { 0.13 })
                        } else if resp.hovered() {
                            color_alpha(p.surface_elevated, 0.7)
                        } else {
                            Color32::TRANSPARENT
                        };
                        let border = if is_sel {
                            color_alpha(*accent, 0.65)
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(rect, CornerRadius::same(14), bg);
                        if is_sel {
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(14),
                                Stroke::new(1.0_f32, border),
                                egui::StrokeKind::Inside,
                            );
                        }

                        let fg = if is_sel { *accent } else { p.text_secondary };
                        let icon_center = Pos2::new(rect.center().x, rect.top() + 25.0);
                        draw_mode_icon(ui.painter(), icon_center, *mode, fg);

                        ui.painter().text(
                            Pos2::new(rect.center().x, rect.bottom() - 18.0),
                            egui::Align2::CENTER_CENTER,
                            label,
                            font_bold(12.5),
                            if is_sel { p.text_primary } else { p.text_secondary },
                        );
                    }
                    if resp.clicked() && !is_sel {
                        next_mode = Some(*mode);
                    }
                }
            });
        });
    next_mode
}

fn draw_mode_icon(painter: &egui::Painter, c: Pos2, mode: NoiseControlMode, col: Color32) {
    let stroke = Stroke::new(1.8_f32, col);
    match mode {
        NoiseControlMode::NoiseCancelling => {
            // Shield / Acoustic seal icon
            let pts = vec![
                Pos2::new(c.x, c.y - 9.0),
                Pos2::new(c.x + 8.0, c.y - 5.5),
                Pos2::new(c.x + 6.5, c.y + 3.5),
                Pos2::new(c.x, c.y + 9.0),
                Pos2::new(c.x - 6.5, c.y + 3.5),
                Pos2::new(c.x - 8.0, c.y - 5.5),
                Pos2::new(c.x, c.y - 9.0),
            ];
            painter.add(Shape::line(pts, stroke));
            painter.circle_filled(c, 2.3, col);
        }
        NoiseControlMode::AmbientSound => {
            // Concentric sound wave arcs
            painter.circle_filled(Pos2::new(c.x - 5.0, c.y), 2.5, col);
            for r in [6.0_f32, 10.5_f32] {
                let mut arc = Vec::new();
                for i in 0..=10 {
                    let a = -0.85 + (i as f32 / 10.0) * 1.7;
                    arc.push(Pos2::new(c.x - 4.0 + r * a.cos(), c.y + r * a.sin()));
                }
                painter.add(Shape::line(arc, stroke));
            }
        }
        NoiseControlMode::Off => {
            painter.circle_stroke(c, 7.5, stroke);
            painter.line_segment(
                [Pos2::new(c.x - 4.5, c.y + 4.5), Pos2::new(c.x + 4.5, c.y - 4.5)],
                stroke,
            );
        }
    }
}

pub fn ambient_precision_scrubber(
    ui: &mut Ui,
    p: &MurmurPalette,
    level: i32,
) -> Option<(i32, bool)> {
    let w = ui.available_width().max(160.0);
    let h = 48.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    let pos_to_level = |x: f32| -> i32 {
        let f = ((x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
        (1.0 + (f * 19.0).round()).clamp(1.0, 20.0) as i32
    };

    let mut emitted = None;
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            emitted = Some((pos_to_level(pos.x), false));
        }
    } else if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let next = pos_to_level(pos.x);
            if next != level {
                emitted = Some((next, true));
            }
        }
    } else if response.drag_stopped() {
        emitted = Some((level, false));
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(16), p.surface_recessed);

        let frac = ((level - 1).clamp(0, 19) as f32) / 19.0;
        let fill_w = (rect.width() * (0.06 + frac * 0.94)).clamp(20.0, rect.width());
        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        draw_horizontal_gradient_rect(
            painter,
            fill_rect,
            color_alpha(p.accent_ambient, 0.20),
            color_alpha(p.accent_ambient, 0.56),
            16.0,
        );

        let pad_x = 14.0;
        let usable_w = (rect.width() - pad_x * 2.0).max(10.0);
        for i in 0..20 {
            let tx = rect.left() + pad_x + (i as f32 / 19.0) * usable_w;
            let tick_h = if i % 5 == 0 || i == 19 { 13.0 } else { 7.0 };
            let active = (i + 1) <= level;
            let col = if active {
                if p.is_dark {
                    color_alpha(Color32::WHITE, 0.58)
                } else {
                    color_alpha(p.text_primary, 0.45)
                }
            } else {
                color_alpha(p.text_tertiary, 0.38)
            };
            painter.line_segment(
                [
                    Pos2::new(tx, rect.center().y - tick_h / 2.0),
                    Pos2::new(tx, rect.center().y + tick_h / 2.0),
                ],
                Stroke::new(1.5_f32, col),
            );
        }

        let handle_x = (rect.left() + fill_w - 9.0).clamp(rect.left() + 10.0, rect.right() - 10.0);
        painter.line_segment(
            [
                Pos2::new(handle_x, rect.top() + 11.0),
                Pos2::new(handle_x, rect.bottom() - 11.0),
            ],
            Stroke::new(3.5_f32, p.accent_ambient),
        );

        painter.rect_stroke(
            rect,
            CornerRadius::same(16),
            Stroke::new(1.0_f32, p.hairline),
            egui::StrokeKind::Inside,
        );
    }
    emitted
}

pub fn bipolar_studio_bar(
    ui: &mut Ui,
    p: &MurmurPalette,
    value: i32,
    min_gain: i32,
    max_gain: i32,
    accent: Color32,
) -> Option<(i32, bool)> {
    let w = ui.available_width().max(160.0);
    let h = 36.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    let span = (max_gain - min_gain).max(1) as f32;
    let x_to_val = |x: f32| -> i32 {
        let f = ((x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
        (min_gain as f32 + (f * span).round()).clamp(min_gain as f32, max_gain as f32) as i32
    };

    let mut emitted = None;
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            emitted = Some((x_to_val(pos.x), false));
        }
    } else if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let next = x_to_val(pos.x);
            if next != value {
                emitted = Some((next, true));
            }
        }
    } else if response.drag_stopped() {
        emitted = Some((value, false));
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(12), p.surface_recessed);

        let mid_x = rect.center().x;
        let pad_x = 14.0;
        let half_span = rect.width() / 2.0 - pad_x;
        let half_gain = max_gain.max(1) as f32;
        let thumb_x = mid_x + (value.clamp(min_gain, max_gain) as f32 / half_gain) * half_span;

        if value != 0 {
            let left = thumb_x.min(mid_x);
            let right = thumb_x.max(mid_x);
            let fill_rect = Rect::from_min_max(
                Pos2::new(left, rect.top() + 5.0),
                Pos2::new(right, rect.bottom() - 5.0),
            );
            painter.rect_filled(fill_rect, CornerRadius::same(6), color_alpha(accent, 0.34));
        }

        painter.line_segment(
            [
                Pos2::new(mid_x, rect.top() + 8.0),
                Pos2::new(mid_x, rect.bottom() - 8.0),
            ],
            Stroke::new(1.5_f32, color_alpha(p.text_tertiary, 0.5)),
        );
        painter.line_segment(
            [
                Pos2::new(thumb_x, rect.top() + 7.0),
                Pos2::new(thumb_x, rect.bottom() - 7.0),
            ],
            Stroke::new(4.0_f32, accent),
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(12),
            Stroke::new(1.0_f32, p.hairline),
            egui::StrokeKind::Inside,
        );
    }

    emitted
}

pub fn vertical_studio_fader(
    ui: &mut Ui,
    p: &MurmurPalette,
    label: &str,
    value: i32,
    min_gain: i32,
    max_gain: i32,
    accent: Color32,
) -> Option<(i32, bool)> {
    let mut emitted = None;
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(if value > 0 {
                format!("+{}", value)
            } else {
                format!("{}", value)
            })
            .font(font_bold(11.5))
            .color(if value != 0 { accent } else { p.text_secondary }),
        );

        let fader_size = Vec2::new(34.0, 92.0);
        let (rect, response) = ui.allocate_exact_size(fader_size, Sense::click_and_drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        let span = (max_gain - min_gain).max(1) as f32;
        let y_to_val = |y: f32| -> i32 {
            let norm = 1.0 - ((y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
            (min_gain as f32 + (norm * span).round()).clamp(min_gain as f32, max_gain as f32) as i32
        };

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                emitted = Some((y_to_val(pos.y), false));
            }
        } else if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let next = y_to_val(pos.y);
                if next != value {
                    emitted = Some((next, true));
                }
            }
        } else if response.drag_stopped() {
            emitted = Some((value, false));
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let track_w = 10.0;
            let track_rect = Rect::from_center_size(rect.center(), Vec2::new(track_w, rect.height() - 10.0));
            painter.rect_filled(track_rect, CornerRadius::same(5), p.surface_recessed);
            painter.rect_stroke(
                track_rect,
                CornerRadius::same(5),
                Stroke::new(1.0_f32, p.hairline),
                egui::StrokeKind::Inside,
            );

            let mid_y = track_rect.center().y;
            painter.line_segment(
                [
                    Pos2::new(track_rect.left() - 4.0, mid_y),
                    Pos2::new(track_rect.right() + 4.0, mid_y),
                ],
                Stroke::new(1.0_f32, color_alpha(p.text_tertiary, 0.45)),
            );

            let norm = (value.clamp(min_gain, max_gain) - min_gain) as f32 / span;
            let thumb_y = egui::lerp(track_rect.bottom()..=track_rect.top(), norm);

            if value != 0 {
                let top = thumb_y.min(mid_y);
                let bottom = thumb_y.max(mid_y);
                let fill_rect = Rect::from_min_max(
                    Pos2::new(track_rect.left() + 1.5, top),
                    Pos2::new(track_rect.right() - 1.5, bottom),
                );
                painter.rect_filled(fill_rect, CornerRadius::same(4), color_alpha(accent, 0.72));
            }

            // Sculpted fader cap
            let cap = Rect::from_center_size(Pos2::new(rect.center().x, thumb_y), Vec2::new(24.0, 12.0));
            painter.rect_filled(
                cap,
                CornerRadius::same(5),
                if value != 0 { accent } else { p.surface_elevated },
            );
            painter.rect_stroke(
                cap,
                CornerRadius::same(5),
                Stroke::new(1.0_f32, if value != 0 { Color32::WHITE } else { p.hairline }),
                egui::StrokeKind::Inside,
            );
        }

        ui.label(RichText::new(label).size(11.0).color(p.text_tertiary));
    });
    emitted
}

pub fn eq_studio_curve(
    ui: &mut Ui,
    p: &MurmurPalette,
    clear_bass: i32,
    bands: &[i32],
    has_clear_bass: bool,
    min_gain: i32,
    max_gain: i32,
) {
    let width = ui.available_width().max(200.0);
    let height = 86.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(14), p.surface_recessed);
    painter.rect_stroke(
        rect,
        CornerRadius::same(14),
        Stroke::new(1.0_f32, p.hairline),
        egui::StrokeKind::Inside,
    );

    let mid_y = rect.center().y;
    painter.line_segment(
        [
            Pos2::new(rect.left() + 14.0, mid_y),
            Pos2::new(rect.right() - 14.0, mid_y),
        ],
        Stroke::new(1.0_f32, color_alpha(p.text_tertiary, 0.32)),
    );

    if bands.is_empty() {
        return;
    }

    let pad_y = 14.0;
    let usable_h = (rect.height() - pad_y * 2.0).max(10.0);
    let span = (max_gain - min_gain).max(2) as f32;
    let val_to_y = |val: i32| -> f32 {
        let norm = ((val - min_gain) as f32 / span).clamp(0.0, 1.0);
        rect.bottom() - pad_y - norm * usable_h
    };

    // Align each band dot directly above its vertical fader column
    let band_pts: Vec<Pos2> = bands
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let t = if bands.len() <= 6 {
                (i as f32 + 0.5) / bands.len() as f32
            } else {
                0.04 + (i as f32 / (bands.len() - 1) as f32) * 0.92
            };
            Pos2::new(rect.left() + t * rect.width(), val_to_y(val))
        })
        .collect();

    // Extend spline smoothly to left & right container edges
    let left_y = if has_clear_bass {
        val_to_y(((clear_bass + bands[0]) as f32 * 0.5).round() as i32)
    } else {
        band_pts[0].y
    };
    let mut ctrl_pts = Vec::with_capacity(band_pts.len() + 2);
    ctrl_pts.push(Pos2::new(rect.left() + 10.0, left_y));
    ctrl_pts.extend_from_slice(&band_pts);
    ctrl_pts.push(Pos2::new(rect.right() - 10.0, band_pts.last().unwrap().y));

    // Smooth Catmull-Rom interpolation
    let mut smooth = Vec::new();
    let sub = 10;
    for i in 0..ctrl_pts.len() - 1 {
        let p0 = ctrl_pts[i.saturating_sub(1)];
        let p1 = ctrl_pts[i];
        let p2 = ctrl_pts[i + 1];
        let p3 = ctrl_pts[(i + 2).min(ctrl_pts.len() - 1)];
        for s in 0..sub {
            let t = s as f32 / sub as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let x = 0.5
                * ((2.0 * p1.x)
                    + (-p0.x + p2.x) * t
                    + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
                    + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);
            let y = 0.5
                * ((2.0 * p1.y)
                    + (-p0.y + p2.y) * t
                    + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
                    + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);
            smooth.push(Pos2::new(x, y.clamp(rect.top() + 8.0, rect.bottom() - 8.0)));
        }
    }
    smooth.push(*ctrl_pts.last().unwrap());

    // Translucent gradient mesh under the curve
    if smooth.len() >= 2 {
        let mut mesh = Mesh::default();
        let top_col = color_alpha(p.accent_anc, if p.is_dark { 0.30 } else { 0.22 });
        let bot_col = color_alpha(p.accent_anc, 0.0);
        let bottom_y = rect.bottom() - 5.0;
        for (i, &pt) in smooth.iter().enumerate() {
            mesh.colored_vertex(pt, top_col);
            mesh.colored_vertex(Pos2::new(pt.x, bottom_y), bot_col);
            if i > 0 {
                let v_top = (i * 2) as u32;
                let v_bot = v_top + 1;
                let prev_top = v_top - 2;
                let prev_bot = v_top - 1;
                mesh.add_triangle(prev_top, v_top, v_bot);
                mesh.add_triangle(prev_top, v_bot, prev_bot);
            }
        }
        painter.add(Shape::mesh(mesh));
        painter.add(Shape::line(smooth, Stroke::new(2.4_f32, p.accent_anc)));
    }

    for pt in band_pts {
        painter.circle_filled(pt, 4.5, p.accent_anc);
        painter.circle_stroke(pt, 4.5, Stroke::new(1.5_f32, p.surface_card));
    }
}

pub fn finish_swatch_circle(
    ui: &mut Ui,
    finish: HeadphoneFinish,
    selected: bool,
    size: f32,
    p: &MurmurPalette,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if ui.is_rect_visible(rect) {
        let c = rect.center();
        let r = size * 0.40;
        ui.painter().circle_filled(c, r, rgb_to_color(finish.shell_rgb()));
        ui.painter()
            .circle_stroke(c, r, Stroke::new(1.5_f32, rgb_to_color(finish.metal_rgb())));
        if selected {
            ui.painter()
                .circle_stroke(c, r + 3.5, Stroke::new(2.0_f32, p.accent_anc));
        }
    }
    resp.clicked()
}

pub fn battery_telemetry_strip(
    ui: &mut Ui,
    p: &MurmurPalette,
    batt: &BatteryState,
    remaining_minutes: Option<i32>,
    is_calibrated: bool,
    codec_label: &str,
) {
    ui.vertical_centered(|ui| {
        if batt.is_dual_battery() {
            let mut chips: Vec<(&'static str, i32, bool)> = Vec::new();
            if let Some(l) = &batt.left {
                chips.push(("LEFT", l.level, l.is_charging()));
            }
            if let Some(r) = &batt.right {
                chips.push(("RIGHT", r.level, r.is_charging()));
            }
            if let Some(c) = &batt.case_battery {
                chips.push(("CASE", c.level, c.is_charging()));
            }
            let spacing = 8.0;
            let total_w: f32 = chips
                .iter()
                .map(|(_, lvl, ch)| {
                    let suffix = if *ch { " ⚡" } else { "" };
                    let galley = ui.painter().layout_no_wrap(
                        format!("{}%{}", lvl, suffix),
                        font_bold(14.5),
                        p.text_primary,
                    );
                    galley.size().x.max(36.0) + 24.0
                })
                .sum::<f32>()
                + spacing * chips.len().saturating_sub(1) as f32;
            let lead = ((ui.available_width() - total_w) * 0.5).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(lead);
                ui.spacing_mut().item_spacing.x = spacing;
                for (tag, lvl, ch) in chips {
                    battery_chip(ui, p, tag, lvl, ch);
                }
            });
        } else if let Some(lvl) = batt.effective_level() {
            let color = if lvl <= 15 {
                p.danger
            } else if batt.is_charging() {
                p.accent_anc
            } else {
                p.text_primary
            };
            ui.label(
                RichText::new(format!("{}%", lvl))
                    .font(font_light(38.0))
                    .color(color),
            );
        } else {
            ui.label(
                RichText::new("—%")
                    .font(font_light(34.0))
                    .color(p.text_tertiary),
            );
        }

        ui.add_space(4.0);
        let remaining_str = if let Some(mins) = remaining_minutes {
            let h = mins / 60;
            let m = mins % 60;
            let tag = if is_calibrated { "Calibrated" } else { "Est." };
            format!("~{}h {}m ({})", h, m, tag)
        } else if batt.is_charging() {
            "Charging".to_string()
        } else {
            "Endurance —".to_string()
        };
        let mut row_chips: Vec<(String, Color32)> = vec![(remaining_str, p.text_secondary)];
        if codec_label != "—" {
            row_chips.push((codec_label.to_string(), p.accent_anc));
        }
        let spacing = 6.0;
        let total_w: f32 = row_chips
            .iter()
            .map(|(txt, col)| {
                let galley = ui
                    .painter()
                    .layout_no_wrap(txt.clone(), font_bold(11.5), *col);
                galley.size().x + 20.0
            })
            .sum::<f32>()
            + spacing * row_chips.len().saturating_sub(1) as f32;
        let lead = ((ui.available_width() - total_w) * 0.5).max(0.0);
        ui.horizontal(|ui| {
            ui.add_space(lead);
            ui.spacing_mut().item_spacing.x = spacing;
            for (txt, col) in row_chips {
                telemetry_chip(ui, p, &txt, col);
            }
        });
    });
}

fn telemetry_chip(ui: &mut Ui, p: &MurmurPalette, text: &str, text_color: Color32) {
    Frame::NONE
        .fill(p.surface_recessed)
        .stroke(Stroke::new(1.0_f32, p.hairline))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(font_bold(11.5))
                    .color(text_color),
            );
        });
}

fn battery_chip(ui: &mut Ui, p: &MurmurPalette, tag: &str, level: i32, charging: bool) {
    Frame::NONE
        .fill(p.surface_recessed)
        .stroke(Stroke::new(1.0_f32, p.hairline))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(tag).font(font_bold(9.5)).color(p.text_tertiary));
                let suffix = if charging { " ⚡" } else { "" };
                let col = if level <= 15 { p.danger } else { p.text_primary };
                ui.label(
                    RichText::new(format!("{}%{}", level, suffix))
                        .font(font_bold(14.5))
                        .color(col),
                );
            });
        });
}

fn draw_horizontal_gradient_rect(
    painter: &egui::Painter,
    rect: Rect,
    left_col: Color32,
    right_col: Color32,
    radius: f32,
) {
    painter.rect_filled(
        rect,
        CornerRadius::same(radius as u8),
        mix_color(left_col, right_col, 0.55),
    );
}
