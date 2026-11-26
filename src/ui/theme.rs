use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Style, Visuals};

use crate::models::AppThemeMode;
use crate::platform::is_windows_dark_theme;

#[derive(Clone, Copy, Debug)]
pub struct MurmurPalette {
    pub is_dark: bool,
    pub canvas: Color32,
    pub surface_card: Color32,
    pub surface_recessed: Color32,
    pub surface_elevated: Color32,
    pub hairline: Color32,
    pub specular: Color32,
    pub accent_anc: Color32,
    pub accent_ambient: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub danger: Color32,
    pub warning_bg: Color32,
    pub warning_border: Color32,
}

pub const STUDIO_DARK: MurmurPalette = MurmurPalette {
    is_dark: true,
    canvas: Color32::from_rgb(0x11, 0x14, 0x19),
    surface_card: Color32::from_rgb(0x1A, 0x1E, 0x26),
    surface_recessed: Color32::from_rgb(0x13, 0x16, 0x1C),
    surface_elevated: Color32::from_rgb(0x24, 0x2A, 0x35),
    hairline: Color32::from_rgba_premultiplied(24, 24, 24, 24),
    specular: Color32::from_rgba_premultiplied(30, 30, 30, 30),
    accent_anc: Color32::from_rgb(0x2C, 0xB5, 0xA6),
    accent_ambient: Color32::from_rgb(0xE0, 0x88, 0x4B),
    text_primary: Color32::from_rgb(0xF2, 0xF5, 0xF8),
    text_secondary: Color32::from_rgb(0xA4, 0xAE, 0xBC),
    text_tertiary: Color32::from_rgb(0x70, 0x7B, 0x8C),
    danger: Color32::from_rgb(0xE5, 0x5B, 0x5B),
    warning_bg: Color32::from_rgb(0x29, 0x1F, 0x15),
    warning_border: Color32::from_rgb(0xE0, 0x88, 0x4B),
};

pub const STUDIO_LIGHT: MurmurPalette = MurmurPalette {
    is_dark: false,
    canvas: Color32::from_rgb(0xF3, 0xF5, 0xF8),
    surface_card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface_recessed: Color32::from_rgb(0xEB, 0xF0, 0xF5),
    surface_elevated: Color32::from_rgb(0xE2, 0xE8, 0xF0),
    hairline: Color32::from_rgba_premultiplied(2, 3, 5, 24),
    specular: Color32::from_rgba_premultiplied(215, 215, 215, 215),
    accent_anc: Color32::from_rgb(0x0D, 0x8A, 0x7E),
    accent_ambient: Color32::from_rgb(0xC8, 0x68, 0x27),
    text_primary: Color32::from_rgb(0x11, 0x14, 0x18),
    text_secondary: Color32::from_rgb(0x5B, 0x64, 0x70),
    text_tertiary: Color32::from_rgb(0x88, 0x92, 0xA0),
    danger: Color32::from_rgb(0xC9, 0x36, 0x36),
    warning_bg: Color32::from_rgb(0xFD, 0xF2, 0xE6),
    warning_border: Color32::from_rgb(0xC8, 0x68, 0x27),
};

pub fn resolve_palette(mode: AppThemeMode) -> MurmurPalette {
    let dark = match mode {
        AppThemeMode::System => is_windows_dark_theme(),
        AppThemeMode::StudioDark => true,
        AppThemeMode::Light => false,
    };
    if dark {
        STUDIO_DARK
    } else {
        STUDIO_LIGHT
    }
}

pub fn install_windows_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let load_font = |fonts: &mut FontDefinitions, key: &str, path: &str, family: FontFamily| {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
            let list = fonts.families.entry(family).or_default();
            list.insert(0, key.to_owned());
        }
    };

    load_font(
        &mut fonts,
        "segoe_ui",
        r"C:\Windows\Fonts\segoeui.ttf",
        FontFamily::Proportional,
    );
    load_font(
        &mut fonts,
        "segoe_ui_bold",
        r"C:\Windows\Fonts\segoeuib.ttf",
        FontFamily::Name("bold".into()),
    );
    load_font(
        &mut fonts,
        "segoe_ui_light",
        r"C:\Windows\Fonts\segoeuil.ttf",
        FontFamily::Name("light".into()),
    );

    // Ensure bold/light fall back to Proportional if system font wasn't present
    if let Some(prop) = fonts.families.get(&FontFamily::Proportional).cloned() {
        for name in ["bold", "light"] {
            let entry = fonts.families.entry(FontFamily::Name(name.into())).or_default();
            for fallback in &prop {
                if !entry.contains(fallback) {
                    entry.push(fallback.clone());
                }
            }
        }
    }

    ctx.set_fonts(fonts);
}

pub fn apply_theme(ctx: &egui::Context, p: &MurmurPalette) {
    let mut visuals = if p.is_dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = p.canvas;
    visuals.window_fill = p.surface_card;
    visuals.extreme_bg_color = p.surface_recessed;
    visuals.faint_bg_color = p.surface_recessed;
    visuals.selection.bg_fill = p.accent_anc;
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.override_text_color = Some(p.text_primary);
    visuals.window_corner_radius = CornerRadius::same(16);
    visuals.menu_corner_radius = CornerRadius::same(12);
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if p.is_dark { 110 } else { 32 }),
    };

    visuals.widgets.noninteractive.bg_fill = p.surface_card;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.hairline);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_secondary);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(12);

    visuals.widgets.inactive.bg_fill = p.surface_elevated;
    visuals.widgets.inactive.weak_bg_fill = p.surface_elevated;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.hairline);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(10);

    visuals.widgets.hovered.bg_fill = p.surface_elevated;
    visuals.widgets.hovered.weak_bg_fill = p.surface_elevated;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent_anc);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2_f32, p.text_primary);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(10);

    visuals.widgets.active.bg_fill = p.accent_anc;
    visuals.widgets.active.weak_bg_fill = p.accent_anc;
    visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(10);

    let mut style = Style {
        visuals,
        ..Default::default()
    };
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(16);
    style.spacing.slider_width = 180.0;
    ctx.set_style(style);
}
