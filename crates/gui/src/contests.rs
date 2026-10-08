//! Cached Home contests and a contest-scoped Codeforces explorer.
use std::collections::HashSet;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use practice::{contests::Contest, db::Db, language::Source};
use crate::workspace::{Center, Focus, Workspace};

#[derive(Default)]
pub struct ContestsState {
    pub list: Vec<Contest>,
    pub selected: Option<u32>,
    pub past_open: bool,
    pub list_loading: bool,
    pub loading: HashSet<u32>,
    pub status: Option<String>,
    pub problem_status: Option<String>,
}

impl ContestsState {
    pub fn load(db: &Db) -> Self { Self { list: practice::contests::cached_list(db).unwrap_or_default(), ..Default::default() } }
    pub fn visible(&self) -> Vec<&Contest> {
        self.list.iter().filter(|contest| !contest.past()).take(3)
            .chain(self.list.iter().filter(|contest| contest.past() && self.past_open).take(10)).collect()
    }
}

impl Workspace {
    pub fn refresh_contests(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.contests.list_loading { return; }
        self.contests.list_loading = true;
        let db = self.db.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { practice::contests::refresh_list(&db) }).await;
            let _ = this.update(cx, |this, cx| {
                this.contests.list_loading = false;
                match result {
                    Ok(list) => { this.contests.list = list; this.contests.status = None; this.omni.stale = true; }
                    Err(error) => this.contests.status = Some(format!("Contest refresh failed: {error}")),
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn open_contest(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.save_now(cx);
        self.refresh_contests(window, cx);
        self.config.source = Source::Codeforces; self.save_config(window, cx);
        self.contests.selected = Some(id);
        self.contests.problem_status = None;
        let problems = practice::contests::cached_problems(&self.db, id).unwrap_or_default();
        self.sources.upsert_problems(problems);
        self.rebuild_rows(); self.sidebar_sel = 0;
        self.home.list = crate::home::HomeList::Contests;
        self.center = Center::Home; self.left = true; self.home.sidebar = true; self.history_mode = false;
        self.focus_nav(Focus::Sidebar, window, cx);
        self.refresh_contest_problems(id, window, cx);
        cx.notify();
    }

    pub fn refresh_contest_problems(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.contests.loading.insert(id) { return; }
        let db = self.db.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { practice::contests::refresh_problems(&db, id) }).await;
            let _ = this.update(cx, |this, cx| {
                this.contests.loading.remove(&id);
                match result {
                    Ok(problems) => {
                        this.sources.upsert_problems(problems); this.omni.stale = true;
                        if this.contests.selected == Some(id) { this.rebuild_rows(); this.contests.problem_status = None; }
                    }
                    Err(error) if this.contests.selected == Some(id) => this.contests.problem_status =
                        if this.contests.list.iter().any(|contest| contest.id == id && contest.phase == "BEFORE") { None }
                        else { Some(format!("Problems unavailable: {error}")) },
                    Err(_) => {},
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn render_contests(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let visible = self.contests.visible();
        v_flex().id("home-contests").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.home.contest_scroll).gap_2()
            .child(h_flex().h_9().flex_shrink_0().items_center().justify_between()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Contests"))
                .child(Button::new("past-contests").ghost().small().icon(if self.contests.past_open { IconName::ChevronDown } else { IconName::ChevronRight })
                    .accessibility_label("Show or hide past contests").tooltip("Past contests")
                    .on_click(cx.listener(|this, _, _, cx| { this.contests.past_open = !this.contests.past_open; cx.notify(); }))))
            .when(visible.is_empty(), |view| view.child(div().text_sm().text_color(theme.muted_foreground)
                .child(if self.contests.list_loading { "Loading contests…" } else { "No upcoming contests" })))
            .children(visible.iter().enumerate().map(|(index, contest)| {
                let id = contest.id;
                h_flex().id(("home-contest", id)).flex_shrink_0().gap_3().px_3().py_2().rounded_lg().cursor_pointer()
                    .bg(if self.home.list == crate::home::HomeList::Contests && self.focus_area == Focus::Home && self.home.selected == index { theme.list_active } else { theme.background })
                    .hover(|row| row.bg(theme.list_hover))
                    .child(crate::brand::source_icon(Source::Codeforces).small())
                    .child(v_flex().flex_1().min_w_0().gap_1()
                        .child(div().truncate().child(contest.name.clone()))
                        .child(h_flex().justify_between().gap_2()
                            .child(div().text_xs().text_color(if contest.past() { theme.muted_foreground } else { theme.primary }).child(contest.timing(now)))
                            .child(Button::new(("home-contest-web", id)).ghost().small().icon(IconName::ExternalLink).label("Open in web")
                                .accessibility_label(format!("Open {} in browser", contest.name))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let _ = open::that_detached(format!("https://codeforces.com/contest/{id}"));
                                }))))
                    .on_click(cx.listener(move |this, _, window, cx| this.open_contest(id, window, cx)))
            }))
            .when(self.contests.selected.is_some_and(|id| self.contests.loading.contains(&id)), |view| view.child(div().text_xs().text_color(theme.muted_foreground).child("Refreshing contest problems…")))
            .when_some(self.contests.problem_status.as_ref().or(self.contests.status.as_ref()), |view, status| view.child(div().text_xs().text_color(theme.muted_foreground).child(status.clone())))
    }
}
