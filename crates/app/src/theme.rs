use egui::{Color32, Rounding, Stroke, Visuals};

pub struct ThemeColors {
    pub bg_dark: Color32,
    pub card_bg: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub border_active: Color32,
    pub crimson: Color32,
    pub crimson_hover: Color32,
    pub text_white: Color32,
    pub text_subdued: Color32,
    pub text_muted: Color32,
}

pub const THEME: ThemeColors = ThemeColors {
    bg_dark: Color32::from_rgb(14, 15, 20),
    card_bg: Color32::from_rgb(21, 22, 28),
    card_hover: Color32::from_rgb(28, 30, 40),
    border: Color32::from_rgb(31, 33, 45),
    border_active: Color32::from_rgb(255, 0, 51),
    crimson: Color32::from_rgb(255, 0, 51),
    crimson_hover: Color32::from_rgb(217, 0, 44),
    text_white: Color32::from_rgb(240, 240, 245),
    text_subdued: Color32::from_rgb(160, 164, 184),
    text_muted: Color32::from_rgb(112, 116, 138),
};

pub fn configure_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    
    visuals.dark_mode = true;
    visuals.override_text_color = Some(THEME.text_white);
    visuals.panel_fill = THEME.bg_dark;
    visuals.window_fill = THEME.bg_dark;
    visuals.extreme_bg_color = Color32::from_rgb(18, 19, 26);
    visuals.faint_bg_color = THEME.card_bg;

    visuals.widgets.noninteractive.bg_fill = THEME.card_bg;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, THEME.border);
    visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

    visuals.widgets.inactive.bg_fill = THEME.card_bg;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, THEME.border);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = THEME.card_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(60, 65, 88));
    visuals.widgets.hovered.rounding = Rounding::same(6.0);

    visuals.widgets.active.bg_fill = Color32::from_rgb(35, 38, 52);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, THEME.crimson);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    ctx.set_visuals(visuals);
}
