//! Universal search (`ctrl+k`, `ctrl+p`, `ctrl+shift+p`): commands, settings,
//! themes, topics, lists, and every problem in one fuzzy list. `>` limits to commands and
//! settings, `#` to topics and lists.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::base::{Presence, Transition};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit::assets::IconName;
use practice::roadmap::{self, List, TOPICS};
use practice::search::{Index, Searcher};

use crate::actions::COMMANDS;
use crate::settings::Setting;
use crate::view::key;
use crate::workspace::{Center, Focus, Workspace};

const LIMIT: usize = 120;
const ROW_H: f32 = 38.;
const VISIBLE_ROWS: f32 = 11.;

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Roadmap,
    Command(usize),
    ContextShortcut(usize),
    Contest(practice::contests::ContestId),
    Setting(Setting),
    List(List),
    Topic(&'static str),
    Theme(SharedString),
    Font(SharedString),
    /// Stable slug across source changes.
    Problem(SharedString),
}

impl Target {
    fn bonus(&self) -> u32 {
        match self {
            Target::Problem(_) | Target::Theme(_) | Target::Font(_) => 0,
            _ => 24,
        }
    }
}

pub struct Item {
    pub target: Target,
    pub title: SharedString,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Problems,
    Themes,
    Fonts,
    Shortcuts,
}

pub struct Omnibar {
    pub open: bool,
    pub scope: Scope,
    pub input: Entity<InputState>,
    items: Rc<Vec<Item>>,
    index: Rc<Index>,
    problem_item: HashMap<String, usize>,
    pub stale: bool,
    shortcuts: Vec<crate::actions::ContextShortcut>,
    pub hits: Vec<usize>,
    pub selected: usize,
    pub scroll: UniformListScrollHandle,
    searcher: Searcher,
    pub return_focus: Focus,
    theme_before: Option<SharedString>,
}

impl Omnibar {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> (Self, Subscription) {
        let input = cx.new(|cx| InputState::new(window, cx));
        let subscription = cx.subscribe_in(&input, window, |ws, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Change) {
                ws.omni_query_changed(window, cx);
            }
        });
        let omnibar = Self {
            open: false,
            scope: Scope::All,
            input,
            items: Rc::default(),
            index: Rc::new(Index::new(Vec::<String>::new())),
            problem_item: HashMap::new(),
            stale: true,
            shortcuts: vec![],
            hits: vec![],
            selected: 0,
            scroll: UniformListScrollHandle::new(),
            searcher: Searcher::default(),
            return_focus: Focus::Editor,
            theme_before: None,
        };
        (omnibar, subscription)
    }
}

impl Workspace {
    /// Rebuilds the searchable items; runs only after the catalog changes.
    fn omni_build(&mut self, cx: &App) {
        let mut items = vec![Item { target: Target::Roadmap, title: "Open roadmap".into() }];
        let mut text = vec!["Open roadmap neetcode graph map".to_string()];
        for (i, c) in COMMANDS.iter().enumerate() {
            items.push(Item { target: Target::Command(i), title: c.label.into() });
            text.push(crate::actions::shortcut_search_text(c.label, c.effective_key(&self.config)));
            items.push(Item { target: Target::Setting(Setting::Keybinding(i)), title: c.label.into() });
            text.push(crate::actions::shortcut_search_text(c.label, c.effective_key(&self.config)));
        }
        let bindings: Vec<_> = cx.key_bindings().borrow().bindings().cloned().collect();
        self.omni.shortcuts = crate::actions::contextual_shortcuts(&bindings, &self.config);
        for (index, shortcut) in self.omni.shortcuts.iter().enumerate() {
            items.push(Item { target: Target::ContextShortcut(index), title: shortcut.label.clone().into() });
            text.push(format!("{} {}", crate::actions::shortcut_search_text(&shortcut.label, &shortcut.keys), shortcut.context));
        }
        for s in Setting::ALL {
            items.push(Item { target: Target::Setting(s), title: format!("Settings: {}", s.label()).into() });
            text.push(format!("Settings {} {} preferences", s.label(), s.keywords()));
        }
        for list in [List::NeetCode150, List::NeetCode250, List::All] {
            items.push(Item { target: Target::List(list), title: format!("Show {}", list.label()).into() });
            text.push(format!("{} list", list.label()));
        }
        for t in TOPICS {
            items.push(Item { target: Target::Topic(t.name), title: t.name.into() });
            text.push(format!("{} topic", t.name));
        }
        for name in crate::theme::names(cx) {
            text.push(format!("Theme {name}"));
            items.push(Item { target: Target::Theme(name.clone()), title: name });
        }
        for name in crate::theme::fonts(cx) {
            text.push(format!("Font fonts family {name}"));
            items.push(Item { target: Target::Font(name.clone().into()), title: name.into() });
        }
        for contest in &self.contests.list {
            items.push(Item { target: Target::Contest(contest.id()), title: contest.name().to_owned().into() });
            text.push(format!("{} contest {} {}", contest.id().source().label(), contest.id().key(), contest.name()));
        }
        let mut problem_item = HashMap::new();
        for p in self.catalog.iter().chain(self.sources.catalog.iter()).chain(self.sources.codechef.iter()) {
            problem_item.insert(p.slug.clone(), items.len());
            let topic = roadmap::entry(&p.slug).map_or("", |e| e.topic.as_str());
            let provider = if p.slug.starts_with("cc:") { "CodeChef" } else if p.slug.starts_with("cf:") { "Codeforces" } else if roadmap::entry(&p.slug).is_some() { "NeetCode LeetCode" } else { "LeetCode" };
            text.push(format!("{} {} {} {topic} {provider}", p.frontend_id, p.title, p.slug));
            items.push(Item { target: Target::Problem(p.slug.clone().into()), title: format!("{}. {}", practice::codechef::problem_code(&p.slug).map(str::to_owned).unwrap_or_else(|_| p.frontend_id.to_string()), p.title).into() });
        }
        self.omni.index = Rc::new(Index::new(text));
        self.omni.items = Rc::new(items);
        self.omni.problem_item = problem_item;
        self.omni.stale = false;
    }

    pub fn omni_open(&mut self, scope: Scope, window: &mut Window, cx: &mut Context<Self>) {
        if self.omni.stale {
            self.omni_build(cx);
        }
        if !self.omni.open {
            self.omni.return_focus = self.focus_area;
        }
        self.omni.open = true;
        self.omni.scope = scope;
        self.omni.theme_before = (scope == Scope::Themes).then(|| cx.theme().theme_name().clone());
        let placeholder = match scope {
            Scope::All => "Search problems, commands, settings, themes…   > commands   # topics",
            Scope::Problems => "Go to problem by name, number or topic",
            Scope::Themes => "Theme · arrows preview, enter keeps",
            Scope::Fonts => "Search installed fonts…",
            Scope::Shortcuts => "Search shortcuts by action or key…",
        };
        self.omni.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(placeholder, window, cx);
            input.focus(window, cx);
        });
        self.focus_area = Focus::Omnibar;
        self.omni_query_changed(window, cx);
        if scope == Scope::Themes {
            let current = cx.theme().theme_name().clone();
            if let Some(pos) = self.omni.hits.iter().position(|&i| self.omni.items[i].title == current) {
                self.omni_select(pos, cx);
            }
        }
    }

    pub fn omni_close(&mut self, restore_theme: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.omni.open {
            return;
        }
        self.omni.open = false;
        if let Some(before) = self.omni.theme_before.take().filter(|_| restore_theme) {
            crate::theme::apply(&before, cx);
        }
        match self.omni.return_focus {
            Focus::Editor | Focus::Omnibar => self.focus_editor(window, cx),
            area => self.focus_nav(area, window, cx),
        }
        cx.notify();
    }

    fn omni_query_changed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.omni.input.read(cx).value().to_string();
        let (query, only): (&str, Option<fn(&Target) -> bool>) = match (self.omni.scope, raw.chars().next()) {
            (Scope::All, Some('>')) => (&raw[1..], Some(|t| matches!(t, Target::Command(_) | Target::Setting(_) | Target::ContextShortcut(_)))),
            (Scope::All, Some('#')) => (&raw[1..], Some(|t| matches!(t, Target::Topic(_) | Target::List(_) | Target::Roadmap))),
            (Scope::All, _) => (&raw, None),
            (Scope::Problems, _) => (&raw, Some(|t| matches!(t, Target::Problem(_)))),
            (Scope::Themes, _) => (&raw, Some(|t| matches!(t, Target::Theme(_)))),
            (Scope::Fonts, _) => (&raw, Some(|t| matches!(t, Target::Font(_)))),
            (Scope::Shortcuts, _) => (&raw, Some(|t| matches!(t, Target::Setting(Setting::Keybinding(_)) | Target::ContextShortcut(_)))),
        };
        let items = self.omni.items.clone();
        let keep = |i: usize| only.is_none_or(|f| f(&items[i].target));
        let limit = if matches!(self.omni.scope, Scope::Fonts | Scope::Shortcuts) { items.len() } else { LIMIT };
        self.omni.hits = if query.trim().is_empty() && matches!(self.omni.scope, Scope::Fonts | Scope::Shortcuts) {
            (0..items.len()).filter(|&i| keep(i)).collect()
        } else if query.trim().is_empty() && self.omni.scope != Scope::Themes {
            self.omni_defaults(&keep)
        } else {
            let mut hits = self.omni.searcher.rank(&self.omni.index, query, limit * 2, keep);
            for h in &mut hits {
                h.score += items[h.index].target.bonus();
            }
            hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));
            hits.into_iter().take(limit).map(|h| h.index).collect()
        };
        self.omni_select(0, cx);
    }

    /// Before typing: recent problems, then the roadmap's next unsolved ones, then commands.
    fn omni_defaults(&self, keep: &impl Fn(usize) -> bool) -> Vec<usize> {
        let recent = self.recent();
        let next_unsolved = self
            .library
            .ordered()
            .map(|e| roadmap::ENTRIES[e].slug.as_str())
            .filter(|slug| !self.solved.contains(*slug));
        let mut hits: Vec<usize> = recent
            .iter()
            .map(String::as_str)
            .chain(next_unsolved)
            .filter_map(|slug| self.omni.problem_item.get(slug).copied())
            .filter(|&i| keep(i))
            .take(if self.omni.scope == Scope::Problems { LIMIT } else { 12 })
            .collect();
        hits.dedup();
        let others = (0..self.omni.items.len()).filter(|&i| !matches!(self.omni.items[i].target, Target::Problem(_) | Target::Theme(_) | Target::Font(_)) && keep(i));
        hits.extend(others);
        hits.truncate(LIMIT);
        hits
    }

    pub fn omni_select(&mut self, pos: usize, cx: &mut Context<Self>) {
        if self.omni.hits.is_empty() {
            self.omni.selected = 0;
            cx.notify();
            return;
        }
        self.omni.selected = pos.min(self.omni.hits.len() - 1);
        self.omni.scroll.scroll_to_item(self.omni.selected, ScrollStrategy::Nearest);
        if self.omni.scope == Scope::Themes {
            if let Target::Theme(name) = &self.omni.items[self.omni.hits[self.omni.selected]].target {
                crate::theme::apply(name, cx);
            }
        }
        cx.notify();
    }

    pub fn omni_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = self.omni.hits.len() as isize;
        if len > 0 {
            let pos = (self.omni.selected as isize + delta).rem_euclid(len) as usize;
            self.omni_select(pos, cx);
        }
    }

    pub fn omni_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&i) = self.omni.hits.get(self.omni.selected) else { return };
        let target = self.omni.items[i].target.clone();
        if let Target::ContextShortcut(index) = target {
            self.flash(format!("Available in {}", self.omni.shortcuts[index].context), cx);
            return;
        }
        // Keep a previewed theme; every other choice restores the original look.
        let keep_theme = matches!(target, Target::Theme(_));
        if keep_theme {
            self.omni.theme_before = None;
        }
        self.omni_close(!keep_theme, window, cx);
        match target {
            Target::ContextShortcut(_) => {},
            Target::Contest(id) => self.open_contest(id, window, cx),
            Target::Roadmap => self.show_roadmap(window, cx),
            Target::Command(c) => window.dispatch_action((COMMANDS[c].action)(), cx),
            Target::Setting(s) => self.open_settings(Some(s), window, cx),
            Target::List(list) => {
                self.config.roadmap_list = list;
                self.save_config(window, cx);
                self.rebuild_library();
                self.flash(list.label(), cx);
            }
            Target::Topic(topic) => self.reveal_topic(topic, window, cx),
            Target::Theme(name) => {
                crate::theme::apply(&name, cx);
                self.config.theme = name.to_string();
                self.save_config(window, cx);
                self.flash(format!("Theme: {name}"), cx);
            }
            Target::Font(name) => {
                crate::theme::set_font(name.clone(), cx);
                self.config.font_family = name.to_string();
                self.save_config(window, cx);
                self.flash(format!("Font: {name}"), cx);
            }
            Target::Problem(slug) => {
                let slug = slug.to_string();
                self.center = Center::Editor;
                self.open_problem(slug, window, cx);
            }
        }
    }

    pub fn render_omnibar(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let presence = Presence::new("omnibar", self.omni.open)
            .transition(Transition::new(Duration::from_millis(140)).ease(ease_out_quint()))
            .sample(window, cx);
        if !presence.should_render() {
            return None;
        }
        let p = presence.progress.clamp(0., 1.);
        let theme = cx.theme().clone();
        let count = self.omni.hits.len();
        let list_h = (count.max(1) as f32).min(VISIBLE_ROWS) * ROW_H;
        let list = if count == 0 {
            div()
                .h(px(ROW_H * 2.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child("No matches")
                .into_any_element()
        } else {
            uniform_list("omni-results", count, cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                range.map(|pos| this.render_omni_row(pos, cx)).collect()
            }))
            .track_scroll(&self.omni.scroll)
            .w_full()
            .h(px(list_h))
            .into_any_element()
        };
        Some(
            div()
                .absolute()
                .inset_0()
                .bg(gpui_kit::black().opacity(0.28 * p))
                .flex()
                .justify_center()
                .items_start()
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.omni_close(true, window, cx)))
                .child(
                    v_flex()
                        .key_context("Omnibar")
                        .mt(px(84. - 10. * (1. - p)))
                        .w(px(700.))
                        .h_auto()
                        .opacity(p)
                        .bg(theme.popover)
                        .border_1()
                        .border_color(theme.border)
                        .rounded_xl()
                        .shadow_2xl()
                        .overflow_hidden()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_action(cx.listener(|this, _: &OmniUp, _, cx| this.omni_step(-1, cx)))
                        .on_action(cx.listener(|this, _: &OmniDown, _, cx| this.omni_step(1, cx)))
                        .on_action(cx.listener(|this, _: &OmniPageUp, _, cx| this.omni_step(-8, cx)))
                        .on_action(cx.listener(|this, _: &OmniPageDown, _, cx| this.omni_step(8, cx)))
                        .on_action(cx.listener(|this, _: &OmniConfirm, window, cx| this.omni_confirm(window, cx)))
                        .on_action(cx.listener(|this, _: &OmniClose, window, cx| this.omni_close(true, window, cx)))
                        .when(self.omni.scope == Scope::Shortcuts, |view| view.child(h_flex().px_4().pt_3().gap_2()
                            .child(Icon::new(IconName::Keyboard).small().text_color(theme.primary))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Keyboard shortcuts"))))
                        .child(
                            h_flex()
                                .px_4()
                                .h_12()
                                .gap_3()
                                .border_b_1()
                                .border_color(theme.border)
                                .child(Icon::new(IconName::Search).text_color(theme.muted_foreground))
                                .child(div().flex_1().child(Input::new(&self.omni.input).appearance(false).bordered(false))),
                        )
                        .child(div().p_1p5().child(list))
                        .child(
                            h_flex()
                                .px_4()
                                .h_8()
                                .gap_4()
                                .border_t_1()
                                .border_color(theme.border)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(h_flex().gap_1().child(key("up")).child(key("down")).child("move"))
                                .child(h_flex().gap_1().child(key("enter")).child("open"))
                                .child(h_flex().gap_1().child(key("escape")).child("close"))
                                .child(div().flex_1())
                                .child(format!("{count} results")),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_omni_row(&self, pos: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let item = &self.omni.items[self.omni.hits[pos]];
        let selected = pos == self.omni.selected;
        let muted = theme.muted_foreground;
        let (icon, kind, detail): (IconName, &str, AnyElement) = match &item.target {
            Target::Contest(id) => (IconName::CalendarRange, "Contest", div().child(id.source().label()).into_any_element()),
            Target::Roadmap => (IconName::Map, "View", key(crate::actions::key_for("ToggleRoadmap", &self.config)).into_any_element()),
            Target::Command(c) => (IconName::SquareTerminal, "Command", crate::view::shortcut_keys(COMMANDS[*c].effective_key(&self.config)).into_any_element()),
            Target::ContextShortcut(index) => {
                let shortcut = &self.omni.shortcuts[*index];
                (IconName::Keyboard, "Shortcut", h_flex().gap_2()
                    .child(div().max_w(px(140.)).truncate().child(shortcut.context.clone()))
                    .child(crate::view::shortcut_keys(&shortcut.keys)).into_any_element())
            }
            Target::Setting(Setting::Keybinding(index)) => (IconName::Keyboard, "Shortcut", crate::view::shortcut_keys(COMMANDS[*index].effective_key(&self.config)).into_any_element()),
            Target::Setting(s) => (IconName::Settings, "Setting", div().max_w(px(220.)).truncate().child(s.value(self, cx)).into_any_element()),
            Target::List(l) => (
                IconName::BookOpen,
                "List",
                div().child(if *l == self.config.roadmap_list { "current" } else { "" }).into_any_element(),
            ),
            Target::Topic(t) => {
                let (done, total) = self.library.progress(t);
                (IconName::Map, "Topic", div().child(format!("{done}/{total}")).into_any_element())
            }
            Target::Theme(name) => (
                IconName::Palette,
                "Theme",
                div().child(if self.omni.theme_before.as_ref().unwrap_or(cx.theme().theme_name()) == name { "current" } else { "" }).into_any_element(),
            ),
            Target::Font(name) => (
                IconName::Settings,
                "Font",
                div().child(if name.as_ref() == self.config.font_family { "current" } else { "" }).into_any_element(),
            ),
            Target::Problem(slug) => {
                let Some(p) = self.item(slug) else { return div().into_any_element(); };
                let (label, color) = crate::view::difficulty(p.level, cx);
                let solved = self.solved.contains(&p.slug);
                let topic = roadmap::entry(&p.slug).map(|e| e.topic.clone());
                (
                    IconName::BookOpen,
                    if p.slug.starts_with("cc:") { "CodeChef" } else if p.slug.starts_with("cf:") { "Codeforces" } else if roadmap::entry(&p.slug).is_some() { "NeetCode" } else { "LeetCode" },
                    h_flex()
                        .gap_2()
                        .when_some(topic, |el, t| el.child(t))
                        .when(solved, |el| el.child(div().text_color(theme.success).child("✓")))
                        .child(div().w_3().font_weight(FontWeight::BOLD).text_color(color).child(label))
                        .into_any_element(),
                )
            }
        };
        h_flex()
            .id(pos)
            .w_full()
            .h(px(ROW_H))
            .px_3()
            .gap_3()
            .rounded_md()
            .when(selected, |el| el.bg(theme.list_active))
            .child(match &item.target {
                Target::Problem(slug) => crate::brand::source_icon(if slug.starts_with("cc:") { practice::language::Source::CodeChef } else if slug.starts_with("cf:") { practice::language::Source::Codeforces } else if roadmap::entry(slug).is_some() { practice::language::Source::NeetCode } else { practice::language::Source::LeetCode }).small(),
                _ => Icon::new(icon).small().text_color(if selected { theme.primary } else { muted }),
            })
            .child(div().flex_1().truncate().child(item.title.clone()))
            .child(h_flex().gap_3().text_xs().text_color(muted).child(detail).child(div().w(px(64.)).text_right().whitespace_nowrap().child(kind)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.omni.selected = pos;
                this.omni_confirm(window, cx);
            }))
            .into_any_element()
    }
}

gpui_kit::actions!(omnibar, [OmniUp, OmniDown, OmniPageUp, OmniPageDown, OmniConfirm, OmniClose]);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Omnibar > Input");
    cx.bind_keys([
        KeyBinding::new("up", OmniUp, ctx),
        KeyBinding::new("down", OmniDown, ctx),
        KeyBinding::new("pageup", OmniPageUp, ctx),
        KeyBinding::new("pagedown", OmniPageDown, ctx),
        KeyBinding::new("enter", OmniConfirm, ctx),
        KeyBinding::new("escape", OmniClose, ctx),
    ]);
}
