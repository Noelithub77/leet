//! Native chat composer and chronological per-problem conversation.

use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonRounded, ButtonVariants as _};
use gpui_kit::component::input::{InputGroup, InputGroupAddon, InputGroupAddonAlignment, Textarea};
use gpui_kit::component::{ActiveTheme as _, FocusableExt as _, Theme, Icon, Sizable as _, h_flex, v_flex};

use super::{Assist, Action, Phase, SendChat, NewChat, Target, icon, accent};

impl Assist {
    pub(crate) fn refresh_context(&mut self, slug: Option<String>, has_problem: bool, has_attempt: bool, web: bool, window: &mut Window, cx: &mut Context<Self>) {
        let key = (slug, has_problem, has_attempt, web);
        if self.context_key.as_ref() == Some(&key) { return; }
        self.set_problem(key.0.as_deref(), cx);
        self.sync_composer(window, cx);
        self.context_key = Some(key);
        cx.notify();
    }

    pub(crate) fn sync_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer_thread == self.selected_thread { return; }
        self.persist_draft(cx);
        let draft = self.selected_thread.and_then(|id| self.db.chat_thread(id).ok()).map(|thread| thread.draft).unwrap_or_default();
        self.composer_citations = match self.selected_thread {
            Some(id) => match self.db.chat_draft_citations(id) { Ok(citations) => citations, Err(error) => { self.conversation_error = Some(error.to_string()); vec![] } },
            None => vec![],
        };
        self.composer_thread = self.selected_thread;
        self.composer.update(cx, |input, cx| input.set_value(draft, window, cx));
        self.scroll.scroll_to_bottom();
    }

    /// Keeps the prompt a one-line pill until focused, then lets it grow with its content.
    pub(super) fn fit_composer(&mut self, window: &Window, cx: &mut Context<Self>) {
        let expanded = self.composer.focus_handle(cx).is_focused(window);
        if expanded == self.composer_expanded { return; }
        self.composer_expanded = expanded;
        self.composer.update(cx, |input, cx| if expanded { input.set_auto_grow(3, 10, cx) } else { input.set_auto_grow(1, super::COMPOSER_IDLE_ROWS, cx) });
    }

    fn run_action(&self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| { let _ = workspace.update(cx, |ws, cx| ws.run_assist(action, window, cx)); });
    }

    pub(super) fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.read(cx).value().trim().is_empty() && self.composer_citations.is_empty() && self.editing.is_none() { return; }
        if self.runs.iter().any(|run| Some(run.thread_id) == self.selected_thread && (run.phase.active() || run.phase == Phase::Confirm)) { return; }
        self.ai_tab = 2;
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
        self.fit_composer(window, cx);
        let slug = self.slug.clone();
        let cards: Vec<AnyElement> = self.runs.iter().rev().filter(|run| Some(run.thread_id) == self.selected_thread).map(|run| self.card(run, window, cx)).collect();
        let busy = self.runs.iter().any(|run| Some(run.thread_id) == self.selected_thread && (run.phase.active() || run.phase == Phase::Confirm));
        let empty = cards.is_empty();
        let review = self.runs.iter().filter(|run| Some(&run.slug) == slug.as_ref() && (run.action == Action::Review || matches!(run.answer, Some(super::Answer::Review(_))))).max_by_key(|run| run.message_id)
            .map(|run| self.card(run, window, cx));
        v_flex().key_context("AgentChat").relative().size_full().min_h_0().text_xs()
            .on_action(cx.listener(|this, _: &SendChat, window, cx| this.send(window, cx)))
            .on_action(cx.listener(|this, _: &NewChat, window, cx| this.new_thread(window, cx)))
            .child(self.chat_header(cx))
            .when(self.ai_tab == 1, |el| el.child(v_flex().id("ai-review").test_support().flex_1().min_h_0().overflow_y_scroll().px_3().py_3().gap_3()
                .when(review.is_none(), |area| area.justify_center())
                .child(v_flex().w_full().max_w(px(440.)).flex_none().gap_3().mx_auto()
                .when(review.is_none(), |area| area.child(review_placeholder(&theme)))
                .child(Button::new("analyse-my-solution").primary().small().icon(IconName::Sparkles).label("Run analysis")
                    .tooltip("Check correctness, bugs, complexity, improvements, and a visual dry run")
                    .disabled(!has_problem || !has_attempt || busy)
                    .on_click(cx.listener(|this, _, window, cx| this.run_action(Action::Review, window, cx)))))
                .children(review)))
            .when(self.ai_tab == 2, |el| el.child(v_flex().id("assist-cards").test_support().flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).px_3().pt_3().pb_4().gap_5()
                .when(empty, |area| area.justify_center()
                .child(div().id("ai-action-grid").test_support().flex_none().w_full().max_w(px(300.)).mx_auto().grid().grid_cols(3).gap_1p5()
                .children([Action::Hints, Action::Stuck, Action::Explain, Action::Visualize, Action::Pattern, Action::Solve].into_iter().map(|action| {
                    let color = accent(action, &theme);
                    let disabled = !has_problem || (action.needs_attempt() && !has_attempt) || (web && action == Action::Solve);
                    v_flex().id(SharedString::from(format!("ai-action-{}", action.id()))).w_full().min_w_0().h(px(72.)).items_center().justify_center().gap_1p5().rounded_lg()
                        .opacity(if disabled { 0.35 } else { 1. }).when(!disabled, |tile| tile.cursor_pointer().hover(|tile| tile.bg(theme.secondary)))
                        .child(div().size(px(32.)).rounded_lg().border_1().border_color(color.opacity(0.2)).bg(color.opacity(0.12)).flex().items_center().justify_center()
                            .child(Icon::new(icon(action)).size_4().text_color(color)))
                        .child(div().text_size(super::META).child(action.label()))
                        .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(action.instructions()).build(window, cx))
                        .on_click(cx.listener(move |this, _, window, cx| { if !disabled { this.run_action(action, window, cx); } }))
                }))))
                .children(cards)))
            .child(self.render_composer(has_problem, web, busy, cx))
            .children(self.thread_popover(cx))
    }
}

impl Assist {
    pub(super) fn render_composer(&self, has_problem: bool, web: bool, busy: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex().flex_none().px_3().pb_3().pt_1().gap_1p5()
                .when_some(self.conversation_error.clone(), |el, error| el.child(div().text_color(theme.danger).child(error)))
                .when(self.editing.is_some(), |el| el.child(h_flex().items_center().child(div().flex_1().text_color(theme.muted_foreground).child("Edit as a new branch"))
                    .child(Button::new("cancel-chat-edit").ghost().xsmall().icon(IconName::X).tooltip("Cancel edit").on_click(cx.listener(|this, _, window, cx| this.cancel_edit(window, cx))))))
                .child(InputGroup::new("chat-composer").disabled(!has_problem).focus_ring(false).rounded(px(16.)).text_xs()
                    .input(Textarea::new(&self.composer).min_h_0().aria_label("Message or action instructions"))
                    .when(!self.composer_citations.is_empty(), |group| group.addon(InputGroupAddon::new("chat-citations").align(InputGroupAddonAlignment::BlockStart)
                        .child(h_flex().gap_1().flex_wrap().children(self.composer_citations.iter().enumerate().map(|(index, citation)| {
                            h_flex().gap_0p5().items_center().child(super::selection::citation_pill(("draft-citation", super::selection::citation_key(citation)), citation, &theme))
                                .child(Button::new(("remove-citation", super::selection::citation_key(citation))).ghost().xsmall().icon(IconName::X).accessibility_label("Remove citation")
                                    .on_click(cx.listener(move |this, _, _, cx| { this.composer_citations.remove(index); this.persist_draft(cx); cx.notify(); })))
                        })))))
                    .addon(InputGroupAddon::new("chat-composer-action").align(InputGroupAddonAlignment::InlineEnd).self_end().pb_1().pr_1()
                    .child(if busy {
                        Button::new("chat-send-stop").ghost().xsmall().rounded(ButtonRounded::Large).icon(IconName::CircleStop).tooltip("Stop current run").accessibility_label("Stop current run")
                            .on_click(cx.listener(|this, _, _, cx| { let ids: Vec<_> = this.runs.iter().filter(|run| Some(&run.slug) == this.slug.as_ref() && (run.phase.active() || run.phase == Phase::Confirm)).map(|run| run.id).collect(); for id in ids { this.stop(id, cx); } })).into_any_element()
                    } else {
                        Button::new("chat-send").primary().xsmall().rounded(ButtonRounded::Large).icon(if web { IconName::ExternalLink } else { IconName::ArrowUp })
                            .tooltip(if web { "Open question in web chat" } else { "Send question" }).accessibility_label("Send question")
                            .disabled((!has_problem) || (self.editing.is_none() && self.composer.read(cx).value().trim().is_empty() && self.composer_citations.is_empty()))
                            .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))).into_any_element()
                    })))
    }
}

fn review_placeholder(theme: &Theme) -> impl IntoElement {
    let field = |label: &'static str, detail: &'static str| {
        v_flex().flex_1().min_w_0().p_3().gap_2().rounded_xl().bg(theme.secondary.opacity(0.45))
            .child(div().text_xs().text_color(theme.muted_foreground).child(label))
            .child(div().text_lg().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child("__"))
            .child(div().text_xs().text_color(theme.muted_foreground).child(detail))
    };
    v_flex().gap_3()
        .child(h_flex().gap_2().child(field("Time complexity", "Dominant operations"))
            .child(field("Space complexity", "Auxiliary memory")))
        .children(["Correctness", "Improvements", "Dry run"].into_iter().map(|label| {
            h_flex().gap_3().px_3().py_2().rounded_lg().bg(theme.secondary.opacity(0.25))
                .child(div().flex_1().text_xs().text_color(theme.muted_foreground).child(label))
                .child(div().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child("__"))
        }))
}
