//! Shared tooltip presentation through native hover dispatch.
use std::{path::Path, time::Duration};
use anyhow::{Result, ensure};
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::component::button::Button;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::test::TestWindowExt as _;

struct Fixture;
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().key_context("Workspace").size_full().bg(cx.theme().background).justify_center().items_end().p_4()
            .child(div().id("tooltip-long-trigger").test_support().p_2().child("I'm stuck")
                .tooltip(|window, cx| Tooltip::new(practice::assist::Action::Stuck.instructions()).build(window, cx)))
            .child(h_flex().gap_2()
                .child(Button::new("tooltip-short-trigger").label("Short").tooltip("New chat"))
                .child(Button::new("tooltip-key-trigger").label("Shortcut")
                    .tooltip_with_action("Hide AI", &crate::actions::ToggleRight, Some(crate::actions::WORKSPACE))))
            .child(div().id("tooltip-away").test_support().w_full().h_12())
    }
}

pub(super) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<()> {
    cx.update(|cx| crate::actions::bind_keys(&practice::config::Config::default(), cx));
    for (name, width, theme, zoom) in [("regular", 600., "Vesper", 1.), ("compact", 260., "Vesper", 1.), ("light", 600., "Solarized Light", 1.), ("zoom", 360., "Vesper", 1.4)] {
        cx.update(|cx| { crate::theme::apply(theme, cx); crate::theme::set_zoom(zoom, cx); });
        let handle = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(point(px(0.), px(0.)), size(px(width), px(600.))))),
            show: false, focus: false, ..Default::default()
        }, cx, |_, cx| cx.new(|_| Fixture)))?.0.into();
        cx.run_until_parked();
        for (kind, trigger) in [("long", "tooltip-long-trigger"), ("short", "tooltip-short-trigger"), ("key", "tooltip-key-trigger")] {
            cx.update_window(handle, |_, window, cx| window.hover(trigger, cx))?;
            cx.advance_clock(Duration::from_millis(700));
            cx.run_until_parked();
            // Native hover tooltips and transitions also use wall-clock time.
            std::thread::sleep(Duration::from_millis(700));
            cx.update_window(handle, |_, window, cx| -> Result<()> {
                window.render_frame(cx);
                let content = window.find("tooltip-content").bounds();
                let frame = window.find("tooltip-frame").bounds();
                ensure!(frame.left() >= px(0.) && frame.right() <= window.viewport_size().width && frame.top() >= px(0.) && frame.bottom() <= window.viewport_size().height, "{name}/{kind}: tooltip surface escaped the viewport: {frame:?}");
                if kind == "key" {
                    let key = window.find("tooltip-keybinding").bounds();
                    ensure!(key.size.width > px(0.) && key.left() >= content.right() && key.right() < frame.right(), "{name}: shortcut keycaps overlap or escape");
                }
                ensure!(content.left() >= px(0.) && content.right() <= window.viewport_size().width, "{name}/{kind}: tooltip escaped the viewport: {content:?}");
                ensure!(content.size.width <= window.rem_size() * 22., "{name}/{kind}: tooltip exceeded reading width");
                if kind == "long" { ensure!(content.size.height > window.rem_size() * 3., "{name}: long tooltip did not wrap"); }
                if kind == "short" { ensure!(content.size.width < window.rem_size() * 10., "{name}: short tooltip stretched"); }
                Ok(())
            })??;
            if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("tooltip-{kind}-{name}.png")))?; }
            cx.update_window(handle, |_, window, cx| window.hover("tooltip-away", cx))?;
            cx.advance_clock(Duration::from_millis(700));
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(700));
            cx.update_window(handle, |_, window, cx| -> Result<()> {
                window.render_frame(cx);
                ensure!(window.try_find("tooltip-content").is_none(), "{name}/{kind}: tooltip did not dismiss");
                Ok(())
            })??;
        }
        cx.update_window(handle, |_, window, _| window.remove_window())?;
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); });
    Ok(())
}
