//! First-run walkthrough with replay; execution remains user-controlled.

use gpui_kit::assets::IconName;
use gpui_kit::base::{Disableable as _, FocusTrapElement as _};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, ThemeStyled as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

use crate::workspace::{Center, Focus, Workspace};

gpui_kit::actions!(tour, [Next, Back, Skip]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("right", Next, Some("TourCard")),
        KeyBinding::new("left", Back, Some("TourCard")),
        KeyBinding::new("escape", Skip, Some("GuidedTour")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step { Problem, Explorer, Description, Editor, Tests, Assist, Debugger, Playback }

pub(crate) const STEP_COUNT: usize = 8;
const STEPS: [Step; STEP_COUNT] = [Step::Problem, Step::Explorer, Step::Description, Step::Editor, Step::Tests, Step::Assist, Step::Debugger, Step::Playback];

#[derive(Clone, Copy)]
struct Layout { left: bool, home_sidebar: bool, right: bool, bottom: bool, zen: bool, description: bool, history: bool, debug: bool, center: Center, had_problem: bool }

pub struct Tour { index: usize, layout: Layout, focus: FocusHandle, practicing: bool, _keyboard: Subscription, _focus: [Subscription; 2], pending: Vec<Keystroke> }

impl Tour {
    #[cfg(feature = "gui-test")]
    pub(crate) fn index(&self) -> usize { self.index }
}

impl Step {
    fn command(self) -> &'static str {
        match self {
            Self::Problem => "FindProblem", Self::Explorer => "ToggleLeft", Self::Description => "ToggleDescription",
            Self::Editor => "FocusEditor", Self::Tests => "RunTests", Self::Assist => "ToggleRight",
            Self::Debugger => "ToggleDebug", Self::Playback => "",
        }
    }
}

impl Workspace {
    pub fn start_default_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.config.onboarding_completed && self.db.get("guided-tour-v1").ok().flatten().as_deref() != Some("dismissed") {
            self.start_tour(window, cx);
        }
    }

    pub fn start_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tour.is_some() || self.onboarding.is_some() { return; }
        self.save_now(cx);
        let workspace = cx.entity().downgrade();
        let handle = window.window_handle();
        let keyboard = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != handle { return; }
            let _ = workspace.update(cx, |this, cx| this.tour_key(event, window, cx));
        });
        let focus = cx.focus_handle();
        let focus_subscriptions = [
            cx.on_focus_in(&focus, window, |_, _, cx| cx.notify()),
            cx.on_focus_out(&focus, window, |_, _, window, cx| {
                cx.notify();
                let workspace = cx.entity().downgrade();
                window.defer(cx, move |window, cx| {
                    let _ = workspace.update(cx, |this, cx| {
                        if !this.omni.open && let Some(tour) = &this.tour {
                            tour.focus.focus(window, cx);
                            cx.notify();
                        }
                    });
                });
            }),
        ];
        self.tour = Some(Tour { index: 0, focus, practicing: false, _keyboard: keyboard, _focus: focus_subscriptions, pending: vec![], layout: Layout {
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
        if let Some(tour) = self.tour.as_mut() { tour.index = next; tour.practicing = false; tour.pending.clear(); }
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
        focus.focus(window, cx);
        cx.on_next_frame(window, move |this, window, cx| {
            if !this.omni.open && this.tour.as_ref().is_some_and(|tour| STEPS[tour.index] == step) { focus.focus(window, cx); cx.notify(); }
        });
        cx.notify();
    }

    fn tour_key(&mut self, event: &KeystrokeEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.omni.open { return; }
        let Some(tour) = self.tour.as_mut() else { return };
        let step = STEPS[tour.index];
        let key = &event.keystroke;
        let plain = key.modifiers == Modifiers::default();
        // Keep standard button navigation inside the card, without passing input to the editor.
        if plain && matches!(key.key.as_str(), "tab" | "enter" | "space")
            && step != Step::Playback && tour.focus.contains_focused(window, cx) { return; }
        if key.key == "tab" && key.modifiers.shift && tour.focus.contains_focused(window, cx) { return; }
        cx.stop_propagation();
        if plain && key.key == "escape" { self.end_tour(window, cx); return; }
        if plain && step == Step::Playback {
            if key.key == "space" { self.try_tour_step(window, cx); return; }
            let action: Option<Box<dyn Action>> = match key.key.as_str() {
                "left" => Some(Box::new(crate::debug_view::StepBack)),
                "right" => Some(Box::new(crate::debug_view::StepForward)),
                "up" => Some(Box::new(crate::debug_view::PrevCase)),
                "down" => Some(Box::new(crate::debug_view::NextCase)),
                _ => None,
            };
            if let Some(action) = action {
                self.debugger.update(cx, |debugger, cx| debugger.focus(window, cx));
                window.dispatch_action(action, cx);
                self.practice_tour_step(cx);
                return;
            }
        }
        if plain && matches!(key.key.as_str(), "left" | "right") {
            self.tour_move(if key.key == "left" { -1 } else { 1 }, window, cx);
            return;
        }
        tour.pending.push(key.clone());
        let mut partial = false;
        for shortcut in crate::actions::key_for(step.command(), &self.config).split('|').filter(|key| !key.is_empty()) {
            let Ok(binding) = KeyBinding::load(shortcut, Box::new(Next), None, false, None, cx.keyboard_mapper().as_ref()) else { continue };
            match binding.match_keystrokes(&tour.pending) {
                Some(false) => { tour.pending.clear(); self.try_tour_step(window, cx); return; }
                Some(true) => partial = true,
                None => {}
            }
        }
        if !partial { tour.pending.clear(); }
        tour.focus.focus(window, cx);
        cx.notify();
    }

    fn try_tour_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tour) = &self.tour else { return };
        let index = tour.index;
        let command = STEPS[index].command();
        let action: Box<dyn Action> = if STEPS[index] == Step::Playback {
            self.debugger.update(cx, |debugger, cx| debugger.focus(window, cx));
            Box::new(crate::debug_view::PlayPause)
        } else {
            let Some(action) = crate::actions::COMMANDS.iter().find(|action| action.id == command) else { return };
            tour.focus.focus(window, cx);
            (action.action)()
        };
        window.dispatch_action(action, cx);
        // Dispatch first so the real command runs before the next step sets up its layout.
        let workspace = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = workspace.update(cx, |this, cx| {
                if command != "FindProblem" && this.tour.as_ref().is_some_and(|tour| tour.index == index) {
                    this.tour_move(1, window, cx);
                }
            });
        });
    }

    pub fn resume_tour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.omni.open { return; }
        if self.tour.as_ref().is_some_and(|tour| STEPS[tour.index] == Step::Problem)
            && self.session.as_ref().is_some_and(|session| session.question.is_some()) {
            self.tour_move(1, window, cx);
        } else if let Some(tour) = &self.tour {
            tour.focus.focus(window, cx);
            cx.notify();
        }
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

pub fn panel(workspace: &Workspace, window: &mut Window, cx: &mut Context<Workspace>) -> Option<AnyElement> {
    if workspace.omni.open { return None; }
    let tour = workspace.tour.as_ref()?;
    let step = STEPS[tour.index];
    let (title, tip, icon, command) = match step {
        Step::Problem => ("Pick your first problem", "Choose a problem in Explorer, or search by name.", IconName::Search, "FindProblem"),
        Step::Explorer => ("Toggle the left sidebar", "Hide or show Explorer to make room for your solution.", IconName::PanelLeft, "ToggleLeft"),
        Step::Description => ("Toggle the problem description", "Keep the statement nearby, or hide it while you code.", IconName::FileText, "ToggleDescription"),
        Step::Editor => ("Write your solution", "Use the shortcut to focus your solution.", IconName::Code, "FocusEditor"),
        Step::Tests => ("Run your tests", "Run the samples and inspect each result below the editor.", IconName::Play, "RunTests"),
        Step::Assist => ("Toggle the right sidebar", "Show or hide AI for hints, explanations, and solution reviews.", IconName::PanelRight, "ToggleRight"),
        Step::Debugger => ("See your code in motion", "Switch between Code and Debug to inspect your solution.", IconName::Code, "ToggleDebug"),
        Step::Playback => ("Explore every step", "Follow variables and output as your solution runs.", IconName::Play, ""),
    };
    let theme = cx.theme().clone();
    let can_next = workspace.session.as_ref().is_some_and(|session| session.question.is_some());
    let index = tour.index;
    let practicing = tour.practicing;
    let card = v_flex().id("guided-tour-card").test_support().key_context("TourCard").track_focus(&tour.focus)
        .w(rems(if practicing { 22. } else { 26. })).max_w_full().p_5().gap_4().rounded_lg().bg(theme.popover).text_color(theme.popover_foreground).border_1().border_color(theme.border).shadow_lg()
        .when(tour.focus.contains_focused(window, cx), |card| card.focus_ring_style(window, cx))
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
        .child(if step == Step::Playback {
            v_flex().gap_2()
                .child(h_flex().justify_between().child(div().text_sm().child("Steps")).child(crate::view::key("left right")))
                .child(h_flex().justify_between().child(div().text_sm().child("Cases")).child(crate::view::key("up down")))
                .child(Button::new("tour-play").outline().small().label("Play / pause").child(crate::view::key("space"))
                    .on_click(cx.listener(|this, _, window, cx| this.try_tour_step(window, cx)))).into_any_element()
        } else {
            Button::new("tour-try").outline().label(match step { Step::Problem => "Find a problem", Step::Explorer => "Toggle Explorer", Step::Description => "Toggle description", Step::Editor => "Focus editor", Step::Tests => "Run tests", Step::Assist => "Toggle AI", _ => "Switch Code / Debug" })
                .child(crate::view::key(crate::actions::key_for(command, &workspace.config)))
                .on_click(cx.listener(|this, _, window, cx| this.try_tour_step(window, cx))).into_any_element()
        })
        .child(h_flex().gap_1().children((0..STEPS.len()).map(|step| div().flex_1().h_1().rounded_full()
            .bg(if step <= index { theme.primary } else { theme.muted }))))
        .child(h_flex().justify_between().items_center()
            .child(Button::new("tour-back").ghost().small().label("Prev").disabled(tour.index == 0).child(crate::view::key("left"))
                .on_click(cx.listener(|this, _, window, cx| this.tour_move(-1, window, cx))))
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{} of {}", index + 1, STEPS.len())))
            .child(Button::new("tour-next").primary().small().label(if tour.index + 1 == STEPS.len() { "Done" } else { "Next" }).disabled(!can_next)
                .child(crate::view::key("right")).tooltip(if can_next { "Continue tour" } else { "Open a problem first" })
                .on_click(cx.listener(|this, _, window, cx| this.tour_move(1, window, cx)))))
        .when(!practicing, |card| card.child(h_flex().gap_2().justify_center().text_xs().text_color(theme.muted_foreground).child(crate::view::key("escape")).child("to skip")))
        .focus_trap("guided-tour-focus", &tour.focus)
        .with_animation(SharedString::from(format!("tour-step-{index}-{practicing}")), Animation::new(Duration::from_millis(260)).with_easing(|t| 1. - (1. - t).powi(3)), |card, t| card.opacity(t).mt(rems((1. - t) * 0.75)));
    Some(div().absolute().inset_0().flex().p_6()
        .when(!practicing, |overlay| overlay.items_center().justify_center())
        .when(practicing, |overlay| overlay.items_start().justify_end().pt_16())
        .child(card).into_any_element())
}
