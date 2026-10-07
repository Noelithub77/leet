use std::time::Duration;
use gpui_kit::base::Spring;
use gpui_kit::component::Theme;
use practice::viz::Tone;
use gpui_kit::*;

pub(super) const CELL_H: f32 = 30.;
pub(super) const GAP: f32 = 4.;
pub(super) const NODE: f32 = 30.;

pub(super) fn motion() -> Spring { Spring::new(Duration::from_millis(360)) }

/// Fill, border, and text colors for a tone.
pub fn tone_colors(tone: Tone, theme: &Theme) -> (Hsla, Hsla, Hsla) {
    let tint = |color: Hsla, alpha: f32| (color.opacity(alpha), color.opacity(0.75), color);
    match tone {
        Tone::Default => (theme.secondary, theme.border, theme.foreground),
        Tone::Active => tint(theme.info, 0.2),
        Tone::Changed => tint(theme.warning, 0.2),
        Tone::Visited => (theme.muted, theme.muted_foreground.opacity(0.35), theme.muted_foreground),
        Tone::Done => tint(theme.success, 0.18),
        Tone::Muted => (theme.secondary.opacity(0.4), theme.border.opacity(0.4), theme.muted_foreground.opacity(0.6)),
        Tone::Warn => tint(theme.warning, 0.28),
        Tone::Error => tint(theme.danger, 0.22),
    }
}

/// Width that fits a value in the mono font.
pub(super) fn cell_w(value: &str) -> f32 { (value.chars().count() as f32 * 7.6 + 14.).clamp(30., 160.) }

pub(super) fn chip(value: &str, tone: Tone, width: f32, theme: &Theme) -> Div {
    let (bg, border, fg) = tone_colors(tone, theme);
    div().flex_shrink_0().w(px(width)).h(px(CELL_H)).flex().items_center().justify_center().rounded_md()
        .bg(bg).border_1().border_color(border).text_color(fg)
        .font_family(theme.mono_font_family.clone()).text_xs().truncate().child(value.to_owned())
}
