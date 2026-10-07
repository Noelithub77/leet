//! A status-bar picker that preserves each language's separate solution file.
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::popover::Popover;
use practice::language::Language;
use crate::workspace::{Focus, Workspace};

#[derive(Default)]
pub struct State {
    pub open: bool,
    pub loading: bool,
    picker: Option<Entity<LanguagePicker>>,
    return_focus: Option<Focus>,
}

pub(crate) fn close(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    ws.language_picker.open = false;
    ws.language_picker.picker = None;
    if let Some(focus) = ws.language_picker.return_focus.take() {
        match focus {
            Focus::Editor | Focus::Omnibar => ws.focus_editor(window, cx),
            focus => ws.focus_nav(focus, window, cx),
        }
    }
    cx.notify();
}

pub fn status(ws: &Workspace, cx: &mut Context<Workspace>) -> impl IntoElement {
    let language = ws.session.as_ref().map_or(ws.config.preferred_language, |session| session.language);
    let picker = ws.language_picker.picker.clone();
    let detail = ws.intelligence.detail(language);
    Popover::new("status-language-popup").anchor(Anchor::BottomRight).offset(px(8.))
        .open(ws.language_picker.open)
        .on_open_change(cx.listener(move |this, open, window, cx| {
            if *open {
                this.release_update.close();
                crate::ai::close(this, window, cx);
                this.language_picker.return_focus = Some(this.focus_area);
                let language = this.session.as_ref().map_or(this.config.preferred_language, |session| session.language);
                let workspace = cx.weak_entity();
                let picker = cx.new(|cx| LanguagePicker {
                    workspace, focus: cx.focus_handle(), selected: Language::ALL.iter().position(|item| *item == language).unwrap_or(0),
                });
                let focus = picker.read(cx).focus.clone();
                this.language_picker.picker = Some(picker);
                this.language_picker.open = true;
                cx.notify();
                window.defer(cx, move |window, cx| window.focus(&focus, cx));
            } else { close(this, window, cx); }
        }))
        .trigger(Button::new("status-language").ghost().xsmall()
            .icon(gpui_kit::component::Icon::default().path(format!("languages/{}.svg", language.id())).xsmall())
            .label(language.label()).accessibility_label("Switch language").tooltip(detail))
        .content(move |_, _, _| div().w(px(260.)).children(picker.clone()))
}

struct LanguagePicker {
    workspace: WeakEntity<Workspace>,
    focus: FocusHandle,
    selected: usize,
}

impl Render for LanguagePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.workspace.upgrade();
        let ws = workspace.as_ref().map(|workspace| workspace.read(cx));
        let current = ws.map_or(Language::Python, |ws| ws.session.as_ref().map_or(ws.config.preferred_language, |session| session.language));
        let busy = ws.is_some_and(|ws| ws.language_picker.loading || ws.session.as_ref().is_some_and(|session| session.running || matches!(session.judge, Some(crate::workspace::Judge::Running { .. }))));
        let loading = ws.is_some_and(|ws| ws.language_picker.loading);
        v_flex().key_context("LanguagePicker").track_focus(&self.focus).gap_1()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "up" => this.selected = (this.selected + Language::ALL.len() - 1) % Language::ALL.len(),
                    "down" => this.selected = (this.selected + 1) % Language::ALL.len(),
                    "enter" => {
                        let language = Language::ALL[this.selected];
                        let _ = this.workspace.update(cx, |ws, cx| ws.switch_language(language, window, cx));
                    }
                    "escape" => { let _ = this.workspace.update(cx, |ws, cx| close(ws, window, cx)); }
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(div().px_2().pb_2().text_sm().font_weight(FontWeight::SEMIBOLD).child(if loading { "Loading language…" } else { "Language" }))
            .children(Language::ALL.into_iter().enumerate().map(|(index, language)| {
                Button::new(("choose-language", index)).ghost().small().w_full().justify_between()
                    .icon(gpui_kit::component::Icon::default().path(format!("languages/{}.svg", language.id())))
                    .label(language.label()).selected(self.selected == index).disabled(busy)
                    .when(current == language, |button| button.child(gpui_kit::component::Icon::new(IconName::Check).xsmall()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let _ = this.workspace.update(cx, |ws, cx| ws.switch_language(language, window, cx));
                    }))
            }))
            .child(h_flex().pt_2().gap_2().text_xs().text_color(cx.theme().muted_foreground)
                .child(crate::view::key("up down"))
                .child(crate::view::key("enter")).child("Select")
                .child(crate::view::key("escape")).child("Close"))
    }
}
