//! Video-style playback for step sequences: transport buttons, a draggable seek bar
//! with event markers, and speed. Shared by the debugger and AI walkthroughs.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub const SPEEDS: [f32; 4] = [0.5, 1., 2., 4.];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playback {
    pub index: usize,
    pub len: usize,
    pub playing: bool,
    /// Index into `SPEEDS`.
    pub speed: usize,
}

impl Playback {
    pub fn new(len: usize) -> Self { Self { index: 0, len, playing: false, speed: 1 } }

    pub fn seek(&mut self, index: usize) { self.index = index.min(self.len.saturating_sub(1)); }

    pub fn step(&mut self, delta: isize) {
        self.playing = false;
        self.seek((self.index as isize + delta).max(0) as usize);
    }

    pub fn toggle(&mut self) {
        if self.index + 1 >= self.len { self.index = 0; }
        self.playing = !self.playing && self.len > 1;
    }

    /// Advances one frame; returns whether playback continues.
    pub fn advance(&mut self) -> bool {
        if !self.playing { return false; }
        if self.index + 1 >= self.len { self.playing = false; return false; }
        self.index += 1;
        true
    }

    /// Milliseconds per frame at the current speed.
    pub fn interval_ms(&self) -> u64 { (650. / SPEEDS[self.speed.min(SPEEDS.len() - 1)]) as u64 }

    pub fn cycle_speed(&mut self) { self.speed = (self.speed + 1) % SPEEDS.len(); }
}

/// A point of interest on the seek bar.
#[derive(Clone, Debug)]
pub struct Marker {
    pub index: usize,
    pub color: Hsla,
}

pub enum Control {
    Seek(usize),
    Step(isize),
    Toggle,
    Speed,
}

/// Transport row. `on` receives every control; the owner updates its `Playback`.
pub fn controls(id: &str, playback: &Playback, markers: &[Marker], on: impl Fn(Control, &mut Window, &mut App) + 'static, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let on = Rc::new(on);
    let button = |key: &str, icon: IconName, tip: &'static str, control: fn() -> Control| {
        let on = on.clone();
        let shortcut = if id == "debugger" { match key { "first" => "Home", "prev" => "←", "play" => "Space", "next" => "→", "last" => "End", _ => "" } } else { "" };
        let hint = if shortcut.is_empty() { tip.to_owned() } else { format!("{tip} · {shortcut}") };
        Button::new(SharedString::from(format!("{id}-{key}"))).ghost().xsmall().icon(icon).tooltip(hint).accessibility_label(tip)
            .on_click(move |_, window, cx| on(control(), window, cx))
    };
    let len = playback.len.max(1);
    let fraction = if len <= 1 { 0. } else { playback.index as f32 / (len - 1) as f32 };
    let bounds: Rc<Cell<Bounds<Pixels>>> = Rc::default();
    let seek_to = {
        let bounds = bounds.clone();
        move |x: Pixels| -> usize {
            let b = bounds.get();
            let t = if b.size.width > px(0.) { ((x - b.origin.x) / b.size.width).clamp(0., 1.) } else { 0. };
            (t * (len - 1) as f32).round() as usize
        }
    };
    let track = {
        let (down, drag) = (on.clone(), on.clone());
        let (seek_down, seek_drag) = (seek_to.clone(), seek_to);
        let markers: Vec<Marker> = markers.to_vec();
        div().id(SharedString::from(format!("{id}-seek"))).flex_1().min_w(px(80.)).h(px(22.)).relative().cursor_pointer()
            .child(canvas({ let bounds = bounds.clone(); move |b, _, _| bounds.set(b) }, |_, _, _, _| ()).absolute().size_full())
            .child(div().absolute().left_0().right_0().top(px(9.)).h(px(4.)).rounded_full().bg(theme.muted))
            .child(div().absolute().left_0().top(px(9.)).h(px(4.)).rounded_full().bg(theme.primary.opacity(0.85)).w(relative(fraction)))
            .children(markers.iter().map(|m| {
                let at = if len <= 1 { 0. } else { m.index as f32 / (len - 1) as f32 };
                div().absolute().left(relative(at)).top(px(3.)).w(px(3.)).h(px(4.)).ml(px(-1.)).rounded_full().bg(m.color)
            }))
            .child(div().absolute().left(relative(fraction)).top(px(4.)).ml(px(-7.)).size(px(14.)).rounded_full()
                .bg(theme.foreground).border_2().border_color(theme.primary).shadow_sm())
            .on_mouse_down(MouseButton::Left, move |event, window, cx| down(Control::Seek(seek_down(event.position.x)), window, cx))
            .on_mouse_move(move |event, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) { drag(Control::Seek(seek_drag(event.position.x)), window, cx); }
            })
    };
    let speed = SPEEDS[playback.speed.min(SPEEDS.len() - 1)];
    h_flex().w_full().gap_1().items_center()
        .child(button("first", IconName::SkipBack, "First step", || Control::Seek(0)))
        .child(button("prev", IconName::ChevronLeft, "Previous step", || Control::Step(-1)))
        .child(button("play", if playback.playing { IconName::Pause } else { IconName::Play }, if playback.playing { "Pause" } else { "Play" }, || Control::Toggle))
        .child(button("next", IconName::ChevronRight, "Next step", || Control::Step(1)))
        .child(button("last", IconName::SkipForward, "Last step", || Control::Seek(usize::MAX)))
        .child(div().w_2())
        .child(track)
        .child(div().w(px(64.)).text_right().text_xs().text_color(theme.muted_foreground).font_family(theme.mono_font_family.clone())
            .child(format!("{}/{}", (playback.index + 1).min(playback.len), playback.len)))
        .child({
            let on = on.clone();
            Button::new(SharedString::from(format!("{id}-speed"))).ghost().xsmall().label(format!("{speed}×")).tooltip("Playback speed")
                .on_click(move |_, window, cx| on(Control::Speed, window, cx))
        })
        .when(playback.len == 0, |el| el.opacity(0.5))
        .into_any_element()
}

/// Applies a control to `playback`; returns whether a ticker should start.
pub fn apply(playback: &mut Playback, control: Control) -> bool {
    match control {
        Control::Seek(index) => { playback.playing = false; playback.seek(index); false }
        Control::Step(delta) => { playback.step(delta); false }
        Control::Toggle => { playback.toggle(); playback.playing }
        Control::Speed => { playback.cycle_speed(); false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn playback_clamps_restarts_and_stops_at_the_end() {
        let mut p = Playback::new(3);
        p.step(-1); assert_eq!(p.index, 0);
        p.seek(usize::MAX); assert_eq!(p.index, 2);
        p.toggle(); assert!(p.playing); assert_eq!(p.index, 0);
        assert!(p.advance()); assert!(p.advance()); assert!(!p.advance()); assert!(!p.playing);
        assert!(!Playback::new(1).advance());
        p.cycle_speed(); assert!(p.interval_ms() < 650);
    }
}
