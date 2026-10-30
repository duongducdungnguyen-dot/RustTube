use egui::{Color32, Context, Rounding, Stroke, Visuals};

pub const BG_CANVAS: Color32 = Color32::from_rgb(14, 15, 20);
pub const BG_SURFACE: Color32 = Color32::from_rgb(20, 22, 30);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(28, 31, 43);
pub const ACCENT_RED: Color32 = Color32::from_rgb(255, 0, 51);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 238, 245);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(138, 144, 166);

pub fn apply_obsidian_theme(ctx: &Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_CANVAS;
    visuals.window_fill = BG_SURFACE;
    visuals.widgets.noninteractive.bg_fill = BG_SURFACE;
    visuals.widgets.inactive.bg_fill = BG_ELEVATED;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 42, 58);
    visuals.widgets.active.bg_fill = ACCENT_RED;
    visuals.selection.bg_fill = ACCENT_RED.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_RED);
    visuals.window_rounding = Rounding::same(10.0);
    ctx.set_visuals(visuals);
}
