//! Cached source selection and the compact topic explorer.
use std::collections::{HashMap, HashSet};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use practice::db::Db;
use practice::language::Source;
use practice::leetcode::CatalogItem;
use practice::roadmap::{List, TOPICS};
use crate::workspace::{Focus, Workspace};

#[derive(Default)]
pub struct SourcesState {
    pub catalog: Vec<CatalogItem>,
    pub by_slug: HashMap<String, usize>,
    pub ratings: HashMap<String, u32>,
    pub solved: HashSet<String>,
    pub loading: bool,
}
impl SourcesState {
    pub fn load(db: &Db, handle: &str) -> Self {
        let mut state = Self::default();
        if let Some(raw) = db.get("cf-catalog").ok().flatten() {
            if let Ok(problems) = serde_json::from_str(&raw) { state.set_catalog(problems); }
        }
        state.solved = db.get(&format!("cf-progress:{handle}")).ok().flatten().map(|raw| raw.lines().map(str::to_owned).collect()).unwrap_or_default();
        state
    }
    pub fn upsert_problems(&mut self, problems: Vec<practice::codeforces::Problem>) {
        for problem in problems {
            let Some(slug) = problem.slug() else { continue; };
            if let Some(rating) = problem.rating { self.ratings.insert(slug.clone(), rating); }
            let item = CatalogItem { slug: slug.clone(), frontend_id: problem.contest_id.unwrap_or(0), title: problem.name,
                level: match problem.rating { Some(rating) if rating < 1200 => 1, Some(rating) if rating >= 2000 => 3, _ => 2 },
                paid_only: false, ac_rate: 0., status: None };
            if let Some(&index) = self.by_slug.get(&slug) { self.catalog[index] = item; }
            else { self.by_slug.insert(slug, self.catalog.len()); self.catalog.push(item); }
        }
    }
    fn set_catalog(&mut self, problems: Vec<practice::codeforces::Problem>) {
        self.ratings.clear();
        self.catalog = problems.into_iter().filter_map(|problem| {
            let slug = problem.slug()?;
            if let Some(rating) = problem.rating { self.ratings.insert(slug.clone(), rating); }
            Some(CatalogItem { slug, frontend_id: problem.contest_id?, title: problem.name,
                level: match problem.rating { Some(rating) if rating < 1200 => 1, Some(rating) if rating >= 2000 => 3, _ => 2 },
                paid_only: false, ac_rate: 0., status: None })
        }).collect();
        self.by_slug = self.catalog.iter().enumerate().map(|(index, item)| (item.slug.clone(), index)).collect();
    }
}

pub const TOPIC_COLUMNS: isize = 4;
pub fn topic_icon(index: usize) -> IconName {
    match TOPICS[index].name {
        "Arrays & Hashing" => IconName::Brackets, "Two Pointers" => IconName::ArrowLeftRight,
        "Stack" => IconName::Layers, "Binary Search" => IconName::Search,
        "Sliding Window" => IconName::PanelTop, "Linked List" => IconName::Link,
        "Trees" => IconName::Network, "Tries" => IconName::GitBranch,
        "Heap / Priority Queue" => IconName::ListOrdered, "Backtracking" => IconName::Undo2,
        "Intervals" => IconName::CalendarRange, "Greedy" => IconName::Zap,
        "Graphs" => IconName::Share2, "Advanced Graphs" => IconName::Workflow,
        "1-D Dynamic Programming" => IconName::ChartColumn, "2-D Dynamic Programming" => IconName::Grid2x2,
        "Bit Manipulation" => IconName::Binary, _ => IconName::Calculator,
    }
}
fn short_topic(index: usize) -> &'static str {
    match TOPICS[index].name {
        "Arrays & Hashing" => "Arrays", "Two Pointers" => "Pointers", "Binary Search" => "Search",
        "Sliding Window" => "Window", "Linked List" => "Lists", "Heap / Priority Queue" => "Heap",
        "Backtracking" => "Backtrack", "Advanced Graphs" => "Adv. graph",
        "1-D Dynamic Programming" => "1-D DP", "2-D Dynamic Programming" => "2-D DP", "Bit Manipulation" => "Bits",
        "Math & Geometry" => "Math", name => name,
    }
}

impl Workspace {
    pub fn active_catalog(&self) -> &[CatalogItem] {
        if self.config.source == Source::Codeforces { &self.sources.catalog } else { &self.catalog }
    }
    pub fn choose_source(&mut self, source: Source, window: &mut Window, cx: &mut Context<Self>) {
        self.contests.selected = None;
        self.config.source = source;
        self.save_config(window, cx);
        self.rebuild_rows();
        self.omni.stale = true;
        self.omni.open = false;
        self.show_home(window, cx);
        self.home.sidebar = true;
        self.left = true;
        if source == Source::Codeforces {
            self.refresh_codeforces(false, window, cx);
            if self.config.codeforces_handle.is_empty() { self.begin_onboarding(true, window, cx); }
        } else if source == Source::NeetCode { self.focus_nav(Focus::Explorer, window, cx); }
        else { self.focus_nav(Focus::Sidebar, window, cx); }
        cx.notify();
    }
    pub fn refresh_codeforces(&mut self, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.sources.loading { return; }
        self.sources.loading = true;
        let (db, handle) = (self.db.clone(), self.config.codeforces_handle.clone());
        let fetch_catalog = force || self.sources.catalog.is_empty();
        cx.spawn_in(window, async move |this, cx| {
            let account = handle.clone();
            let result = cx.background_spawn(async move {
                let catalog = if fetch_catalog {
                    let problems = practice::codeforces::catalog()?;
                    db.set("cf-catalog", &serde_json::to_string(&problems)?)?;
                    Some(problems)
                } else { None };
                let progress = (|| -> anyhow::Result<_> { if handle.is_empty() { Ok(None) } else {
                    let progress = practice::codeforces::solved(&handle)?;
                    db.set(&format!("cf-progress:{handle}"), &progress.iter().cloned().collect::<Vec<_>>().join("\n"))?;
                    Ok(Some(progress))
                } })();
                anyhow::Ok((catalog, progress))
            }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.sources.loading = false;
                match result {
                    Ok((catalog, progress)) => {
                        if let Some(catalog) = catalog {
                            this.sources.set_catalog(catalog);
                            if let Some(id) = this.contests.selected { this.sources.upsert_problems(practice::contests::cached_problems(&this.db, id).unwrap_or_default()); }
                        }
                        if this.config.codeforces_handle == account {
                            match progress {
                                Ok(Some(progress)) => this.sources.solved = progress,
                                Ok(None) => {},
                                Err(error) => this.toast(Notification::warning(format!("Codeforces progress: {error}")), window, cx),
                            }
                        }
                        this.rebuild_rows(); this.omni.stale = true;
                    }
                    Err(error) => this.toast(Notification::error(format!("Codeforces: {error}")), window, cx),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    pub fn explorer_move(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.explorer_topic = (self.explorer_topic as isize + delta).rem_euclid(TOPICS.len() as isize) as usize;
        self.rebuild_rows(); self.sidebar_sel = 0;
        self.sidebar_scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }
    pub fn render_source_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let label = if self.config.source == Source::NeetCode { self.config.roadmap_list.label().to_owned() } else if let Some(id) = self.contests.selected.filter(|_| self.config.source == Source::Codeforces) { format!("Contest {id}") } else { self.config.source.label().to_owned() };
        let current = self.config.source;
        let current_list = self.config.roadmap_list;
        Button::new("practice-source").ghost().small().label(label).icon(IconName::ChevronDown)
            .accessibility_label("Practice source").tooltip_with_action("Practice source", &crate::actions::CycleSource, Some(crate::actions::WORKSPACE))
            .dropdown_menu(move |mut menu, _, _| {
                for source in Source::ALL {
                    let weak = weak.clone();
                    menu = menu.item(PopupMenuItem::new(source.label()).checked(current == source).on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |ws, cx| ws.choose_source(source, window, cx));
                    }));
                }
                if current == Source::NeetCode {
                    menu = menu.separator();
                    for list in [List::NeetCode150, List::NeetCode250, List::All] {
                        let weak = weak.clone();
                        menu = menu.item(PopupMenuItem::new(list.label()).checked(current_list == list).on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |ws, cx| { ws.config.roadmap_list = list; ws.save_config(window, cx); ws.rebuild_library(); cx.notify(); });
                        }));
                    }
                }
                menu
            })
    }
    pub fn render_topic_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().px_2().gap_1().children(TOPICS.chunks(TOPIC_COLUMNS as usize).enumerate().map(|(row, topics)| {
            h_flex().gap_1().children(topics.iter().enumerate().map(|(column, topic)| {
                let index = row * TOPIC_COLUMNS as usize + column;
                let (done, total) = self.library.progress(topic.name);
                crate::theme::selected_choice(Button::new(("explorer-topic", index)).ghost(), self.explorer_topic == index, cx)
                    .w(px(69. * self.config.zoom)).h(px(46. * self.config.zoom))
                    .child(v_flex().items_center().gap_1().child(Icon::new(topic_icon(index)).size(px(16.))).child(div().text_size(px(10.)).child(short_topic(index))))
                    .tooltip(format!("{} · {done}/{total} solved", topic.name)).accessibility_label(topic.name)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.explorer_topic = index; this.rebuild_rows(); this.sidebar_sel = 0;
                        this.sidebar_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        this.focus_nav(Focus::Explorer, window, cx);
                    }))
            }))
        }))
    }
}
