//! A compact editor overlay for the shared AI preferences.
use std::collections::BTreeMap;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme as _, IndexPath, Sizable as _, Selectable as _, h_flex, v_flex};
use gpui_kit::*;
use practice::config::Config;
use practice::prompts::{Provider, Style};
use crate::workspace::{Focus, Workspace};

gpui_kit::actions!(ai, [SaveAi, CloseAi]);
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-enter", SaveAi, Some("AiForm")),
        KeyBinding::new("ctrl-enter", SaveAi, Some("AiForm > Input")),
        KeyBinding::new("escape", CloseAi, Some("AiForm")),
        KeyBinding::new("escape", CloseAi, Some("AiForm > Input")),
    ]);
}

#[derive(Default)]
pub struct State {
    pub form: Option<Entity<AiForm>>,
    return_focus: Option<Focus>,
}

pub fn open(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    ws.release_update.close();
    crate::language_picker::close(ws, window, cx);
    let weak = cx.weak_entity();
    let config = ws.config.clone();
    let return_focus = ws.focus_area;
    let form = cx.new(|cx| AiForm::new(weak, config, return_focus, window, cx));
    let focus = form.read(cx).instructions.clone();
    ws.ai.return_focus = Some(return_focus);
    ws.ai.form = Some(form);
    cx.notify();
    window.defer(cx, move |window, cx| focus.update(cx, |state, cx| state.focus(window, cx)));
}

pub fn close(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    ws.ai.form = None;
    if let Some(focus) = ws.ai.return_focus.take() {
        match focus {
            Focus::Editor | Focus::Omnibar => ws.focus_editor(window, cx),
            focus => ws.focus_nav(focus, window, cx),
        }
    }
    cx.notify();
}

pub fn status(ws: &Workspace, cx: &mut Context<Workspace>) -> impl IntoElement {
    use gpui_kit::component::popover::Popover;
    let form = ws.ai.form.clone();
    Popover::new("status-ai-popup").anchor(Anchor::BottomRight).offset(px(8.))
        .open(form.is_some())
        .on_open_change(cx.listener(|this, open, window, cx| {
            if *open { self::open(this, window, cx); } else { close(this, window, cx); }
        }))
        .trigger(Button::new("status-ai").ghost().xsmall()
            .icon(crate::brand::icon(ws.config.prompt_provider).xsmall()).label(ws.config.prompt_style.label())
            .accessibility_label("AI assist")
            .tooltip_with_action("AI assist", &crate::actions::ConfigureAi, Some(crate::actions::WORKSPACE)))
        .content(move |_, _, _| div().w(px(420.)).max_w_full().children(form.clone()))
}

pub struct AiForm {
    workspace: WeakEntity<Workspace>,
    return_focus: Focus,
    provider: Provider,
    style: Entity<SelectState<Vec<&'static str>>>,
    instructions: Entity<TextareaState>,
    current_style: Style,
    drafts: BTreeMap<String, String>,
    _subscription: Subscription,
}

impl AiForm {
    fn new(workspace: WeakEntity<Workspace>, config: Config, return_focus: Focus, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let provider = config.prompt_provider;
        let index = Style::ALL.iter().position(|style| *style == config.prompt_style).unwrap_or(0);
        let style = cx.new(|cx| SelectState::new(Style::ALL.iter().map(|style| style.label()).collect::<Vec<_>>(),
            Some(IndexPath::default().row(index)), window, cx));
        let value = config.prompt_instructions.get(config.prompt_style.id()).cloned().unwrap_or_else(|| config.prompt_style.instructions().into());
        let instructions = cx.new(|cx| TextareaState::new(window, cx).rows(3).default_value(value));
        let subscription = cx.subscribe_in(&style, window,
            |this, _, event: &SelectEvent<Vec<&'static str>>, window, cx| {
                if let SelectEvent::Confirm(Some(label)) = event {
                    let Some(style) = Style::ALL.iter().find(|style| style.label() == *label).copied() else { return; };
                    this.store_draft(cx);
                    this.current_style = style;
                    let value = this.drafts.get(style.id()).cloned().unwrap_or_else(|| style.instructions().into());
                    this.instructions.update(cx, |input, cx| input.set_value(value, window, cx));
                    cx.notify();
                }
            });
        Self { workspace, return_focus, provider, style, instructions, current_style: config.prompt_style,
            drafts: config.prompt_instructions, _subscription: subscription }
    }

    fn store_draft(&mut self, cx: &App) {
        let value = self.instructions.read(cx).value().trim().to_owned();
        if value.is_empty() || value == self.current_style.instructions() { self.drafts.remove(self.current_style.id()); }
        else { self.drafts.insert(self.current_style.id().into(), value); }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.store_draft(cx);
        let style = self.current_style;
        let provider = self.provider;
        let instructions = self.drafts.clone();
        let return_focus = self.return_focus;
        let saved = self.workspace.update(cx, |ws, cx| {
            let mut config = ws.config.clone();
            config.prompt_provider = provider;
            config.prompt_style = style;
            config.prompt_instructions = instructions;
            match config.save() {
                Ok(()) => { ws.config = config; ws.ai.form = None; ws.ai.return_focus = None; cx.notify(); match return_focus { Focus::Editor | Focus::Omnibar => ws.focus_editor(window, cx), focus => ws.focus_nav(focus, window, cx) }; ws.flash("AI preferences saved", cx); true }
                Err(error) => { ws.toast(gpui_kit::component::notification::Notification::error(error.to_string()), window, cx); false }
            }
        }).unwrap_or(false);
        if saved { cx.notify(); }
    }
}

impl Render for AiForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        v_flex().key_context("AiForm").gap_3()
            .child(h_flex().items_center().justify_between()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("AI assist"))
                .child(Button::new("close-ai").ghost().xsmall().icon(gpui_kit::assets::IconName::X)
                    .accessibility_label("Close AI assist").on_click(cx.listener(|this, _, window, cx| {
                        let _ = this.workspace.update(cx, |ws, cx| close(ws, window, cx));
                    }))))
            .on_action(cx.listener(|this, _: &CloseAi, window, cx| {
                let _ = this.workspace.update(cx, |ws, cx| close(ws, window, cx));
            }))
            .on_action(cx.listener(|this, _: &SaveAi, window, cx| this.save(window, cx)))
            .child(h_flex().gap_3()
                .child(h_flex().flex_1().gap_1().children(Provider::ALL.into_iter().enumerate().map(|(index, provider)| {
                    Button::new(("popup-provider", index)).ghost().icon(crate::brand::icon(provider)).selected(provider == self.provider)
                        .accessibility_label(provider.label()).tooltip(provider.label())
                        .on_click(cx.listener(move |this, _, _, cx| { this.provider = provider; cx.notify(); }))
                })))
                .child(v_flex().flex_1().gap_1().child(div().text_xs().text_color(muted).child("Prompt style")).child(Select::new(&self.style))))
            .child(v_flex().gap_1().child(div().text_xs().text_color(muted).child("Instructions")).child(Textarea::new(&self.instructions)))
            .child(h_flex().items_center().justify_end()
                .child(Button::new("save-ai").primary().icon(gpui_kit::assets::IconName::Check).accessibility_label("Save AI preferences").tooltip("Save · Ctrl+Enter").on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))))
    }
}
