use gpui_kit::component::{Theme, h_flex};
use practice::viz::Var;
use super::style::tone_colors;
use gpui_kit::*;

pub(super) fn render(vars: &[Var], theme: &Theme) -> AnyElement {
    h_flex().flex_wrap().gap_1p5().max_w(px(640.))
        .children(vars.iter().map(|v| {
            let (bg, border, fg) = tone_colors(v.tone, theme);
            h_flex().h(px(24.)).px_2().gap_1p5().rounded_md().bg(bg).border_1().border_color(border).text_xs()
                .child(div().text_color(theme.muted_foreground).child(v.name.clone()))
                .child(div().text_color(fg).font_family(theme.mono_font_family.clone()).max_w(px(220.)).truncate().child(v.value.clone()))
        }))
        .into_any_element()
}
