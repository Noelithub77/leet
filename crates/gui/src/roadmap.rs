//! Topic details beside the roadmap, backed by the existing cached library.
use std::collections::HashSet;
use std::time::Duration;

use gpui_kit::base::{Transition, transition};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::ProgressCircle;
use gpui_kit::component::{ActiveTheme as _, Icon, Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit::assets::IconName;
use practice::db::Db;
use practice::roadmap::{ENTRIES, TOPICS};
use crate::workspace::{Focus, Workspace};

#[derive(Default)]
pub struct RoadmapState {
    pub details: bool,
    pub selected: usize,
    pub scroll: UniformListScrollHandle,
    stars: HashSet<String>,
}

impl RoadmapState {
    pub fn load(db: &Db) -> Self {
        let stars = db.get("starred-problems").ok().flatten()
            .map(|value| value.lines().map(str::to_owned).collect()).unwrap_or_default();
        Self { stars, ..Self::default() }
    }
}

pub fn panel_width(window: &Window, open: bool) -> f32 {
    if open { (f32::from(window.viewport_size().width) * 0.47).clamp(340., 720.) } else { 0. }
}

pub fn problem_url(source: practice::language::Source, slug: &str) -> String {
    if let Ok(url) = practice::codeforces::problem_url(slug) { return url; }
    if source == practice::language::Source::LeetCode { return format!("https://leetcode.com/problems/{slug}/"); }
    practice::roadmap::entry(slug).map_or_else(
        || format!("https://leetcode.com/problems/{slug}/"),
        |entry| entry.neetcode_url(),
    )
}

fn next_row(current: usize, count: usize, delta: isize) -> usize {
    if count == 0 { 0 } else { (current.min(count - 1) as isize + delta).rem_euclid(count as isize) as usize }
}

impl Workspace {
    pub fn roadmap_close_topic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.roadmap.details = false;
        self.focus_nav(Focus::Roadmap, window, cx);
    }

    pub fn roadmap_move_problem(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.library.topic(TOPICS[self.roadmap_sel].name).map_or(0, |topic| topic.entries.len());
        self.roadmap.selected = next_row(self.roadmap.selected, count, delta);
        self.roadmap.scroll.scroll_to_item(self.roadmap.selected, ScrollStrategy::Top);
        cx.notify();
    }

    pub fn roadmap_open_problem(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entry = self.library.topic(TOPICS[self.roadmap_sel].name)
            .and_then(|topic| topic.entries.get(self.roadmap.selected));
        if let Some(&entry) = entry { self.open_problem(ENTRIES[entry].slug.clone(), window, cx); }
    }

    fn roadmap_star(&mut self, entry: usize, window: &mut Window, cx: &mut Context<Self>) {
        let slug = &ENTRIES[entry].slug;
        let mut stars = self.roadmap.stars.clone();
        if !stars.remove(slug) { stars.insert(slug.clone()); }
        let mut sorted: Vec<_> = stars.iter().map(String::as_str).collect();
        sorted.sort_unstable();
        match self.db.set("starred-problems", &sorted.join("\n")) {
            Ok(()) => { self.roadmap.stars = stars; cx.notify(); }
            Err(error) => self.toast(Notification::error(error.to_string()), window, cx),
        }
    }

    pub(crate) fn roadmap_link(&mut self, url: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = open::that_detached(url) { self.toast(Notification::error(error.to_string()), window, cx); }
    }

    pub(crate) fn render_roadmap(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reveal = transition("roadmap-topic-reveal", if self.roadmap.details { 1. } else { 0. },
            Transition::new(Duration::from_millis(280)), window, cx).clamp(0., 1.);
        let width = panel_width(window, true);
        let occupied = (width + 8.) * reveal;
        h_flex().size_full().min_h_0()
            .child(div().flex_1().min_w_0().h_full().overflow_hidden()
                .child(self.render_roadmap_graph(occupied, window, cx)))
            .when(reveal > 0., |view| view.child(div().w(px(occupied)).h_full().min_h_0()
                .flex_shrink_0().overflow_hidden().pl(px(8. * reveal))
                // Keep text and rows at their final width while the shell reveals them.
                .child(self.render_roadmap_topic(window, cx).opacity(reveal))))
    }

    fn render_roadmap_topic(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let topic = &TOPICS[self.roadmap_sel];
        let (done, total) = self.library.progress(topic.name);
        let progress = if total == 0 { 0. } else { done as f32 / total as f32 * 100. };
        let prerequisites: Vec<_> = TOPICS.iter().enumerate().filter(|(_, parent)| parent.children.contains(&topic.name)).collect();
        v_flex().w(px(panel_width(window, true))).h_full().min_h_0().flex_shrink_0()
            .p_4().gap_4().rounded_lg().bg(theme.sidebar).border_1().border_color(theme.border)
            .child(h_flex().w_full().items_start().justify_between()
                .child(v_flex().flex_1().min_w_0().gap_2()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(topic.name))
                    .child(h_flex().gap_2().items_center()
                        .child(ProgressCircle::new("topic-progress").small().value(progress).color(theme.success)
                            .accessibility_label(format!("{done} of {total} solved")))
                        .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{done} / {total} solved")))))
                .child(Button::new("close-topic").ghost().small().icon(IconName::X).accessibility_label("Close topic")
                    .tooltip("Close topic · Escape").on_click(cx.listener(|this, _, window, cx| this.roadmap_close_topic(window, cx)))))
            .when(!prerequisites.is_empty(), |panel| panel.child(v_flex().gap_2()
                .child(div().text_xs().text_color(theme.muted_foreground).child("Prerequisites"))
                .child(h_flex().flex_wrap().gap_2().children(prerequisites.into_iter().map(|(index, parent)| {
                    let (done, total) = self.library.progress(parent.name);
                    Button::new(("prerequisite", index)).outline().small()
                        .label(parent.name).tooltip(format!("{done} / {total} solved"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.roadmap_sel = index;
                            this.roadmap_open_topic(window, cx);
                        }))
                })))))
            .child(h_flex().px_2().gap_2().text_xs().text_color(theme.muted_foreground)
                .child(div().w_5().child("✓"))
                .child(div().w_7().child("★"))
                .child(div().flex_1().child("Problem"))
                .child(div().w_6())
                .child(div().w(px(62.)).child("Difficulty"))
                .child(div().w_7().child(Icon::new(IconName::Play).xsmall())))
            .child(div().flex_1().min_h_0().rounded_lg().overflow_hidden().border_1().border_color(theme.border)
                .when(total == 0, |view| view.child(div().p_4().text_color(theme.muted_foreground).child("No problems in this list")))
                .when(total > 0, |view| view.child(uniform_list("topic-problems", total,
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range.map(|index| this.render_roadmap_problem(index, cx)).collect()
                    })).track_scroll(&self.roadmap.scroll).size_full())))
            .child(h_flex().gap_2().text_xs().text_color(theme.muted_foreground)
                .child("↑ ↓ select · Enter open · ← graph · Escape close"))
    }

    fn render_roadmap_problem(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(&entry_index) = self.library.topic(TOPICS[self.roadmap_sel].name).and_then(|topic| topic.entries.get(index)) else { return div().into_any_element(); };
        let entry = &ENTRIES[entry_index];
        let theme = cx.theme().clone();
        let solved = self.solved.contains(&entry.slug) || self.neetcode_solved.contains(&entry.slug);
        let starred = self.roadmap.stars.contains(&entry.slug);
        let (difficulty, color) = match entry.difficulty.as_str() { "easy" => ("Easy", theme.success), "hard" => ("Hard", theme.danger), _ => ("Medium", theme.warning) };
        let selected = self.focus_area == Focus::RoadmapTopic && self.roadmap.selected == index;
        let slug = entry.slug.clone();
        h_flex().id(("topic-row", entry_index)).w_full().h_11().px_2().gap_2().items_center().rounded_md()
            .bg(if selected { theme.list_active } else if solved { theme.success.opacity(0.06) } else { theme.background })
            .hover(|style| style.bg(theme.list_hover)).cursor_pointer()
            .child(div().w_5().text_color(if solved { theme.success } else { theme.muted_foreground }).child(if solved { "✓" } else { "·" }))
            .child(Button::new(("star-problem", entry_index)).ghost().xsmall().icon(IconName::Star).selected(starred)
                .when(starred, |button| button.text_color(theme.warning))
                .accessibility_label(if starred { "Unstar problem" } else { "Star problem" })
                .tooltip(if starred { "Unstar problem" } else { "Star problem" })
                .on_click(cx.listener(move |this, _, window, cx| { cx.stop_propagation(); this.roadmap_star(entry_index, window, cx); })))
            .child(div().flex_1().min_w_0().truncate().child(entry.title.clone()))
            .child(Button::new(("source-link", entry_index)).ghost().xsmall().icon(IconName::ExternalLink)
                .accessibility_label("Open on NeetCode").tooltip("Open on NeetCode")
                .on_click(cx.listener(move |this, _, window, cx| { cx.stop_propagation(); this.roadmap_link(ENTRIES[entry_index].neetcode_url(), window, cx); })))
            .child(div().w(px(62.)).text_xs().text_color(color).child(difficulty))
            .child(Button::new(("solution-link", entry_index)).ghost().xsmall().icon(IconName::Play)
                .disabled(entry.video.is_empty()).accessibility_label("Watch solution").tooltip("Watch solution")
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.roadmap_link(format!("https://www.youtube.com/watch?v={}", ENTRIES[entry_index].video), window, cx);
                })))
            .on_click(cx.listener(move |this, _, window, cx| { this.roadmap.selected = index; this.open_problem(slug.clone(), window, cx); }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn problem_selection_handles_empty_changed_and_wrapping_lists() {
        assert_eq!(next_row(0, 0, 1), 0);
        assert_eq!(next_row(0, 3, -1), 2);
        assert_eq!(next_row(2, 3, 1), 0);
        assert_eq!(next_row(20, 2, 1), 0);
    }
    #[::core::prelude::v1::test]
    fn browser_links_use_the_problem_source() {
        assert!(problem_url(practice::language::Source::NeetCode, "two-sum").starts_with("https://neetcode.io/problems/"));
        assert_eq!(problem_url(practice::language::Source::Codeforces, "cf:1:A"), "https://codeforces.com/problemset/problem/1/A");
        assert_eq!(problem_url(practice::language::Source::LeetCode, "a-leetcode-only-problem"), "https://leetcode.com/problems/a-leetcode-only-problem/");
    }
}
