//! The pinned landing view uses the shared, in-memory recent-problem list.
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::base::{Tab, Tabs};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::actions::{self, ShowHome};
use crate::workspace::{Center, EditorPane, Focus, Judge, Loaded, Session, Workspace};
use crate::statement::Statement;
use gpui_kit::component::input::{EditorState, InputEvent, TabSize};

pub struct ProblemTab {
    pub session: Session,
    editor: Entity<EditorState>,
    pane: Entity<EditorPane>,
    statement: Entity<Statement>,
    subscription: Option<Subscription>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum HomeList { #[default] Recent, Contests }

#[derive(Default)]
pub struct HomeState {
    pub list: HomeList,
    pub selected: usize,
    pub sidebar: bool,
    pub scroll: ScrollHandle,
    pub contest_scroll: ScrollHandle,
    pub tabs_scroll: ScrollHandle,
}

impl Workspace {
    pub fn park_tab(&mut self) {
        if let (Some(index), Some(session)) = (self.active_tab, self.session.take()) {
            self.tabs[index] = Some(ProblemTab {
                session, editor: self.editor.clone(), pane: self.editor_pane.clone(),
                statement: self.statement.clone(), subscription: self.editor_subscription.take(),
            });
        }
    }

    fn take_tab(&mut self, index: usize) {
        if let Some(tab) = self.tabs[index].take() {
            self.session = Some(tab.session);
            self.editor = tab.editor;
            self.editor_pane = tab.pane;
            self.statement = tab.statement;
            self.editor_subscription = tab.subscription;
            self.active_tab = Some(index);
        }
    }

    pub fn new_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor = cx.new(|cx| EditorState::new(window, cx).language(self.config.preferred_language.id())
            .line_number(true).indent_guides(true).folding(true).soft_wrap(false)
            .tab_size(TabSize { tab_size: 4, hard_tabs: false }));
        self.editor_pane = cx.new(|cx| EditorPane::new(self.editor.clone(), cx));
        self.statement = cx.new(|_| Statement::default());
        self.editor_subscription = Some(cx.subscribe_in(&self.editor, window,
            |this, editor, event: &InputEvent, window, cx| {
                if *editor == this.editor && matches!(event, InputEvent::Change) {
                    this.sync_language_document(&editor, cx);
                    this.schedule_save(window, cx);
                }
            }));
    }

    pub fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() { return; }
        self.save_now(cx);
        if self.active_tab != Some(index) {
            self.park_tab();
            self.take_tab(index);
        }
        if let Some(session) = &self.session {
            let slug = session.slug.clone();
            self.reveal_in_sidebar(&slug);
        }
        self.back_to_editor(window, cx);
        cx.on_next_frame(window, move |this, _, cx| {
            this.home.tabs_scroll.scroll_to_item(index);
            cx.notify();
        });
    }

    pub fn cycle_tab(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let current = if self.center == Center::Home { None } else { self.active_tab };
        if let Some(next) = next_problem_tab(current, self.tabs.len(), delta) {
            self.select_tab(next, window, cx);
        }
    }

    pub fn session_for_mut(&mut self, slug: &str) -> Option<&mut Session> {
        if self.session.as_ref().is_some_and(|s| s.slug == slug) { return self.session.as_mut(); }
        self.tabs.iter_mut().flatten().find(|tab| tab.session.slug == slug).map(|tab| &mut tab.session)
    }

    pub fn finish_loading(&mut self, slug: &str, loaded: anyhow::Result<Loaded>, window: &mut Window, cx: &mut Context<Self>) {
        let background = self.session.as_ref().is_none_or(|s| s.slug != slug);
        let previous = self.active_tab;
        let focus = self.focus_area;
        if background {
            let Some(index) = self.tabs.iter().position(|tab| tab.as_ref().is_some_and(|tab| tab.session.slug == slug)) else { return; };
            self.park_tab();
            self.take_tab(index);
            self.focus_area = Focus::Home;
        }
        match loaded {
            Ok(loaded) => self.apply_loaded(loaded, window, cx),
            Err(_) if self.session.as_ref().and_then(|s| s.question.as_ref()).is_some_and(|q| q.meta["statementSource"] == "competitive-companion") => {},
            Err(err) => self.statement.update(cx, |s, cx| {
                s.status = Some(err.to_string().into());
                cx.notify();
            }),
        }
        if background {
            self.park_tab();
            if let Some(index) = previous { self.take_tab(index); }
            else { self.active_tab = None; }
            self.focus_area = focus;
        }
        cx.notify();
    }

    pub fn show_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_now(cx);
        self.refresh_contests(window, cx);
        if let Some(id) = self.contests.selected { self.refresh_contest_problems(id, window, cx); }
        self.center = Center::Home;
        self.history_mode = false;
        self.settings.editing = None;
        self.home.selected = self.home.selected.min(self.home_item_count().saturating_sub(1));
        self.focus_nav(Focus::Home, window, cx);
        cx.on_next_frame(window, |this, window, cx| {
            if this.center == Center::Home && !this.omni.open && this.focus_area == Focus::Home {
                this.focus_nav(Focus::Home, window, cx);
                this.home_active_scroll().scroll_to_item(this.home_scroll_index());
                this.home.tabs_scroll.scroll_to_item(0);
                cx.notify();
            }
        });
    }

    pub fn home_move(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.home_item_count();
        if count > 0 {
            self.home.selected = (self.home.selected as isize + delta)
                .clamp(0, count as isize - 1) as usize;
            self.home_active_scroll().scroll_to_item(self.home_scroll_index());
            cx.notify();
        }
    }

    fn home_item_count(&self) -> usize {
        match self.home.list { HomeList::Recent => self.recent_slugs.len(), HomeList::Contests => self.contests.visible().len() }
    }

    fn home_active_scroll(&self) -> &ScrollHandle {
        match self.home.list { HomeList::Recent => &self.home.scroll, HomeList::Contests => &self.home.contest_scroll }
    }

    fn home_scroll_index(&self) -> usize { self.home.selected + usize::from(self.home.list == HomeList::Contests) }

    pub fn select_home_list(&mut self, list: HomeList, window: &mut Window, cx: &mut Context<Self>) {
        self.home.list = list;
        self.home.selected = 0;
        self.home_active_scroll().scroll_to_item(0);
        if list == HomeList::Contests { self.refresh_contests(window, cx); }
        self.focus_nav(Focus::Home, window, cx);
        cx.notify();
    }

    pub fn home_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.home.list {
            HomeList::Recent => if let Some(slug) = self.recent_slugs.get(self.home.selected).cloned() { self.open_problem(slug, window, cx); },
            HomeList::Contests => if let Some(contest) = self.contests.visible().get(self.home.selected) { self.open_contest(contest.id, window, cx); },
        }
    }

    pub fn home_type(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.center != Center::Home || self.focus_area != Focus::Home
            || self.omni.open || !self.nav_focus.is_focused(window) {
            return;
        }
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function { return; }
        let Some(text) = event.keystroke.key_char.as_ref().filter(|text| !text.is_empty() && !text.chars().any(char::is_control)) else { return; };
        cx.stop_propagation();
        self.omni_open(crate::omnibar::Scope::All, window, cx);
        self.omni.input.update(cx, |input, cx| input.set_value(text.clone(), window, cx));
    }

    pub fn close_problem(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else { return; };
        if session.running || matches!(session.judge, Some(Judge::Running { .. })) {
            self.toast(gpui_kit::component::notification::Notification::warning("Wait for the current run to finish"), window, cx);
            return;
        }
        self.save_now(cx);
        if let Some(session) = &self.session {
            if session.question.is_some() && std::fs::read_to_string(&session.path).ok().as_deref()
                != Some(self.editor.read(cx).value().as_ref()) {
                self.toast(gpui_kit::component::notification::Notification::error("Solution could not be saved; tab kept open"), window, cx);
                return;
            }
        }
        self.session = None;
        if let Some(index) = self.active_tab.take() {
            self.tabs.remove(index);
            if !self.tabs.is_empty() {
                self.take_tab(index.min(self.tabs.len() - 1));
                self.back_to_editor(window, cx);
                return;
            }
        }
        self.show_home(window, cx);
    }

    pub fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let selected = match self.center { Center::Home => 0, Center::Editor => self.active_tab.map_or(0, |i| i + 1), _ => usize::MAX };
        let theme = cx.theme().clone();
        let make_tab = |index| Tab::new(("workspace-tab", index)).selected(selected == index)
            .set_position(index + 1, self.tabs.len() + 1).h_10().px_2p5().gap_2().rounded(px(12.)).flex_shrink_0()
            .text_sm().text_color(theme.tab_foreground).cursor_pointer()
            .styles(|styles| styles.selected(|style| style.bg(theme.tab_active).text_color(rgb(0xffffff))))
            .hover(|tab| tab.bg(if selected == index { theme.tab_active } else { theme.secondary }));
        let home = make_tab(0).accessibility_label("Home").child(Icon::new(IconName::House).small())
            .tooltip(|window, cx| Tooltip::new("Home").action(&ShowHome, Some(actions::WORKSPACE)).build(window, cx))
            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                this.show_home(window, cx);
                if event.click_count() >= 3 { this.flash("1337 · Achievement unlocked: home sweet home", cx); }
            }));
        let mut tabs = Tabs::new("workspace-tabs").flex().gap_1().min_w_0().flex_1().overflow_x_scroll()
            .track_scroll(&self.home.tabs_scroll);
        for (index, tab) in self.tabs.iter().enumerate() {
            let session = tab.as_ref().map(|tab| &tab.session).or_else(|| {
                (self.active_tab == Some(index)).then_some(self.session.as_ref()).flatten()
            });
            let Some(session) = session else { continue; };
            let title = if session.frontend_id == 0 { session.title.clone() } else { format!("{}. {}", session.frontend_id, session.title) };
            let tooltip = title.clone();
            tabs = tabs.child(make_tab(index + 1).max_w(px(240.)).accessibility_label(title.clone())
                .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                .child(div().min_w_0().flex_1().line_height(relative(1.4)).py_1().truncate().child(title))
                .child(Button::new(("close-problem-tab", index)).ghost().xsmall().icon(IconName::X)
                    .accessibility_label("Close problem").tooltip("Close problem")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.select_tab(index, window, cx);
                        this.close_problem(window, cx);
                    })))
                .on_click(cx.listener(move |this, _, window, cx| this.select_tab(index, window, cx))));
        }
        h_flex().m_2().gap_2().min_w_0().child(home).child(tabs).child(Button::new("open-settings-icon").ghost().small().icon(IconName::Settings)
            .accessibility_label("Settings").tooltip_with_action("Settings", &actions::OpenSettings, Some(actions::WORKSPACE))
            .on_click(cx.listener(|this, _, window, cx| this.open_settings(None, window, cx))))
    }

    pub fn render_home(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let focused = self.focus_area == Focus::Home && self.nav_focus.is_focused(window);
        v_flex().size_full().min_h_0().items_center().px_6().py_10()
            .child(h_flex().w_full().max_w(px(1120.)).h_full().min_h_0().gap_8()
                .child(v_flex().flex_1().min_w_0().h_full().min_h_0().gap_4()
                .child(h_flex().flex_shrink_0().justify_between()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.muted_foreground).child("Recent"))
                    .child(h_flex().gap_1()
                        .child(Button::new("home-find").ghost().small().icon(IconName::Search)
                            .accessibility_label("Find problem").tooltip_with_action("Find problem", &actions::FindProblem, Some(actions::WORKSPACE))
                            .on_click(cx.listener(|this, _, window, cx| this.omni_open(crate::omnibar::Scope::Problems, window, cx))))
                        .when(!self.home.sidebar, |row| row.child(self.render_source_menu(cx)))
                        .when(!self.home.sidebar, |row| row.child(Button::new("home-roadmap").ghost().small().icon(IconName::Map)
                            .accessibility_label("Roadmap").tooltip_with_action("Roadmap", &actions::ToggleRoadmap, Some(actions::WORKSPACE))
                            .on_click(cx.listener(|this, _, window, cx| this.show_roadmap(window, cx)))))))
                .child(v_flex().id("home-recents").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.home.scroll).gap_1()
                    .when(self.recent_slugs.is_empty(), |list| list.child(div().py_4().text_sm().text_color(theme.muted_foreground).child("No recent problems")))
                    .children(self.recent_slugs.iter().enumerate().map(|(i, slug)| {
                        let item = self.item(slug);
                        let title = item.map(|p| p.title.clone())
                            .or_else(|| practice::roadmap::entry(slug).map(|p| p.title.clone())).unwrap_or_else(|| slug.clone());
                        let slug = slug.clone();
                        let row = h_flex().id(("home-recent", i)).flex_shrink_0().h_10().px_3().gap_3().rounded_md()
                            .hover(|style| style.bg(theme.list_hover))
                            .when(focused && self.home.list == HomeList::Recent && i == self.home.selected, |row| row.bg(theme.list_active))
                            .child(div().flex_1().min_w_0().truncate().child(title))
                            .when_some(item, |row, item| {
                                let (label, color) = crate::view::difficulty(item.level, cx);
                                row.child(div().text_xs().text_color(color).child(label))
                            });
                        row.on_click(cx.listener(move |this, _, window, cx| {
                            this.home.list = HomeList::Recent;
                            this.home.selected = i;
                            this.open_problem(slug.clone(), window, cx);
                        }))
                    }))))
                .child(v_flex().flex_1().min_w_0().max_w(px(360.)).h_full().min_h_0().child(self.render_contests(cx))))
    }
}

fn next_problem_tab(current: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 { return None; }
    Some(match current {
        Some(index) => (index as isize + delta).rem_euclid(count as isize) as usize,
        None if delta < 0 => count - 1,
        None => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::next_problem_tab;

    #[::core::prelude::v1::test]
    fn tab_navigation_wraps_only_among_problems() {
        assert_eq!(next_problem_tab(None, 0, 1), None);
        assert_eq!(next_problem_tab(None, 3, 1), Some(0));
        assert_eq!(next_problem_tab(None, 3, -1), Some(2));
        assert_eq!(next_problem_tab(Some(2), 3, 1), Some(0));
        assert_eq!(next_problem_tab(Some(0), 3, -1), Some(2));
        assert_eq!(next_problem_tab(Some(0), 1, 1), Some(0));
        assert_eq!(next_problem_tab(Some(0), 1, -1), Some(0));
    }
}
