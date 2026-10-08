//! Cached Home contests and provider-scoped native explorers.
use std::collections::{HashMap, HashSet};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use practice::{contests::{ContestEntry, ContestId, ContestProblems}, db::Db, language::Source};
use crate::workspace::{Center, Focus, Workspace};

#[derive(Default)]
pub struct ContestsState {
    pub list: Vec<ContestEntry>,
    pub selected: Option<ContestId>,
    pub members: HashMap<ContestId, Vec<String>>,
    pub past_open: bool,
    pub list_loading: bool,
    pub loading: HashSet<ContestId>,
    pub status: Option<String>,
    pub problem_status: Option<String>,
}

impl ContestsState {
    pub fn includes(&self, source: Source, slug: &str) -> bool {
        match self.selected.as_ref().filter(|id| id.source() == source) {
            Some(ContestId::Codeforces(id)) => practice::codeforces::problem_id(slug).is_ok_and(|(contest, _)| contest == *id),
            Some(id @ ContestId::LeetCode(_)) => self.members.get(id).is_some_and(|slugs| slugs.iter().any(|member| member == slug)),
            None => true,
        }
    }
    pub fn load(db: &Db) -> Self { Self { list: practice::contests::cached_entries(db).unwrap_or_default(), ..Default::default() } }
    pub fn visible(&self) -> Vec<&ContestEntry> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut visible: Vec<_> = self.list.iter().filter(|c| !c.past(now) && c.id().source() == Source::Codeforces).take(3)
            .chain(self.list.iter().filter(|c| !c.past(now) && c.id().source() == Source::LeetCode).take(2)).collect();
        visible.sort_by_key(|c| c.start());
        if self.past_open {
            let mut past: Vec<_> = self.list.iter().filter(|c| c.past(now)).collect();
            past.sort_by_key(|c| std::cmp::Reverse(c.start()));
            visible.extend(past.into_iter().take(10));
        }
        visible
    }
}

impl Workspace {
    pub fn refresh_contests(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.contests.list_loading { return; }
        self.contests.list_loading = true;
        let db = self.db.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { practice::contests::refresh_entries(&db) }).await;
            let _ = this.update(cx, |this, cx| {
                this.contests.list_loading = false;
                match result {
                    Ok((list, errors)) => { this.contests.list = list; this.contests.status = if errors.is_empty() { None } else { Some(errors.join(" · ")) }; this.omni.stale = true; }
                    Err(error) => this.contests.status = Some(format!("Contest refresh failed: {error}")),
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn open_contest(&mut self, id: ContestId, window: &mut Window, cx: &mut Context<Self>) {
        self.save_now(cx);
        self.refresh_contests(window, cx);
        self.config.source = id.source(); self.save_config(window, cx);
        self.contests.selected = Some(id.clone());
        self.contests.problem_status = None;
        if let Ok(problems) = practice::contests::cached_contest_problems(&self.db, &id) { self.apply_contest_problems(&id, problems); }
        self.rebuild_rows(); self.sidebar_sel = 0;
        self.home.list = crate::home::HomeList::Contests;
        self.center = Center::Home; self.left = true; self.home.sidebar = true; self.history_mode = false;
        self.focus_nav(Focus::Sidebar, window, cx);
        self.refresh_contest_problems(id, window, cx);
        cx.notify();
    }

    pub fn refresh_contest_problems(&mut self, id: ContestId, window: &mut Window, cx: &mut Context<Self>) {
        if !self.contests.loading.insert(id.clone()) { return; }
        let db = self.db.clone();
        let client = self.client.clone();
        let lookup = id.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { practice::contests::refresh_contest_problems(&db, &lookup, &client) }).await;
            let _ = this.update(cx, |this, cx| {
                this.contests.loading.remove(&id);
                match result {
                    Ok(problems) => {
                        this.apply_contest_problems(&id, problems); this.omni.stale = true;
                        if this.contests.selected.as_ref() == Some(&id) { this.rebuild_rows(); this.contests.problem_status = None; }
                    }
                    Err(error) if this.contests.selected.as_ref() == Some(&id) => this.contests.problem_status =
                        if this.contests.list.iter().any(|contest| contest.id() == id && contest.start() > std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()) { Some("Problems appear when the contest starts".into()) }
                        else { Some(format!("Problems unavailable: {error}")) },
                    Err(_) => {},
                }
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn apply_contest_problems(&mut self, id: &ContestId, problems: ContestProblems) {
        match problems {
            ContestProblems::Codeforces(problems) => self.sources.upsert_problems(problems),
            ContestProblems::LeetCode(items) => {
                self.contests.members.insert(id.clone(), items.iter().map(|item| item.slug.clone()).collect());
                let catalog = std::rc::Rc::make_mut(&mut self.catalog);
                for item in items {
                    if !self.by_slug.contains_key(&item.slug) { self.by_slug.insert(item.slug.clone(), catalog.len()); catalog.push(item); }
                }
            }
        }
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
                let id = contest.id();
                let key = id.key();
                let url = id.url();
                h_flex().id(SharedString::from(format!("home-contest-{key}"))).flex_shrink_0().gap_3().px_3().py_2().rounded_lg().cursor_pointer()
                    .bg(if self.home.list == crate::home::HomeList::Contests && self.focus_area == Focus::Home && self.home.selected == index { theme.list_active } else { theme.background })
                    .hover(|row| row.bg(theme.list_hover))
                    .child(crate::brand::source_icon(id.source()).small())
                    .child(v_flex().flex_1().min_w_0().gap_1()
                        .child(div().truncate().child(contest.name().to_owned()))
                        .child(h_flex().justify_between().gap_2()
                            .child(div().text_xs().text_color(if contest.past(now) { theme.muted_foreground } else { theme.primary }).child(contest.timing(now)))
                            .child(Button::new(SharedString::from(format!("home-contest-web-{key}"))).ghost().small()
                                .icon(gpui_kit::component::Icon::new(IconName::ExternalLink).text_color(theme.primary))
                                .tooltip("Open in web")
                                .accessibility_label(format!("Open {} in browser", contest.name()))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let _ = open::that_detached(url.clone());
                                }))))
                    .on_click(cx.listener(move |this, _, window, cx| this.open_contest(id.clone(), window, cx)))
            }))
            .when(self.contests.selected.as_ref().is_some_and(|id| self.contests.loading.contains(id)), |view| view.child(div().text_xs().text_color(theme.muted_foreground).child("Refreshing contest problems…")))
            .when_some(self.contests.problem_status.as_ref().or(self.contests.status.as_ref()), |view, status| view.child(div().text_xs().text_color(theme.muted_foreground).child(status.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::ContestsState;
    use practice::contests::{Contest, ContestEntry, ContestId, LeetCodeContest};
    use practice::language::Source;

    #[test]
    fn home_keeps_both_providers_visible_and_orders_upcoming_and_past() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let cf = |id, phase: &str, start| ContestEntry::Codeforces(Contest { id, name: format!("Round {id}"), phase: phase.into(), duration_seconds: 5400, start_time_seconds: Some(start) });
        let lc = |slug: &str, start| ContestEntry::LeetCode(LeetCodeContest { title: slug.into(), title_slug: slug.into(), start_time: start, duration: 5400 });
        let state = ContestsState { list: vec![cf(1, "BEFORE", now + 10), cf(2, "BEFORE", now + 20), cf(3, "BEFORE", now + 30), cf(4, "BEFORE", now + 40), cf(5, "FINISHED", now - 9000), lc("weekly-contest-1", now + 100), lc("biweekly-contest-1", now + 90), lc("weekly-contest-2", now - 6000)], ..Default::default() };
        let visible = state.visible();
        assert_eq!(visible.len(), 5);
        assert_eq!(visible.iter().filter(|c| c.id().source() == Source::LeetCode).count(), 2);
        assert!(visible.windows(2).all(|pair| pair[0].start() <= pair[1].start()));
        let state = ContestsState { past_open: true, ..state };
        let visible = state.visible();
        assert_eq!(visible.len(), 7);
        assert_eq!(visible[5].id(), ContestId::LeetCode("weekly-contest-2".into()));
        assert_eq!(visible[6].id(), ContestId::Codeforces(5));
    }

    #[test]
    fn contest_scope_cannot_leak_other_contests_or_provider_catalogs() {
        let id = ContestId::LeetCode("weekly-contest-1".into());
        let mut state = ContestsState { selected: Some(id.clone()), ..Default::default() };
        assert!(!state.includes(Source::LeetCode, "two-sum"));
        state.members.insert(id, vec!["two-sum".into()]);
        assert!(state.includes(Source::LeetCode, "two-sum"));
        assert!(!state.includes(Source::LeetCode, "three-sum"));
        assert!(state.includes(Source::Codeforces, "cf:1:A"));
        state.selected = Some(ContestId::Codeforces(1));
        assert!(state.includes(Source::Codeforces, "cf:1:A"));
        assert!(!state.includes(Source::Codeforces, "cf:2:A"));
        assert!(!state.includes(Source::Codeforces, "two-sum"));
        state.selected = None;
        assert!(state.includes(Source::LeetCode, "three-sum"));
    }
}
