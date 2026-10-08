//! Native chat composer and chronological per-problem conversation.

use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Textarea;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};

use super::{Assist, Action, Phase, SendChat, Target, icon};

impl Assist {
    pub(super) fn sync_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer_slug == self.slug { return; }
        if let Some(slug) = self.composer_slug.take() { self.drafts.insert(slug, self.composer.read(cx).value().to_string()); }
        let draft = self.slug.as_ref().and_then(|slug| self.drafts.remove(slug)).unwrap_or_default();
        self.composer.update(cx, |input, cx| input.set_value(draft, window, cx));
        self.composer_slug = self.slug.clone();
        self.scroll.scroll_to_bottom();
    }

    pub fn has_instructions(&self, cx: &App) -> bool { !self.composer.read(cx).value().trim().is_empty() }

    fn run_action(&self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| { let _ = workspace.update(cx, |ws, cx| ws.run_assist(action, window, cx)); });
    }

    pub(super) fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.read(cx).value().trim().is_empty() { return; }
        if self.runs.iter().any(|run| Some(&run.slug) == self.slug.as_ref() && (run.phase.active() || run.phase == Phase::Confirm)) { return; }
        self.run_action(Action::Ask, window, cx);
    }
}

impl Render for Assist {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let workspace = self.workspace.upgrade();
        let current = workspace.as_ref().and_then(|ws| ws.read(cx).session.as_ref().map(|s| s.slug.clone()));
        if current != self.slug { self.set_problem(current.as_deref(), cx); }
        let (has_problem, has_attempt) = workspace.as_ref().map_or((false, false), |ws| {
            let ws = ws.read(cx);
            (ws.session.as_ref().is_some_and(|s| s.question.is_some()), ws.has_attempt(cx))
        });
        let web = workspace.as_ref().map(|ws| matches!(self.target(&ws.read(cx).config), Target::Web(_))).unwrap_or(true);
        self.sync_composer(window, cx);
        let workspace = self.workspace.clone();
        let menu = Button::new("chat-actions").ghost().small().icon(IconName::Sparkles).label("Actions")
            .dropdown_menu(move |mut menu, _, _| {
                for action in Action::ALL {
                    let workspace = workspace.clone();
                    let disabled = !has_problem || (action.needs_attempt() && !has_attempt) || (web && action == Action::Solve);
                    menu = menu.item(PopupMenuItem::new(action.label()).icon(icon(action)).disabled(disabled)
                        .on_click(move |_, window, cx| { let _ = workspace.update(cx, |ws, cx| ws.run_assist(action, window, cx)); }));
                }
                menu
            });
        let slug = self.slug.clone();
        let cards: Vec<AnyElement> = self.runs.iter().rev().filter(|run| Some(&run.slug) == slug.as_ref()).map(|run| self.card(run, window, cx)).collect();
        let busy = self.runs.iter().any(|run| Some(&run.slug) == slug.as_ref() && (run.phase.active() || run.phase == Phase::Confirm));
        let empty = cards.is_empty();
        v_flex().key_context("AgentChat").size_full().min_h_0()
            .on_action(cx.listener(|this, _: &SendChat, window, cx| this.send(window, cx)))
            .child(v_flex().id("assist-cards").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).px_3().py_3().gap_3()
                .when(empty, |el| el.child(v_flex().flex_1().justify_center().items_center().gap_3().py_8()
                    .child(Icon::new(IconName::MessageCircle).size_6().text_color(theme.primary))
                    .child(div().text_sm().child(if !has_problem { "Open a problem to chat" } else { "Ask about this problem" }))
                    .children([Action::Explain, Action::Hints, Action::Analyze].into_iter().map(|action| {
                        Button::new(SharedString::from(format!("chat-suggestion-{}", action.id()))).ghost().small().icon(icon(action)).label(action.label())
                            .disabled(!has_problem || (action.needs_attempt() && !has_attempt))
                            .on_click(cx.listener(move |this, _, window, cx| this.run_action(action, window, cx)))
                    }))))
                .children(cards))
            .child(v_flex().p_3().gap_2().border_t_1().border_color(theme.border)
                .child(Textarea::new(&self.composer).disabled(!has_problem).aria_label("Message or action instructions"))
                .child(h_flex().gap_2().items_center().child(menu)
                    .child(div().flex_1().text_xs().text_color(theme.muted_foreground).child("Ctrl+Enter"))
                    .child(if busy {
                        Button::new("chat-send-stop").ghost().small().icon(IconName::CircleStop).tooltip("Stop current run").accessibility_label("Stop current run")
                            .on_click(cx.listener(|this, _, _, cx| { let ids: Vec<_> = this.runs.iter().filter(|run| Some(&run.slug) == this.slug.as_ref() && (run.phase.active() || run.phase == Phase::Confirm)).map(|run| run.id).collect(); for id in ids { this.stop(id, cx); } })).into_any_element()
                    } else {
                        Button::new("chat-send").primary().small().icon(if web { IconName::ExternalLink } else { IconName::ArrowUp })
                            .tooltip(if web { "Open question in web chat" } else { "Send question" }).accessibility_label("Send question")
                            .disabled(!has_problem || self.composer.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))).into_any_element()
                    })))
    }
}

