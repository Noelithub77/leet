//! First-run walkthrough with replay; execution remains user-controlled.

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

use crate::workspace::{Center, Focus, Workspace};

gpui_kit::actions!(tour, [Next, Back, Skip]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Next, Some("TourCard")),
        KeyBinding::new("backspace", Back, Some("TourCard")),
        KeyBinding::new("escape", Skip, Some("GuidedTour")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step { Problem, Explorer, Description, Editor, Tests, Assist, Debugger, Playback }

pub(crate) const STEP_COUNT: usize = 8;
const STEPS: [Step; STEP_COUNT] = [Step::Problem, Step::Explorer, Step::Description, Step::Editor, Step::Tests, Step::Assist, Step::Debugger, Step::Playback];

#[derive(Clone, Copy)]
struct Layout { left: bool, home_sidebar: bool, right: bool, bottom: bool, zen: bool, description: bool, history: bool, debug: bool, center: Center, had_problem: bool }

pub struct Tour { index: usize, layout: Layout, focus: FocusHandle, practicing: bool }

impl Workspace {
    pub fn start_default_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.config.onboarding_completed && self.db.get("guided-tour-v1").ok().flatten().as_deref() != Some("dismissed") {
            self.start_tour(window, cx);
        }
    }

    pub fn start_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tour.is_some() || self.onboarding.is_some() { return; }
        self.save_now(cx);
        self.tour = Some(Tour { index: 0, focus: cx.focus_handle(), practicing: false, layout: Layout {
            left: self.left, home_sidebar: self.home.sidebar, right: self.right, bottom: self.bottom, zen: self.zen, description: self.description,
            history: self.history_mode, debug: self.debug_mode, center: self.center, had_problem: self.session.is_some(),
        }});
        self.tour_step(window, cx);
    }

    pub fn tour_move(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tour) = self.tour.as_ref() else { return };
        if delta > 0 && !self.session.as_ref().is_some_and(|session| session.question.is_some()) { return; }
        let next = (tour.index as isize + delta).max(0) as usize;
        if next >= STEPS.len() { self.end_tour(window, cx); return; }
        if let Some(tour) = self.tour.as_mut() { tour.index = next; tour.practicing = false; }
        self.tour_step(window, cx);
    }

    pub fn practice_tour_step(&mut self, cx: &mut Context<Self>) {
        if let Some(tour) = self.tour.as_mut() { tour.practicing = true; cx.notify(); }
    }

    fn tour_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tour) = &self.tour else { return };
        let (step, focus) = (STEPS[tour.index], tour.focus.clone());
        self.zen = false;
        if step == Step::Problem {
            self.center = Center::Home;
            self.history_mode = false;
            self.home.sidebar = true;
        } else {
            self.center = Center::Editor;
            self.left = step == Step::Explorer;
            self.right = step == Step::Assist;
            self.bottom = step == Step::Tests;
            self.description = matches!(step, Step::Description | Step::Assist);
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
        if let Err(error) = self.db.set("guided-tour-v1", "dismissed") {
            self.toast(gpui_kit::component::notification::Notification::error(format!("Tour preference not saved: {error}")), window, cx);
        }
        let layout = tour.layout;
        self.home.sidebar = layout.home_sidebar;
        self.left = layout.left; self.right = layout.right; self.bottom = layout.bottom;
        self.zen = layout.zen; self.description = layout.description; self.history_mode = layout.history;
        self.center = if !layout.had_problem && self.session.is_some() { Center::Editor } else { layout.center };
        self.save_layout();
        if layout.debug && self.session.is_some() { self.preview_debug(window, cx); }
        else { self.set_debug(false, window, cx); }
        if self.center == Center::Editor { self.focus_editor(window, cx); }
        else { self.focus_nav(match self.center { Center::Home => Focus::Home, Center::Settings => Focus::Settings, Center::Roadmap => Focus::Roadmap, _ => Focus::Home }, window, cx); }
        cx.notify();
    }
}

pub fn button(workspace: &Workspace, cx: &mut Context<Workspace>) -> AnyElement {
    if workspace.tour.is_some() {
        div().into_any_element()
    } else {
        Button::new("guided-tour").ghost().xsmall().icon(IconName::Info).tooltip("Guided tour").accessibility_label("Guided tour")
            .disabled(workspace.onboarding.is_some())
            .on_click(cx.listener(|this, _, window, cx| this.start_tour(window, cx))).into_any_element()
    }
}

pub fn panel(workspace: &Workspace, cx: &mut Context<Workspace>) -> Option<AnyElement> {
    if workspace.omni.open { return None; }
    let tour = workspace.tour.as_ref()?;
    let step = STEPS[tour.index];
    let (title, tip, icon, command) = match step {
        Step::Problem => ("Pick your first problem", "Choose a problem in Explorer, or search by name.", IconName::Search, "FindProblem"),
        Step::Explorer => ("Toggle the left sidebar", "Hide or show Explorer to make room for your solution.", IconName::PanelLeft, "ToggleLeft"),
        Step::Description => ("Toggle the problem description", "Keep the statement nearby, or hide it while you code.", IconName::FileText, "ToggleDescription"),
        Step::Editor => ("Write your solution", "Focus the editor and try a few lines of code.", IconName::Code, "FocusEditor"),
        Step::Tests => ("Run your tests", "Run the samples and inspect each result below the editor.", IconName::Play, "RunTests"),
        Step::Assist => ("Toggle the right sidebar", "Show or hide AI for hints, explanations, and solution reviews.", IconName::PanelRight, "ToggleRight"),
        Step::Debugger => ("See your code in motion", "Switch between Code and Debug to inspect your solution.", IconName::Code, "ToggleDebug"),
        Step::Playback => ("Explore every step", "Follow variables and output as your solution runs.", IconName::Play, ""),
    };
    let theme = cx.theme().clone();
    let can_next = workspace.session.as_ref().is_some_and(|session| session.question.is_some());
    let index = tour.index;
    let practicing = tour.practicing;
    let action = crate::actions::COMMANDS.iter().find(|action| action.id == command).map(|command| command.action);
    let card = v_flex().id("guided-tour-card").key_context("TourCard").track_focus(&tour.focus)
        .w(rems(if practicing { 22. } else { 26. })).max_w_full().p_5().gap_4().rounded_lg().bg(theme.popover).text_color(theme.popover_foreground).border_1().border_color(theme.border).shadow_lg()
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { if let Some(tour) = &this.tour { tour.focus.focus(window, cx); } }))
        .child(h_flex().justify_between().items_center()
            .child(h_flex().gap_2()
                .child(gpui_kit::component::Icon::new(icon).text_color(theme.primary))
                .child(div().text_sm().text_color(theme.muted_foreground).child("Guided tour")))
            .child(Button::new("skip-tour").ghost().xsmall().label("Skip").tooltip("Skip tour")
                .on_click(cx.listener(|this, _, window, cx| this.end_tour(window, cx)))))
        .when(!practicing, |card| card.child(v_flex().gap_2()
            .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(div().text_sm().text_color(theme.muted_foreground).child(tip))))
        .when(practicing, |card| card.child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(title)))
        .when(!practicing, |card| card.child(if step == Step::Playback {
            v_flex().gap_2()
                .child(h_flex().justify_between().child(div().text_sm().child("Steps")).child(crate::view::key("left right")))
                .child(h_flex().justify_between().child(div().text_sm().child("Cases")).child(crate::view::key("up down")))
                .child(Button::new("tour-play").outline().small().label("Play / pause").child(crate::view::key("space"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.practice_tour_step(cx);
                        this.debugger.update(cx, |debugger, cx| debugger.focus(window, cx));
                        window.dispatch_action(Box::new(crate::debug_view::PlayPause), cx);
                    }))).into_any_element()
        } else {
            Button::new("tour-try").outline().label(match step { Step::Problem => "Find a problem", Step::Explorer => "Toggle Explorer", Step::Description => "Toggle description", Step::Editor => "Focus editor", Step::Tests => "Run tests", Step::Assist => "Toggle AI", _ => "Switch Code / Debug" })
                .child(crate::view::key(crate::actions::key_for(command, &workspace.config)))
                .on_click(move |_, window, cx| { if let Some(action) = action { window.dispatch_action(action(), cx); } }).into_any_element()
        }))
        .child(h_flex().gap_1().children((0..STEPS.len()).map(|step| div().flex_1().h_1().rounded_full()
            .bg(if step <= index { theme.primary } else { theme.muted }))))
        .child(h_flex().justify_between().items_center()
            .child(Button::new("tour-back").ghost().small().label("Back").disabled(tour.index == 0).child(crate::view::key("backspace"))
                .on_click(cx.listener(|this, _, window, cx| this.tour_move(-1, window, cx))))
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{} of {}", index + 1, STEPS.len())))
            .child(Button::new("tour-next").primary().small().label(if tour.index + 1 == STEPS.len() { "Done" } else { "Next" }).disabled(!can_next)
                .child(crate::view::key("enter")).tooltip(if can_next { "Continue tour" } else { "Open a problem first" })
                .on_click(cx.listener(|this, _, window, cx| this.tour_move(1, window, cx)))))
        .when(!practicing, |card| card.child(h_flex().gap_2().justify_center().text_xs().text_color(theme.muted_foreground).child(crate::view::key("escape")).child("to skip")))
        .with_animation(SharedString::from(format!("tour-step-{index}-{practicing}")), Animation::new(Duration::from_millis(260)).with_easing(|t| 1. - (1. - t).powi(3)), |card, t| card.opacity(t).mt(rems((1. - t) * 0.75)));
    Some(div().absolute().inset_0().flex().p_6()
        .when(!practicing, |overlay| overlay.items_center().justify_center())
        .when(practicing, |overlay| overlay.items_start().justify_end().pt_16())
        .child(card).into_any_element())
}
