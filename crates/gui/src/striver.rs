//! Native curriculum navigation; judging stays with each problem's provider.
use std::collections::HashSet;
use gpui_kit::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Selectable as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use practice::{creds::{self, Account}, striver::Sheet};
use crate::workspace::{Center, Focus, Workspace};

#[derive(Default)]
pub struct State {
    expanded: HashSet<String>,
    pub busy: bool,
    epoch: u64,
    progress: [HashSet<u64>; 2],
    pub error: Option<String>,
    scroll: ScrollHandle,
}

impl State {
    pub fn restore(&mut self, db: &practice::db::Db, user: &str) {
        for (i, sheet) in Sheet::ALL.into_iter().enumerate() {
            self.progress[i] = db.get(&format!("striver-progress:{user}:{}", sheet.slug())).ok().flatten()
                .and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or_default();
        }
    }

    pub fn clear_account(&mut self) {
        self.progress = Default::default();
        self.busy = false;
        self.error = None;
        self.epoch += 1;
    }
}

impl Workspace {
    pub fn open_striver(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sources.menu.hide();
        self.save_now(cx);
        self.center = Center::Sheets;
        self.focus_nav(Focus::Home, window, cx);
        cx.notify();
    }

    pub fn refresh_striver(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.striver.busy { return; }
        self.striver.busy = true;
        self.striver.error = None;
        self.striver.epoch += 1;
        let epoch = self.striver.epoch;
        let db = self.db.clone();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move {
                let session = practice::striver::validate(&creds::load(Account::TakeUForward)?)?;
                let mut marks = [HashSet::new(), HashSet::new()];
                for (i, sheet) in Sheet::ALL.into_iter().enumerate() {
                    marks[i] = practice::striver::progress(sheet, &session)?;
                }
                for (sheet, marks) in Sheet::ALL.into_iter().zip(&marks) {
                    db.set(&format!("striver-progress:{}:{}", session.user_id, sheet.slug()), &serde_json::to_string(marks)?)?;
                }
                anyhow::Ok((session, marks))
            }).await;
            let _ = this.update_in(cx, |this, _, cx| {
                if epoch != this.striver.epoch { return; }
                this.striver.busy = false;
                match result {
                    Ok((session, marks)) => {
                        this.account_names[2] = session.display_name().unwrap_or("Account").to_owned();
                        this.striver.progress = marks;
                    }
                    Err(_) => this.striver.error = Some("Could not sync TUF progress. Retry or reconnect your account.".into()),
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn render_striver(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let sheet = self.config.striver_sheet;
        let index = if sheet == Sheet::A2z { 0 } else { 1 };
        let data = sheet.data();
        let done = |item: &practice::striver::Item| self.striver.progress[index].contains(&item.id)
            || item.leetcode_slug.as_ref().is_some_and(|slug| self.solved.contains(slug));
        let total = data.sections.iter().map(|section| section.items.len()).sum::<usize>();
        let solved = data.sections.iter().flat_map(|section| &section.items).filter(|item| done(item)).count();
        v_flex().size_full().min_h_0().p_4().gap_3()
            .child(h_flex().gap_2().flex_wrap()
                .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Striver sheets"))
                .children(Sheet::ALL.into_iter().map(|choice| Button::new(choice.slug()).tab_stop(true).ghost().label(choice.label())
                    .selected(sheet == choice).on_click(cx.listener(move |this, _, window, cx| {
                        this.config.striver_sheet = choice;
                        this.striver.scroll = ScrollHandle::new();
                        this.save_config(window, cx);
                        cx.notify();
                    }))))
                .child(div().flex_1())
                .child(div().text_sm().text_color(theme.muted_foreground).child(format!("{solved} / {total}")))
                .child(Button::new("striver-account").tab_stop(true).ghost().label(if self.account_names[2] == "Signed out" { "Connect TUF…" } else { "TUF account…" })
                    .on_click(cx.listener(|this, _, window, cx| crate::accounts::open(Account::TakeUForward, this.account_names[2] != "Signed out", window, cx))))
                .child(Button::new("striver-sync").tab_stop(true).ghost().icon(IconName::RefreshCw).accessibility_label("Sync TUF progress")
                    .disabled(self.striver.busy || self.account_names[2] == "Signed out")
                    .label(if self.striver.busy { "Syncing" } else { "Sync" })
                    .on_click(cx.listener(|this, _, window, cx| this.refresh_striver(window, cx)))))
            .when_some(self.striver.error.clone(), |view, error| view.child(div().text_sm().text_color(theme.danger).child(error)))
            .child(div().text_xs().text_color(theme.muted_foreground).child("Linked problems open in the editor. Lessons and TUF exercises open in your browser."))
            .child(v_flex().id("striver-sections").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.striver.scroll).gap_2()
                .children(data.sections.iter().map(|section| {
                    let key = format!("{}:{}", sheet.slug(), section.id);
                    let expanded = self.striver.expanded.contains(&key);
                    let completed = section.items.iter().filter(|item| done(item)).count();
                    v_flex().id(("striver-group", section.id)).flex_shrink_0().gap_1()
                        .child(Button::new(SharedString::from(format!("striver-section-{}", section.id))).tab_stop(true).ghost().w_full().justify_start()
                            .icon(if expanded { IconName::ChevronDown } else { IconName::ChevronRight })
                            .label(section.path.join(" / "))
                            .child(div().flex_1())
                            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{completed} / {}", section.items.len())))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.striver.expanded.remove(&key) { this.striver.expanded.insert(key.clone()); }
                                cx.notify();
                            })))
                        .when(expanded, |view| view.child(v_flex().pl_4().gap_1().children(section.items.iter().map(|item| {
                            let slug = item.leetcode_slug.clone();
                            let url = item.url.clone();
                            let completed = done(item);
                            Button::new(SharedString::from(format!("striver-item-{}", item.id))).tab_stop(true).ghost().w_full().justify_start()
                                .icon(if completed { IconName::Check } else if slug.is_some() { IconName::Code } else { IconName::ExternalLink })
                                .label(item.title.clone()).child(div().flex_1())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Some(slug) = &slug { this.open_problem(slug.clone(), window, cx); }
                                    else { this.roadmap_link(url.clone(), window, cx); }
                                }))
                        }))))
                })))
    }
}
