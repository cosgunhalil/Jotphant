//! Warm color themes.
//!
//! Two palettes — a cream-and-amber light theme and a charcoal-and-amber dark theme —
//! expressed as egui [`Visuals`]. The goal is a warm, motivating feel rather than the
//! neutral gray defaults.

use eframe::egui::{Color32, CornerRadius, Stroke, Visuals};

use crate::domain::config::ThemeChoice;

/// The warm colors a theme is built from.
struct Palette {
    /// Main screen background.
    panel: Color32,
    /// Floating window / modal background.
    window: Color32,
    /// Faint fill for striped rows and subtle emphasis.
    faint: Color32,
    /// Text-edit / deepest background.
    extreme: Color32,
    /// Regular text.
    text: Color32,
    /// Emphasized text (headings, strong).
    text_strong: Color32,
    /// Default widget (button, card) fill.
    widget: Color32,
    /// Hovered widget fill.
    widget_hovered: Color32,
    /// Pressed/active widget fill.
    widget_active: Color32,
    /// Borders and separators.
    outline: Color32,
    /// The warm accent (selection, links, highlights).
    accent: Color32,
    /// Selection background behind text/items.
    selection: Color32,
}

const LIGHT: Palette = Palette {
    panel: Color32::from_rgb(248, 242, 232),
    window: Color32::from_rgb(252, 248, 240),
    faint: Color32::from_rgb(240, 231, 218),
    extreme: Color32::from_rgb(255, 253, 248),
    text: Color32::from_rgb(77, 66, 55),
    text_strong: Color32::from_rgb(56, 46, 37),
    widget: Color32::from_rgb(238, 228, 214),
    widget_hovered: Color32::from_rgb(231, 217, 197),
    widget_active: Color32::from_rgb(222, 203, 176),
    outline: Color32::from_rgb(213, 199, 181),
    accent: Color32::from_rgb(210, 110, 25),
    selection: Color32::from_rgb(247, 210, 156),
};

const DARK: Palette = Palette {
    panel: Color32::from_rgb(36, 31, 26),
    window: Color32::from_rgb(44, 38, 32),
    faint: Color32::from_rgb(48, 42, 35),
    extreme: Color32::from_rgb(28, 24, 20),
    text: Color32::from_rgb(226, 215, 199),
    text_strong: Color32::from_rgb(245, 238, 226),
    widget: Color32::from_rgb(56, 48, 40),
    widget_hovered: Color32::from_rgb(69, 59, 49),
    widget_active: Color32::from_rgb(82, 70, 57),
    outline: Color32::from_rgb(72, 62, 51),
    accent: Color32::from_rgb(255, 178, 90),
    selection: Color32::from_rgb(122, 84, 42),
};

/// Builds the egui visuals for the chosen theme.
#[must_use]
pub fn visuals(theme: ThemeChoice) -> Visuals {
    match theme {
        ThemeChoice::Light => apply(Visuals::light(), &LIGHT),
        ThemeChoice::Dark => apply(Visuals::dark(), &DARK),
    }
}

fn apply(mut visuals: Visuals, palette: &Palette) -> Visuals {
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.window;
    visuals.faint_bg_color = palette.faint;
    visuals.extreme_bg_color = palette.extreme;
    visuals.code_bg_color = palette.faint;
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.accent;
    visuals.window_stroke = Stroke::new(1.0, palette.outline);
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);

    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);

    let corner = CornerRadius::same(6);
    let widgets = &mut visuals.widgets;

    widgets.noninteractive.bg_fill = palette.panel;
    widgets.noninteractive.weak_bg_fill = palette.faint;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.outline);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);
    widgets.noninteractive.corner_radius = corner;

    widgets.inactive.bg_fill = palette.widget;
    widgets.inactive.weak_bg_fill = palette.widget;
    widgets.inactive.bg_stroke = Stroke::new(1.0, palette.outline);
    widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    widgets.inactive.corner_radius = corner;

    widgets.hovered.bg_fill = palette.widget_hovered;
    widgets.hovered.weak_bg_fill = palette.widget_hovered;
    widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent);
    widgets.hovered.fg_stroke = Stroke::new(1.5, palette.text_strong);
    widgets.hovered.corner_radius = corner;

    widgets.active.bg_fill = palette.widget_active;
    widgets.active.weak_bg_fill = palette.widget_active;
    widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);
    widgets.active.fg_stroke = Stroke::new(1.5, palette.text_strong);
    widgets.active.corner_radius = corner;

    widgets.open.bg_fill = palette.widget;
    widgets.open.weak_bg_fill = palette.widget;
    widgets.open.bg_stroke = Stroke::new(1.0, palette.accent);
    widgets.open.fg_stroke = Stroke::new(1.0, palette.text_strong);
    widgets.open.corner_radius = corner;

    visuals
}

/// Blends `base` toward the value-score hue: positive scores toward green (quick
/// win), zero toward warm amber (rated but balanced), negative toward red (money
/// pit). Intensity grows with the score's magnitude.
#[must_use]
pub fn value_tint(base: Color32, score: i8) -> Color32 {
    const GREEN: Color32 = Color32::from_rgb(110, 170, 80);
    const AMBER: Color32 = Color32::from_rgb(210, 110, 25);
    const RED: Color32 = Color32::from_rgb(200, 80, 60);
    let (target, strength) = if score > 0 {
        (GREEN, 0.14 * f32::from(score))
    } else if score < 0 {
        (RED, 0.14 * f32::from(-score))
    } else {
        (AMBER, 0.10)
    };
    base.lerp_to_gamma(target, strength)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_tints_shift_toward_their_hue() {
        let base = LIGHT.panel;
        let quick_win = value_tint(base, 2);
        let money_pit = value_tint(base, -2);
        let balanced = value_tint(base, 0);
        // Green pulls the green channel up relative to red; red does the opposite.
        assert!(
            i32::from(quick_win.g()) - i32::from(quick_win.r())
                > i32::from(base.g()) - i32::from(base.r())
        );
        assert!(
            i32::from(money_pit.r()) - i32::from(money_pit.g())
                > i32::from(base.r()) - i32::from(base.g())
        );
        // All three differ from the base and from each other.
        assert_ne!(quick_win, base);
        assert_ne!(money_pit, base);
        assert_ne!(balanced, base);
        assert_ne!(quick_win, money_pit);
        // Stronger scores tint more.
        assert_ne!(value_tint(base, 1), quick_win);
    }

    #[test]
    fn light_and_dark_use_their_palettes() {
        let light = visuals(ThemeChoice::Light);
        assert!(!light.dark_mode);
        assert_eq!(light.panel_fill, LIGHT.panel);
        assert_eq!(light.hyperlink_color, LIGHT.accent);

        let dark = visuals(ThemeChoice::Dark);
        assert!(dark.dark_mode);
        assert_eq!(dark.panel_fill, DARK.panel);
        assert_eq!(dark.hyperlink_color, DARK.accent);
    }
}
