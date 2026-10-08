//! Optional, keyboard-first tour. Panels change; execution remains user-controlled.

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::workspace::{Center, Workspace};

gpui_kit::actions!(tour, [Next, Back, Skip]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Next, Some("TourCard")),
        KeyBinding::new("backspace", Back, Some("TourCard")),
        KeyBinding::new("escape", Skip, Some("GuidedTour")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step { Problem, Editor, Tests, Assist, Debugger, Playback }

const STEPS: [Step; 6] = [Step::Problem, Step::Editor, Step::Tests, Step::Assist, Step::Debugger, Step::Playback];

#[derive(Clone, Copy)]
struct Layout { left: bool, right: bool, bottom: bool, zen: bool, assist: bool, history: bool, debug: bool, center: Center, had_problem: bool }

pub struct Tour { index: usize, layout: Layout, focus: FocusHandle }

impl Workspace {
    pub fn start_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tour.is_some() || self.onboarding.is_some() { return; }
        self.tour = Some(Tour { index: 0, focus: cx.focus_handle(), layout: Layout {
            left: self.left, right: self.right, bottom: self.bottom, zen: self.zen, assist: self.assist_open,
            history: self.history_mode, debug: self.debug_mode, center: self.center, had_problem: self.session.is_some(),
        }});
        self.tour_step(window, cx);
    }

    pub fn tour_move(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tour) = self.tour.as_ref() else { return };
        if delta > 0 && !self.session.as_ref().is_some_and(|session| session.question.is_some()) { return; }
        let next = (tour.index as isize + delta).max(0) as usize;
        if next >= STEPS.len() { self.end_tour(window, cx); return; }
        if let Some(tour) = self.tour.as_mut() { tour.index = next; }
        self.tour_step(window, cx);
    }

    fn tour_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tour) = &self.tour else { return };
        let (step, focus) = (STEPS[tour.index], tour.focus.clone());
        self.zen = false;
        if step == Step::Problem {
            self.show_home(window, cx);
        } else {
            self.center = Center::Editor;
            self.left = false;
            self.right = step == Step::Assist;
            self.bottom = step == Step::Tests;
            self.assist_open = step == Step::Assist;
            self.history_mode = false;
            if matches!(step, Step::Debugger | Step::Playback) { self.preview_debug(window, cx); }
            else { self.set_debug(false, window, cx); }
        }
        // The playback step leaves arrows with the debugger so users can try them.
        if step != Step::Playback {
            focus.focus(window, cx);
            cx.on_next_frame(window, move |this, window, cx| {
                if this.tour.as_ref().is_some_and(|tour| STEPS[tour.index] == step) { focus.focus(window, cx); }
            });
        }
        cx.notify();
    }

    pub fn end_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tour) = self.tour.take() else { return };
        let layout = tour.layout;
        self.left = layout.left; self.right = layout.right; self.bottom = layout.bottom;
        self.zen = layout.zen; self.assist_open = layout.assist; self.history_mode = layout.history;
        self.center = if !layout.had_problem && self.session.is_some() { Center::Editor } else { layout.center };
        if layout.debug && self.session.is_some() { self.preview_debug(window, cx); }
        else { self.set_debug(false, window, cx); }
        if self.center != Center::Editor { self.focus.focus(window, cx); }
        cx.notify();
    }
}

pub fn button(workspace: &Workspace, cx: &mut Context<Workspace>) -> AnyElement {
    if workspace.tour.is_some() {
        Button::new("skip-tour").ghost().xsmall().label("Skip tour").tooltip("Skip tour · Esc")
            .on_click(cx.listener(|this, _, window, cx| this.end_tour(window, cx))).into_any_element()
    } else {
        Button::new("guided-tour").ghost().xsmall().icon(IconName::Info).tooltip("Guided tour").accessibility_label("Guided tour")
            .disabled(workspace.onboarding.is_some())
            .on_click(cx.listener(|this, _, window, cx| this.start_tour(window, cx))).into_any_element()
    }
}

pub fn panel(workspace: &Workspace, cx: &mut Context<Workspace>) -> Option<AnyElement> {
    let tour = workspace.tour.as_ref()?;
    let step = STEPS[tour.index];
    let (title, command) = match step {
        Step::Problem => ("Open a problem", "FindProblem"),
        Step::Editor => ("Write your solution", "FocusEditor"),
        Step::Tests => ("Run tests", "RunTests"),
        Step::Assist => ("Ask Assist", "ToggleAssist"),
        Step::Debugger => ("Code ↔ Debug", "ToggleDebug"),
        Step::Playback => ("Step through", ""),
    };
    let theme = cx.theme();
    let can_next = workspace.session.as_ref().is_some_and(|session| session.question.is_some());
    Some(v_flex().id("guided-tour-card").key_context("TourCard").track_focus(&tour.focus)
        .absolute().top(px(60.)).when(step == Step::Assist, |el| el.left(px(20.)))
        .when(step != Step::Assist, |el| el.right(px(20.)))
        .w(px(252.)).p_3().gap_3().rounded_lg().bg(theme.background).border_1().border_color(theme.primary.opacity(0.6)).shadow_md()
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { if let Some(tour) = &this.tour { tour.focus.focus(window, cx); } }))
        .child(h_flex().gap_3().items_center()
            .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{}/{}", tour.index + 1, STEPS.len()))))
        .child(if step == Step::Playback {
            v_flex().gap_2()
                .child(h_flex().gap_2().child(crate::view::key("left right")).child(div().text_xs().child("steps")))
                .child(h_flex().gap_2().child(crate::view::key("up down")).child(div().text_xs().child("cases")))
                .child(h_flex().gap_2().child(crate::view::key("space")).child(div().text_xs().child("play / pause"))).into_any_element()
        } else { crate::view::key(crate::actions::key_for(command, &workspace.config)).into_any_element() })
        .child(h_flex().justify_between()
            .child(Button::new("tour-back").ghost().small().label("Back").disabled(tour.index == 0).tooltip("Back · Backspace")
                .on_click(cx.listener(|this, _, window, cx| this.tour_move(-1, window, cx))))
            .child(Button::new("tour-next").primary().small().label(if tour.index + 1 == STEPS.len() { "Done" } else { "Next" }).disabled(!can_next)
                .tooltip(if can_next { "Next · Enter" } else { "Open a problem first" })
                .on_click(cx.listener(|this, _, window, cx| this.tour_move(1, window, cx)))))
        .into_any_element())
}
