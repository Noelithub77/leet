//! Rendering. Panels spring open and closed; results reveal with a short stagger.

use std::time::Duration;

use gpui_kit::base::{Disableable as _, Spring, Transition, spring, transition};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use practice::roadmap::{self, ENTRIES, TOPICS};
use practice::runner::Verdict;

use crate::workspace::{Center, Focus, Judge, Row, Workspace};

const LEFT_W: f32 = 300.;
const RIGHT_W: f32 = 440.;
const BOTTOM_H: f32 = 230.;

fn panel_spring() -> Spring {
    Spring::new(Duration::from_millis(320))
}

/// A shortcut as key caps; for `a|b` alternatives only the first is shown.
pub fn key(k: &str) -> impl IntoElement + use<> {
    let first = k.split('|').next().unwrap_or_default();
    h_flex().gap_0p5().children(first.split_whitespace().filter_map(|k| Keystroke::parse(k).ok()).map(Kbd::new))
}

pub fn difficulty(level: u8, cx: &App) -> (&'static str, Hsla) {
    match level {
        1 => ("E", cx.theme().success),
        3 => ("H", cx.theme().danger),
        _ => ("M", cx.theme().warning),
    }
}

/// Fades rows in one after another; `delay_steps` staggers by 18ms each.
fn fade_in(delay_steps: usize) -> Animation {
    let delay = 18. * delay_steps.min(14) as f32;
    let fade = 160.;
    let total = delay + fade;
    Animation::new(Duration::from_millis(total as u64)).with_easing(move |t| {
        let t = ((t * total - delay) / fade).clamp(0., 1.);
        1. - (1. - t).powi(3)
    })
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        // Roadmap and settings take the whole window; panels slide away for them.
        let panels = !self.zen && self.center == Center::Editor;
        let z = self.config.zoom;
        let show_left = !self.zen && ((self.center == Center::Editor && self.left) || (self.center == Center::Home && self.home.sidebar));
        let left_w = spring("left-w", if show_left { LEFT_W * z } else { 0. }, panel_spring(), window, cx);
        let right_w = spring("right-w", if panels && self.right { RIGHT_W * z } else { 0. }, panel_spring(), window, cx);
        let bottom_h = spring("bottom-h", if panels && self.bottom { BOTTOM_H * z } else { 0. }, panel_spring(), window, cx);
        let tabs = self.render_tabs(cx);
        let center = match self.center {
            Center::Onboarding => div().flex_1().h_full().children(self.onboarding.clone()).into_any_element(),
            Center::Home => self.render_home(window, cx).into_any_element(),
            Center::Editor => v_flex()
                .flex_1()
                .min_h_0()
                .child(self.render_header(cx))
                .child(div().flex_1().min_h_0().rounded_lg().overflow_hidden().child(self.editor_pane.clone().cached(StyleRefinement::default().size_full())))
                .into_any_element(),
            Center::Roadmap => self.render_roadmap(window, cx).into_any_element(),
            Center::Settings => div().key_context("Settings").flex_1().min_h_0().h_full().overflow_hidden().child(self.render_settings(window, cx)).into_any_element(),
        };
        let omnibar = self.render_omnibar(window, cx);

        let root = v_flex()
            .relative()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_sm()
            .track_focus(&self.focus);
        Workspace::register(root, cx).on_key_down(cx.listener(|this, event, window, cx| this.home_type(event, window, cx))).child(tabs).child(
            h_flex().flex_1().min_h_0().mx_2().gap_2().track_focus(&self.nav_focus)
                .when(self.nav_focus.is_focused(window), |view| Workspace::register_nav(view, cx))
            .child(self.render_left(left_w, window, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .rounded_lg().overflow_hidden()
                    .child(center)
                    .child(self.render_bottom(bottom_h, cx)),
            )
            .child(self.render_right(right_w, cx)),
        )
        .child(self.render_status(window, cx))
        .children(omnibar)
    }
}

impl Workspace {
    fn render_left(&self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let focused = self.focus_area == Focus::Sidebar && self.nav_focus.is_focused(window);
        div()
            .h_full()
            .rounded_lg()
            .w(px(width.max(0.)))
            .flex_shrink_0()
            .overflow_hidden()
            .border_r_1()
            .border_color(theme.border.opacity((width / (LEFT_W * self.config.zoom)).clamp(0., 1.)))
            .bg(theme.sidebar)
            .child(
                v_flex()
                    .w(px(LEFT_W * self.config.zoom))
                    .h_full()
                    .opacity((width / (LEFT_W * self.config.zoom)).clamp(0., 1.))
                    .child(self.render_tree(focused, cx)),
            )
    }

    fn render_tree(&self, focused: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (solved, total) = if self.config.source == practice::language::Source::NeetCode {
            (self.library.solved, self.library.total)
        } else if self.config.source == practice::language::Source::Codeforces {
            (self.sources.solved.len(), self.sources.catalog.len())
        } else { (self.solved.len(), self.catalog.len()) };
        v_flex().size_full().gap_2()
            .child(h_flex().h_9().px_2().gap_2().items_center()
                .child(self.render_source_menu(cx))
                .child(div().flex_1().text_xs().text_color(theme.muted_foreground).child(format!("{solved}/{total}")))
                .child(gpui_kit::component::button::Button::new("explorer-roadmap").ghost().xsmall()
                    .icon(gpui_kit::assets::IconName::Map).accessibility_label("Roadmap")
                    .tooltip_with_action("Roadmap", &crate::actions::ToggleRoadmap, Some(crate::actions::WORKSPACE))
                    .on_click(cx.listener(|this, _, window, cx| this.show_roadmap(window, cx)))))
            .when(self.config.source == practice::language::Source::NeetCode, |view| view
                .child(self.render_topic_grid(cx))
                .child(h_flex().px_3().gap_2().child(div().flex_1().truncate().text_xs().font_weight(FontWeight::SEMIBOLD).child(TOPICS[self.explorer_topic].name))
                    .child(div().text_xs().text_color(theme.muted_foreground).child({ let (done, total) = self.library.progress(TOPICS[self.explorer_topic].name); format!("{done}/{total}") }))))
            .child(uniform_list("sidebar", self.rows.len(), cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range.map(|ix| this.render_row(ix, focused, cx)).collect()
            })).track_scroll(&self.sidebar_scroll).flex_1())
    }

    fn render_row(&self, ix: usize, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let selected = ix == self.sidebar_sel;
        let base = h_flex()
            .id(ix)
            .rounded_md()
            .w_full()
            .h_7()
            .px_3()
            .gap_2()
            .border_l_2()
            .border_color(gpui_kit::transparent_black())
            .when(selected, |el| {
                el.bg(if focused { theme.list_active } else { theme.list_hover })
                    .when(focused, |el| el.border_color(theme.list_active_border))
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.sidebar_sel = ix;
                this.focus_nav(Focus::Sidebar, window, cx);
                this.sidebar_activate(window, cx);
            }));
        match self.rows[ix] {
            Row::Catalog(index) => {
                let item = &self.active_catalog()[index];
                let solved = if item.slug.starts_with("cf:") { self.sources.solved.contains(&item.slug) } else { self.solved.contains(&item.slug) };
                let (_, color) = difficulty(item.level, cx);
                let rating = self.sources.ratings.get(&item.slug).map(u32::to_string);
                let label = rating.unwrap_or_else(|| difficulty(item.level, cx).0.into());
                let id = practice::codeforces::problem_id(&item.slug).map(|(contest, index)| format!("{contest}{index}")).unwrap_or_else(|_| item.frontend_id.to_string());
                base.child(div().w_3().text_color(if solved { theme.success } else { theme.muted_foreground }).child(if solved { "✓" } else { "·" }))
                    .child(div().flex_1().truncate().child(format!("{id}. {}", item.title)))
                    .child(div().text_xs().text_color(color).child(label)).into_any_element()
            }
            Row::Problem(e) => {
                let entry = &ENTRIES[e];
                let slug = entry.slug.as_str();
                let level = self.item(slug).map_or(2, |p| p.level);
                let (label, color) = difficulty(level, cx);
                let current = self.session.as_ref().is_some_and(|s| s.slug == slug);
                let solved = self.solved.contains(slug) || self.neetcode_solved.contains(slug);
                let (mark, mark_color) = match (solved, current) {
                    (true, _) => ("✓", theme.success),
                    (false, true) => ("●", theme.primary),
                    _ => ("·", theme.muted_foreground),
                };
                let row = base
                    .pl_6()
                    .child(div().w_3().text_color(mark_color).child(mark))
                    .child(div().flex_1().truncate().when(current, |el| el.text_color(theme.primary)).child(entry.title.clone()))
                    .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(color).child(label));
                row.into_any_element()
            }
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(s) = &self.session else {
            return h_flex().h_9().px_4().border_b_1().border_color(theme.border).text_color(theme.muted_foreground).child("No problem open").into_any_element();
        };
        let level = self.item(&s.slug).map_or(2, |p| p.level);
        let (label, color) = difficulty(level, cx);
        let solved = self.solved.contains(&s.slug);
        let judging = matches!(s.judge, Some(Judge::Running { .. }));
        h_flex()
            .h_9()
            .px_4()
            .gap_3()
            .border_b_1()
            .border_color(theme.border)
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(color).child(match label {
                "E" => "Easy",
                "H" => "Hard",
                _ => "Medium",
            }))
            .when(solved, |el| el.child(div().text_xs().text_color(theme.success).child("✓ Solved")))
            .child(div().flex_1())
            .when(judging || s.running, |el| {
                el.child(h_flex().gap_2().text_xs().text_color(theme.muted_foreground).child(Spinner::new().xsmall()).child(if judging { "LeetCode judging" } else { "Running" }))
            })
            .when(s.question.is_none(), |el| el.child(Spinner::new().xsmall()))
            .into_any_element()
    }

    fn render_right(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let content = if self.history_mode {
            self.render_versions(cx).into_any_element()
        } else {
            self.statement.clone().cached(StyleRefinement::default().size_full()).into_any_element()
        };
        div()
            .h_full()
            .rounded_lg()
            .w(px(width.max(0.)))
            .flex_shrink_0()
            .overflow_hidden()
            .border_l_1()
            .border_color(theme.border.opacity((width / (RIGHT_W * self.config.zoom)).clamp(0., 1.)))
            .bg(theme.sidebar)
            .child(v_flex().w(px(RIGHT_W * self.config.zoom)).h_full().opacity((width / (RIGHT_W * self.config.zoom)).clamp(0., 1.))
                .child(h_flex().p_2().gap_1()
                    .child(crate::theme::selected_choice(Button::new("right-question").ghost().small(), !self.history_mode, cx).icon(IconName::FileText).tooltip("Question and solution").accessibility_label("Question and solution")
                        .on_click(cx.listener(|this,_,_,cx|{this.history_mode=false;cx.notify();})))
                    .child(crate::theme::selected_choice(Button::new("right-history").ghost().small(), self.history_mode, cx).icon(IconName::GitBranch).tooltip_with_action("History", &crate::actions::ToggleHistory, Some(crate::actions::WORKSPACE)).accessibility_label("History")
                        .on_click(cx.listener(|this,_,window,cx|{this.history_mode=true;if this.session.as_ref().is_some_and(|session|!session.history_loaded){this.load_remote_versions(window,cx);}this.focus_nav(Focus::Sidebar,window,cx);cx.notify();}))))
                .child(div().flex_1().min_h_0().child(content)))
    }

    fn render_bottom(&self, height: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .w_full()
            .h(px(height.max(0.)))
            .flex_shrink_0()
            .overflow_hidden()
            .border_t_1()
            .border_color(theme.border.opacity((height / (BOTTOM_H * self.config.zoom)).clamp(0., 1.)))
            .child(
                div()
                    .h(px(BOTTOM_H * self.config.zoom))
                    .opacity((height / (BOTTOM_H * self.config.zoom)).clamp(0., 1.))
                    .child(self.render_results(cx)),
            )
    }

    fn render_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(s) = &self.session else { return div().into_any_element() };
        if let Some(err) = &s.compile_error {
            return v_flex()
                .p_3()
                .gap_2()
                .child(div().text_color(theme.danger).font_weight(FontWeight::SEMIBOLD).child("Syntax error"))
                .child(div().font_family(theme.mono_font_family.clone()).text_xs().child(err.clone()))
                .into_any_element();
        }
        let judge = self.render_judge(cx);
        let chips = h_flex().id("test-case-tabs").min_w_0().flex_1().overflow_x_scroll().gap_1p5().children(s.cases.iter().enumerate().map(|(i, case)| {
            let result = s.results.get(i).and_then(|r| r.as_ref());
            let (glyph, color) = match result.map(|r| r.verdict) {
                Some(Verdict::Pass) => ("✓", theme.success),
                Some(Verdict::Fail) => ("✗", theme.danger),
                Some(Verdict::Error) => ("!", theme.danger),
                Some(Verdict::Timeout) => ("⏱", theme.warning),
                Some(Verdict::Ran) => ("•", theme.info),
                None => ("", theme.muted_foreground),
            };
            let active = i == s.selected_case;
            let chip = h_flex()
                .id(("case", i))
                .flex_shrink_0()
                .h_7()
                .px_2p5()
                .gap_1p5()
                .rounded_md()
                .text_xs()
                .bg(if active { theme.secondary_active } else { theme.secondary })
                .when(active, |el| el.border_1().border_color(color.opacity(0.6)))
                .child(div().text_color(color).child(glyph))
                .child(if case.custom { format!("Custom {}", i + 1) } else { format!("Case {}", i + 1) })
                .when_some(result.filter(|r| r.ms >= 1.), |el, r| el.child(div().text_color(theme.muted_foreground).child(format!("{:.0}ms", r.ms))))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(s) = &mut this.session {
                        s.selected_case = i;
                        cx.notify();
                    }
                }));
            // Results pop in one after another when a run finishes.
            if result.is_some() {
                chip.with_animation(SharedString::from(format!("chip-{}-{i}", s.run_id)), fade_in(i * 2), |el, t| el.opacity(0.3 + 0.7 * t))
                    .into_any_element()
            } else {
                chip.opacity(if s.running { 0.5 } else { 1. }).into_any_element()
            }
        }));
        let detail = s.cases.get(s.selected_case).map(|case| {
            let result = s.results.get(s.selected_case).and_then(|r| r.as_ref());
            let field = |label: &'static str, value: String, color: Option<Hsla>| {
                v_flex()
                    .gap_1()
                    .min_w_0()
                    .flex_1()
                    .child(div().text_xs().text_color(theme.muted_foreground).child(label))
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme.muted)
                            .font_family(theme.mono_font_family.clone())
                            .text_xs()
                            .when_some(color, |el, c| el.text_color(c))
                            .child(if value.is_empty() { "—".to_string() } else { value }),
                    )
            };
            let failed = result.is_some_and(|r| r.verdict == Verdict::Fail);
            h_flex()
                .gap_3()
                .items_start()
                .child(field("Input", case.input.clone(), None))
                .child(field("Expected", case.expected.clone().unwrap_or_default(), None))
                .child(field(
                    "Output",
                    result.map(|r| if r.error.is_empty() { r.output.clone() } else { r.error.lines().last().unwrap_or("").to_string() }).unwrap_or_default(),
                    failed.then_some(theme.danger).or(result.filter(|r| r.verdict == Verdict::Pass).map(|_| theme.success)),
                ))
                .when_some(result.filter(|r| !r.stdout.is_empty()), |el, r| el.child(field("Stdout", r.stdout.clone(), None)))
        });
        v_flex()
            .id("results")
            .size_full()
            .p_3()
            .gap_3()
            .overflow_y_scroll()
            .child(
                h_flex()
                    .justify_between()
                    .child(h_flex().min_w_0().flex_1().gap_2().child(chips)
                        .child(Button::new("add-test-case").ghost().xsmall().icon(IconName::Plus)
                            .accessibility_label("Add test case").tooltip_with_action("Add test case", &crate::actions::AddCustomTest, Some(crate::actions::WORKSPACE))
                            .disabled(s.running || matches!(s.judge, Some(Judge::Running { .. })))
                            .on_click(cx.listener(|this, _, window, cx| crate::dialogs::open_custom_test(this, window, cx))))
                        .child(Button::new("edit-test-case").ghost().xsmall().icon(IconName::Pencil)
                            .accessibility_label("Edit selected test case").tooltip_with_action("Edit selected test case", &crate::actions::EditTestCase, Some(crate::actions::WORKSPACE))
                            .disabled(s.cases.is_empty() || s.running || matches!(s.judge, Some(Judge::Running { .. })))
                            .on_click(cx.listener(|this, _, window, cx| crate::dialogs::edit_test_case(this, window, cx))))
                        .child(Button::new("reset-test-cases").ghost().xsmall().icon(IconName::RefreshCw)
                            .accessibility_label("Restore question examples").tooltip("Restore question examples; remove edits and custom cases")
                            .disabled(s.running || matches!(s.judge, Some(Judge::Running { .. })))
                            .on_click(cx.listener(|this, _, _, cx| this.reset_test_cases(cx)))))
                    .child(h_flex().gap_3().text_xs().text_color(theme.muted_foreground).child(h_flex().gap_1().child(key(crate::actions::key_for("RunTests", &self.config))).child("run")).child(h_flex().gap_1().child(key(crate::actions::key_for("Submit", &self.config))).child("submit"))),
            )
            .children(judge)
            .children(detail)
            .into_any_element()
    }

    fn render_judge(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let judge = self.session.as_ref()?.judge.as_ref()?;
        let el = match judge {
            Judge::Running { submission } => h_flex()
                .gap_2()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(Spinner::new().xsmall())
                .child(if *submission { "Submitting to LeetCode…" } else { "Running on LeetCode…" }),
            Judge::Failed(err) => h_flex().text_xs().text_color(theme.danger).child(err.clone()),
            Judge::Done(r) => {
                let ok = r.accepted();
                let color = if ok { theme.success } else { theme.danger };
                let status = if r.status_code == 10 && !ok { "Wrong Answer".to_string() } else { r.status.clone() };
                let mut row = h_flex()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(color.opacity(0.1))
                    .text_xs()
                    .child(div().font_weight(FontWeight::BOLD).text_color(color).child(format!("LeetCode · {status}")));
                if let (Some(c), Some(t)) = (r.total_correct, r.total_testcases) {
                    row = row.child(format!("{c}/{t} testcases"));
                }
                if !r.runtime.is_empty() && r.submission {
                    row = row.child(format!("{} · {}", r.runtime, r.memory));
                }
                if let Some(p) = r.runtime_percentile {
                    row = row.child(div().text_color(theme.muted_foreground).child(format!("beats {p:.0}%")));
                }
                if !ok && !r.last_input.is_empty() {
                    row = row.child(div().truncate().text_color(theme.muted_foreground).child(format!(
                        "input {} → expected {}, got {}",
                        r.last_input.replace('\n', ", "),
                        r.expected_output,
                        r.actual_output
                    )));
                }
                if !r.error.is_empty() {
                    row = row.child(div().truncate().text_color(theme.danger).child(r.error.lines().last().unwrap_or("").to_string()));
                }
                row
            }
        };
        Some(el.into_any_element())
    }

    pub(crate) fn render_roadmap_graph(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let max_col = TOPICS.iter().map(|t| t.col).fold(0., f32::max);
        let max_row = TOPICS.iter().map(|t| t.row).max().unwrap_or(0) as f32;
        // Scale the graph to fit the window, leaving room for the title and status bar.
        let viewport = window.viewport_size();
        let (avail_w, avail_h) = (f32::from(viewport.width) - 64. - crate::roadmap::panel_width(window, self.roadmap.details), f32::from(viewport.height) - 182.);
        let (base_w, base_h) = (max_col * 190. + 168., max_row * 96. + 56.);
        let scale = (avail_w / base_w).min(avail_h / base_h).clamp(0.25, 1.3);
        let (node_w, node_h, col, row) = (168. * scale, 52., 190. * scale, ((avail_h - 52.) / max_row).min(96.));
        let (width, height) = (max_col * col + node_w, max_row * row + node_h);
        let pos = |t: &roadmap::Topic| (t.col * col, t.row as f32 * row);
        let edges: Vec<((f32, f32), (f32, f32))> = TOPICS
            .iter()
            .flat_map(|t| {
                let from = pos(t);
                t.children.iter().filter_map(move |c| roadmap::topic(c).map(|c| (from, pos(c))))
            })
            .collect();
        let edge_color = theme.border.opacity(1.);

        v_flex()
            .flex_1()
            .size_full()
            .items_center()
            .justify_start()
            .pt_6()
            .gap_6()
            .child(
                v_flex()
                    .items_center()
                    .gap_1()
                    .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(format!("{} roadmap", self.config.roadmap_list.label())))
            )
            .child(
                div()
                    .relative()
                    .w(px(width))
                    .h(px(height))
                    .child(
                        canvas(|_, _, _| (), move |bounds, _, window, _| {
                            for ((x1, y1), (x2, y2)) in &edges {
                                let start = point(bounds.origin.x + px(x1 + node_w / 2.), bounds.origin.y + px(y1 + node_h));
                                let end = point(bounds.origin.x + px(x2 + node_w / 2.), bounds.origin.y + px(*y2));
                                let mid = (start.y + end.y) / 2.;
                                let mut path = PathBuilder::stroke(px(1.5));
                                path.move_to(start);
                                path.cubic_bezier_to(end, point(start.x, mid), point(end.x, mid));
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, edge_color);
                                }
                            }
                        })
                        .absolute()
                        .size_full(),
                    )
                    .children(TOPICS.iter().enumerate().map(|(i, t)| {
                        let (x, y) = pos(t);
                        let (done, total) = self.library.progress(t.name);
                        let ratio = if total == 0 { 0. } else { done as f32 / total as f32 };
                        let fill = transition(SharedString::from(format!("node-fill-{i}")), ratio, Transition::new(Duration::from_millis(500)), window, cx);
                        let selected = i == self.roadmap_sel;
                        let highlight = transition(SharedString::from(format!("node-highlight-{i}")), if selected { 1. } else { 0. }, Transition::new(Duration::from_millis(140)), window, cx);
                        v_flex()
                            .id(("node", i))
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(node_w))
                            .h(px(node_h))
                            .px_2()
                            .justify_center()
                            .gap_1()
                            .rounded_md()
                            .bg(theme.secondary)
                            .border_1()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .child(div().absolute().inset_0().rounded_md().border_2().border_color(theme.primary).opacity(highlight))
                            .child(
                                div().text_center().text_size(px(11.)).font_weight(FontWeight::MEDIUM).child(t.name),
                            )
                            .child(
                                div()
                                    .h_1()
                                    .w_full()
                                    .rounded_full()
                                    .bg(theme.muted)
                                    .child(div().h_full().rounded_full().bg(if ratio >= 1. { theme.success } else { theme.primary }).w(relative(fill.clamp(0., 1.)))),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.roadmap_sel = i;
                                this.roadmap_open_topic(window, cx);
                            }))
                    })),
            )
    }

    fn render_status(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let flash = self.flash.clone();
        let flash_opacity = transition("flash", if flash.is_some() { 1. } else { 0. }, Transition::new(Duration::from_millis(160)), window, cx);
        h_flex()
            .h_7()
            .px_3()
            .gap_4()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.title_bar)
            .text_xs()
            .text_color(theme.muted_foreground)
            .when(self.syncing, |el| el.child(h_flex().gap_1().child(Spinner::new().xsmall()).child("syncing")))
            .child(div().flex_1())
            .when_some(flash, |el, (label, _)| {
                el.child(div().opacity(flash_opacity).text_color(theme.foreground).child(label))
            })
            .when(self.update_ready, |el| {
                el.child(h_flex().gap_1().text_color(theme.primary).child("Update ready").child(key(crate::actions::key_for("Restart", &self.config))))
            })
            .when_some(self.session.as_ref(), |row, session| {
                let detail = self.intelligence.detail(session.language);
                row.child(div().id("status-lsp").child(self.intelligence.label(session.language)).tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(detail.clone()).build(window, cx)))
            })
            .child(div().id("status-ai").child(h_flex().gap_1().items_center().child(crate::brand::icon(self.config.prompt_provider).xsmall()).child(self.config.prompt_style.label()))
                .tooltip({ let label = format!("{} · {}", self.config.prompt_provider.label(), self.config.prompt_style.label());
                    move |window, cx| gpui_kit::component::tooltip::Tooltip::new(label.clone()).action(&crate::actions::PromptDefault, Some(crate::actions::WORKSPACE)).build(window, cx) }))
            .child(if self.client.signed_in() { "LeetCode ✓" } else { "LeetCode signed out" })
            .child(h_flex().gap_1().child(key(crate::actions::key_for("Search", &self.config))).child("commands"))
    }
}

pub(crate) fn relative_time(secs: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let d = (now - secs).max(0);
    match d {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", d / 60),
        3600..86400 => format!("{}h ago", d / 3600),
        _ => format!("{}d ago", d / 86400),
    }
}
