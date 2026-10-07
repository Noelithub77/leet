//! The single window: app state, the open problem, and the work behind every action.
//! Action wiring lives in `commands.rs`; rendering in `view.rs`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui_kit::component::input::{Editor, EditorState, InputEvent, TabSize};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use practice::config::Config;
use practice::creds::{self, Account};
use practice::db::Db;
use practice::git;
use practice::leetcode::{CatalogItem, Client, JudgeResult, Question};
use practice::prompts::{self, Style};
use practice::roadmap::{self, ENTRIES, TOPICS};
use practice::language::{Language, Source};
use practice::runner::{self, Case, CaseResult, Compare, Verdict};
use practice::workspace as ws;

use crate::library::Library;
use crate::omnibar::Omnibar;
use crate::settings::SettingsState;
use crate::statement::Statement;

const RECENT_LIMIT: usize = 12;

/// One row of the left sidebar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Catalog(usize),
    /// Index into `roadmap::ENTRIES`.
    Problem(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Home,
    Editor,
    Sidebar,
    Explorer,
    Roadmap,
    RoadmapTopic,
    Settings,
    Omnibar,
}

/// What fills the center column.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Center {
    Onboarding,
    Home,
    Editor,
    Roadmap,
    Settings,
}

pub enum Judge {
    Running { submission: bool },
    Done(JudgeResult),
    Failed(String),
}

/// The open problem.
pub struct Session {
    pub slug: String,
    pub title: String,
    pub frontend_id: u32,
    pub question: Option<Question>,
    pub language: Language,
    pub source: Source,
    pub rel: PathBuf,
    pub path: PathBuf,
    pub disk_mtime: Option<SystemTime>,
    pub cases: Vec<Case>,
    pub results: Vec<Option<CaseResult>>,
    pub compile_error: Option<String>,
    pub selected_case: usize,
    pub running: bool,
    /// Bumped per run so result animations restart.
    pub run_id: u64,
    pub judge: Option<Judge>,
    pub hints_shown: usize,
    pub history: Vec<crate::history::LocalVersion>,
    pub selected_commit: usize,
    pub remote_versions: Vec<crate::history::RemoteVersion>,
    pub history_status: Option<String>,
    pub history_loading: bool,
    pub history_loaded: bool,
    pub version_sequence: u64,
    pub history_sequence: u64,
}

impl Session {
    pub fn failures(&self) -> Vec<String> {
        self.cases
            .iter()
            .zip(&self.results)
            .filter_map(|(case, r)| {
                let r = r.as_ref()?;
                let input = case.input.replace('\n', ", ");
                match r.verdict {
                    Verdict::Fail => Some(format!(
                        "input `{input}` → expected `{}`, got `{}`",
                        case.expected.as_deref().unwrap_or(""),
                        r.output
                    )),
                    Verdict::Error | Verdict::Timeout => {
                        Some(format!("input `{input}` → {}", r.error.lines().last().unwrap_or("error")))
                    }
                    _ => None,
                }
            })
            .collect()
    }
}

/// The editor in its own view, so typing re-renders only the editor.
pub struct EditorPane { state: Entity<EditorState>, _observe: Subscription }
impl EditorPane {
    pub fn new(state: Entity<EditorState>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&state, |_, _, cx| cx.notify());
        Self { state, _observe: observe }
    }
}


impl Render for EditorPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let hover = state.diagnostics().and_then(|set| set.iter().find_map(|entry| {
            let mut bounds = state.range_to_bounds(&entry.range)?;
            bounds.size.width = bounds.size.width.max(px(8.));
            bounds.contains(&window.mouse_position()).then(|| (entry.message.clone(), bounds))
        }));
        let input_bounds = state.input_bounds();
        let origin = input_bounds.origin;
        let cursor = state.cursor_layout().map(|(bounds, _)| bounds);
        // The default diagnostic popover sits on the source line. Keep its squiggles,
        // and place the message below the hovered range instead.
        self.state.update(cx, |editor, cx| editor.clear_diagnostic_popover(cx));
        div().relative().size_full().on_mouse_move(cx.listener(|this, _, _, cx| {
            this.state.update(cx, |editor, cx| editor.clear_diagnostic_popover(cx)); cx.notify();
        })).on_action(cx.listener(|this, _: &crate::language_server::Complete, window, cx| {
            let editor = this.state.clone();
            let state = editor.read(cx); let offset = state.cursor();
            let Some(provider) = state.lsp().completion_provider.clone() else { return; };
            let text = state.text().clone(); let snapshot = text.to_string();
            let start = snapshot[..offset].char_indices().rev().find(|(_, ch)| !ch.is_alphanumeric() && *ch != '_').map_or(0, |(index, ch)| index + ch.len_utf8());
            let query = snapshot[start..offset].to_owned();
            let task = provider.completions(&text, offset, lsp_types::CompletionContext { trigger_kind: lsp_types::CompletionTriggerKind::INVOKED, trigger_character: None }, window, cx);
            cx.spawn_in(window, async move |_, cx| {
                if let Ok(response) = task.await {
                    let items = match response { lsp_types::CompletionResponse::Array(items) => items, lsp_types::CompletionResponse::List(list) => list.items };
                    let _ = editor.update(cx, |editor, cx| { if editor.cursor() == offset && editor.value().as_str() == snapshot { editor.present_completion_items(start, &query, items, cx); } });
                }
            }).detach();
        })).child(Editor::new(&self.state).bordered(false).h_full())
            .when_some(hover, |view, (message, bounds)| view.child(deferred(
                div().absolute().left((cursor.unwrap_or(bounds).origin.x - origin.x).clamp(px(0.), (input_bounds.size.width - px(420.)).max(px(0.))))
                    .top(cursor.unwrap_or(bounds).bottom().max(bounds.bottom()) - origin.y + px(8.)).max_w(px(420.)).max_h(px(180.)).id("hovered-diagnostic").overflow_y_scroll().p_3().rounded_lg()
                    .bg(rgb(0x202024)).border_1().border_color(rgb(0xffa6a6).opacity(0.4)).shadow_md()
                    .text_sm().text_color(rgb(0xffb4b4)).child(message)
            )))
    }
}

pub struct Workspace {
    pub companion_task: Option<Task<()>>,
    pub intelligence: crate::language_server::Intelligence,
    pub onboarding: Option<Entity<crate::onboarding::Setup>>,
    pub home: crate::home::HomeState,
    pub recent_slugs: Vec<String>,
    pub focus: FocusHandle,
    pub nav_focus: FocusHandle,
    pub focus_area: Focus,
    pub db: Arc<Db>,
    pub client: Arc<Client>,
    pub neetcode: Arc<practice::neetcode::Client>,
    pub neetcode_solved: HashSet<String>,
    pub neetcode_syncing: bool,
    pub account_names: [String; 2],
    pub config: Config,
    pub catalog: Rc<Vec<CatalogItem>>,
    pub by_slug: HashMap<String, usize>,
    pub solved: HashSet<String>,
    pub library: Library,
    pub left: bool,
    pub right: bool,
    pub bottom: bool,
    pub zen: bool,
    pub history_mode: bool,
    pub center: Center,
    pub roadmap_sel: usize,
    pub roadmap: crate::roadmap::RoadmapState,
    pub explorer_topic: usize,
    pub sources: crate::sources::SourcesState,
    pub contests: crate::contests::ContestsState,
    pub rows: Vec<Row>,
    pub sidebar_sel: usize,
    pub sidebar_scroll: UniformListScrollHandle,
    pub session: Option<Session>,
    pub tabs: Vec<Option<crate::home::ProblemTab>>,
    pub active_tab: Option<usize>,
    pub editor: Entity<EditorState>,
    pub editor_subscription: Option<Subscription>,
    pub editor_pane: Entity<EditorPane>,
    pub statement: Entity<Statement>,
    pub omni: Omnibar,
    pub settings: SettingsState,
    pub syncing: bool,
    pub update_ready: bool,
    pub release_update: crate::update::State,
    pub ai: crate::ai::State,
    pub language_picker: crate::language_picker::State,
    pub case_edit: Option<crate::case_editor::Draft>,
    /// Last shortcut, shown briefly in the status bar.
    pub flash: Option<(SharedString, u64)>,
    flash_seq: u64,
    save_task: Task<()>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(config: Config, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let db = Arc::new(Db::open(&practice::config::database_path()).expect("open leet database"));
        let recent_slugs = db.get("recent").ok().flatten().map(|text| text.lines().map(str::to_owned).take(RECENT_LIMIT).collect()).unwrap_or_default();
        let client = Arc::new(Client::new(creds::load(Account::LeetCode).ok()));
        let accounts = [Account::LeetCode, Account::NeetCode].map(|account| creds::load(account).ok());
        let account_names = accounts.each_ref().map(|session| session.as_ref().map_or("Signed out".into(), |s| if s.username.is_empty() { "Saved session".into() } else { format!("{} · saved session", s.username) }));
        let neetcode_solved = accounts[1].as_ref().and_then(|s| db.get(&format!("neetcode-progress:{}", s.user_id)).ok().flatten())
            .map(|text| text.lines().map(str::to_owned).collect()).unwrap_or_default();
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("python")
                .line_number(true)
                .indent_guides(true)
                .folding(true)
                .soft_wrap(false)
                .tab_size(TabSize { tab_size: 4, hard_tabs: false })
                .placeholder("Open a problem with ctrl+p")
        });
        let editor_sub = cx.subscribe_in(&editor, window, |this, editor, event: &InputEvent, window, cx| {
            if *editor == this.editor && matches!(event, InputEvent::Change) {
                this.schedule_save(window, cx);
            }
        });
        // Shortcuts need a focused element inside the workspace from the first frame.
        let (omni, omni_sub) = Omnibar::new(window, cx);
        cx.set_global(crate::statement::TagsVisible(config.show_tags));
        cx.set_global(crate::statement::Sections(db.statement_sections().unwrap_or([true, false, false])));
        let mut this = Self {
            companion_task: None,
            onboarding: None,
            home: crate::home::HomeState::default(),
            recent_slugs,
            focus: cx.focus_handle(),
            nav_focus: cx.focus_handle(),
            focus_area: Focus::Home,
            roadmap: crate::roadmap::RoadmapState::load(&db),
            db: db.clone(),
            client,
            neetcode: Arc::new(practice::neetcode::Client::default()),
            neetcode_solved,
            neetcode_syncing: false,
            account_names,
            config: config.clone(),
            catalog: Rc::default(),
            by_slug: HashMap::new(),
            solved: HashSet::new(),
            library: Library::default(),
            left: true,
            right: true,
            bottom: true,
            zen: false,
            history_mode: false,
            center: Center::Home,
            roadmap_sel: 0,
            explorer_topic: 0,
            intelligence: Default::default(),
            sources: crate::sources::SourcesState::load(&db, &config.codeforces_handle),
            contests: crate::contests::ContestsState::load(&db),
            rows: vec![],
            sidebar_sel: 0,
            sidebar_scroll: UniformListScrollHandle::new(),
            session: None,
            tabs: vec![],
            active_tab: None,
            editor_pane: cx.new(|cx| EditorPane::new(editor.clone(), cx)),
            editor,
            editor_subscription: Some(editor_sub),
            statement: cx.new(|_| Statement::default()),
            omni,
            settings: SettingsState::new(),
            syncing: false,
            update_ready: false,
            release_update: crate::update::State::default(),
            ai: crate::ai::State::default(),
            language_picker: crate::language_picker::State::default(),
            case_edit: None,
            flash: None,
            flash_seq: 0,
            save_task: Task::ready(()),
            _tasks: vec![],
            _subscriptions: vec![omni_sub],
        };
        this.restore_layout();
        this.rebuild_library();
        let workspace = this.config.workspace.clone();
        cx.background_spawn(async move {
            if let Err(err) = git::ensure_repo(&workspace) {
                eprintln!("leet: git init {}: {err}", workspace.display());
            }
        })
        .detach();
        this.show_home(window, cx);
        let initial_focus = this.nav_focus.clone();
        window.defer(cx, move |window, cx| initial_focus.focus(window, cx));
        this.load_catalog(window, cx);
        if accounts[1].is_some() { this.refresh_neetcode(window, cx); }
        this._tasks.push(this.watch_disk(window, cx));
        let watcher = crate::update::watch(&mut this, window, cx);
        this._tasks.push(watcher);
        this._tasks.push(crate::update::check(window, cx));
        this.start_companion(window, cx);
        if !this.config.onboarding_completed { this.begin_onboarding(false, window, cx); }
        else if this.config.codeforces_handle.is_empty() { this.begin_onboarding(true, window, cx); }
        if this.config.source == Source::Codeforces { this.refresh_codeforces(false, window, cx); }
        this
    }

    // ---- persistence -------------------------------------------------------------------

    fn restore_layout(&mut self) {
        if let Ok(Some(layout)) = self.db.get("layout") {
            let flags: Vec<bool> = layout.chars().map(|c| c == '1').collect();
            if let [left, right, bottom, ..] = flags[..] {
                (self.left, self.right, self.bottom) = (left, right, bottom);
            }
        }
    }

    pub fn save_layout(&self) {
        let bit = |b: bool| if b { '1' } else { '0' };
        let layout: String = [self.left, self.right, self.bottom].map(bit).iter().collect();
        let _ = self.db.set("layout", &layout);
    }

    pub fn save_config(&self, window: &mut Window, cx: &mut App) {
        if let Err(err) = self.config.save() {
            window.push_notification(Notification::error(format!("Settings not saved: {err}")), cx);
        }
    }

    pub fn recent(&self) -> &[String] {
        &self.recent_slugs
    }

    fn push_recent(&mut self, slug: &str) {
        self.recent_slugs.retain(|s| s != slug);
        self.recent_slugs.insert(0, slug.to_owned());
        self.home.selected = 0;
        self.recent_slugs.truncate(RECENT_LIMIT);
        let _ = self.db.set("recent", &self.recent_slugs.join("\n"));
    }

    // ---- feedback ----------------------------------------------------------------------

    /// Shows the action in the status bar for a moment.
    pub fn flash(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.flash_seq += 1;
        let seq = self.flash_seq;
        self.flash = Some((label.into(), seq));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1400)).await;
            let _ = this.update(cx, |this, cx| {
                if this.flash.as_ref().is_some_and(|(_, s)| *s == seq) {
                    this.flash = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn toast(&self, note: impl Into<Notification>, window: &mut Window, cx: &mut App) {
        window.push_notification(note, cx);
    }

    // ---- catalog -----------------------------------------------------------------------

    fn load_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let db = self.db.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            let cached = cx
                .background_spawn(async move { (db.catalog().unwrap_or_default(), db.solved().unwrap_or_default()) })
                .await;
            let stale = this
                .update_in(cx, |this, _, cx| {
                    let empty = cached.0.is_empty();
                    this.set_catalog(cached.0, cached.1, cx);
                    empty || this.catalog_age_hours() > 12
                })
                .unwrap_or(false);
            if stale {
                let _ = this.update_in(cx, |this, window, cx| this.refresh_catalog(false, window, cx));
            }
        });
        self._tasks.push(task);
    }

    fn catalog_age_hours(&self) -> u64 {
        let then: u64 = self.db.get("catalog_at").ok().flatten().and_then(|s| s.parse().ok()).unwrap_or(0);
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        now.saturating_sub(then) / 3600
    }

    fn set_catalog(&mut self, items: Vec<CatalogItem>, solved: Vec<String>, cx: &mut Context<Self>) {
        self.by_slug = items.iter().enumerate().map(|(i, p)| (p.slug.clone(), i)).collect();
        self.catalog = Rc::new(items);
        self.solved = solved.into_iter().collect();
        self.omni.stale = true;
        self.rebuild_library();
        cx.notify();
    }

    /// Recomputes roadmap membership and progress after the list, catalog, or solved set changes.
    pub fn rebuild_library(&mut self) {
        let progress = self.solved.union(&self.neetcode_solved).cloned().collect();
        self.library = Library::build(self.config.roadmap_list, &self.catalog, &self.by_slug, &progress);
        self.rebuild_rows();
    }

    pub fn refresh_catalog(&mut self, announce: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        self.syncing = true;
        cx.notify();
        let (db, client) = (self.db.clone(), self.client.clone());
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let items = client.catalog()?;
                    db.replace_catalog(&items)?;
                    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
                    db.set("catalog_at", &now.to_string())?;
                    anyhow::Ok((db.catalog()?, db.solved()?, client.signed_in()))
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.syncing = false;
                match result {
                    Ok((items, solved, signed_in)) => {
                        let count = solved.len();
                        this.set_catalog(items, solved, cx);
                        if announce {
                            let who = if signed_in { format!("{count} solved synced") } else { "signed out".into() };
                            this.toast(Notification::success(format!("Catalog refreshed · {who}")), window, cx);
                        }
                    }
                    Err(err) => this.toast(Notification::error(format!("Catalog refresh failed: {err}")), window, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn item(&self, slug: &str) -> Option<&CatalogItem> {
        if slug.starts_with("cf:") { self.sources.by_slug.get(slug).map(|&i| &self.sources.catalog[i]) }
        else { self.by_slug.get(slug).map(|&i| &self.catalog[i]) }
    }

    // ---- sidebar -----------------------------------------------------------------------

    pub fn rebuild_rows(&mut self) {
        let selected = self.rows.get(self.sidebar_sel).copied();
        self.rows.clear();
        if self.config.source == Source::NeetCode {
            if let Some(topic) = self.library.topic(TOPICS[self.explorer_topic].name) {
                self.rows.extend(topic.entries.iter().map(|&entry| Row::Problem(entry)));
            }
        } else {
            self.rows = self.active_catalog().iter().enumerate().filter(|(_, item)| !item.paid_only && (self.config.source != Source::Codeforces || self.contests.selected.is_none_or(|id| practice::codeforces::problem_id(&item.slug).is_ok_and(|(contest, _)| contest == id)))).map(|(index, _)| Row::Catalog(index)).collect();
        }
        if let Some(i) = selected.and_then(|sel| self.rows.iter().position(|r| *r == sel)) {
            self.sidebar_sel = i;
        }
        self.sidebar_sel = self.sidebar_sel.min(self.rows.len().saturating_sub(1));
    }

    fn select_row(&mut self, i: usize, strategy: ScrollStrategy) {
        self.sidebar_sel = i;
        self.sidebar_scroll.scroll_to_item(i, strategy);
    }

    pub(crate) fn reveal_in_sidebar(&mut self, slug: &str) {
        if self.config.source == Source::NeetCode {
            let Some(entry) = roadmap::entry_index(slug) else { return; };
            self.explorer_topic = TOPICS.iter().position(|topic| topic.name == ENTRIES[entry].topic).unwrap_or(0);
            self.rebuild_rows();
            if let Some(index) = self.rows.iter().position(|row| *row == Row::Problem(entry)) { self.select_row(index, ScrollStrategy::Center); }
        } else if let Some(index) = self.active_catalog().iter().position(|item| item.slug == slug) {
            if let Some(row) = self.rows.iter().position(|row| *row == Row::Catalog(index)) { self.select_row(row, ScrollStrategy::Center); }
        }
    }

    pub fn reveal_topic(&mut self, topic: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.config.source = Source::NeetCode;
        self.explorer_topic = TOPICS.iter().position(|entry| entry.name == topic).unwrap_or(0);
        self.rebuild_rows(); self.sidebar_sel = 0;
        self.left = true; self.home.sidebar = true; self.zen = false; self.history_mode = false;
        if self.session.is_none() { self.center = Center::Home; } else { self.center = Center::Editor; }
        self.focus_nav(Focus::Explorer, window, cx);
    }

    pub fn sidebar_move(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.history_mode {
            let len = self.versions().len();
            if let Some(s) = &mut self.session {
                if len > 0 {
                    s.selected_commit = (s.selected_commit as isize + delta).clamp(0, len as isize - 1) as usize;
                }
            }
        } else if !self.rows.is_empty() {
            let i = (self.sidebar_sel as isize + delta).clamp(0, self.rows.len() as isize - 1) as usize;
            self.select_row(i, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    pub fn sidebar_expand(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !open && self.config.source == Source::NeetCode { self.focus_nav(Focus::Explorer, window, cx); }
        else if open { self.sidebar_activate(window, cx); }
    }

    pub fn sidebar_activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let slug = match self.rows.get(self.sidebar_sel).copied() {
            Some(Row::Problem(index)) => Some(ENTRIES[index].slug.clone()),
            Some(Row::Catalog(index)) => self.active_catalog().get(index).map(|item| item.slug.clone()),
            None => None,
        };
        if let Some(slug) = slug { self.open_problem(slug, window, cx); }
    }

    // ---- views -------------------------------------------------------------------------

    pub fn show_roadmap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.config.source = Source::NeetCode;
        self.save_config(window, cx);
        self.rebuild_rows();
        self.omni.stale = true;
        self.center = Center::Roadmap;
        self.roadmap.details = false;
        if let Some(e) = self.session.as_ref().and_then(|s| roadmap::entry(&s.slug)) {
            self.roadmap_sel = TOPICS.iter().position(|t| t.name == e.topic).unwrap_or(0);
        }
        self.focus_nav(Focus::Roadmap, window, cx);
        self.flash("Roadmap", cx);
    }

    pub fn focus_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_area = Focus::Editor;
        let handle = self.editor.read(cx).focus_handle(cx);
        handle.focus(window, cx);
        cx.notify();
    }

    pub fn focus_nav(&mut self, area: Focus, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_area = area;
        self.nav_focus.focus(window, cx);
        cx.notify();
    }

    /// Leaves the roadmap or settings for the editor.
    pub fn back_to_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.is_none() {
            self.show_home(window, cx);
            return;
        }
        self.center = Center::Editor;
        self.history_mode = false;
        self.settings.editing = None;
        self.attach_language_server(window, cx);
        self.focus_editor(window, cx);
    }

    // ---- problem session ---------------------------------------------------------------

    pub fn open_problem(&mut self, slug: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.as_ref().is_some_and(|session| session.slug == slug) {
            self.back_to_editor(window, cx);
            return;
        }
        if let Some(index) = self.tabs.iter().position(|tab| tab.as_ref().is_some_and(|tab| tab.session.slug == slug)) {
            self.select_tab(index, window, cx);
            return;
        }
        self.save_now(cx);
        self.park_tab();
        self.new_editor(window, cx);
        self.active_tab = Some(self.tabs.len());
        self.tabs.push(None);
        self.center = Center::Editor;
        self.focus_editor(window, cx);
        let item = self.item(&slug).cloned();
        let title = item
            .as_ref()
            .map(|i| i.title.clone())
            .or_else(|| roadmap::entry(&slug).map(|e| e.title.clone()))
            .unwrap_or_else(|| slug.clone());
        let frontend_id = item.as_ref().map_or(0, |i| i.frontend_id);
        let language = self.config.preferred_language;
        let rel = ws::solution_rel(frontend_id, &slug, language);
        let path = self.config.workspace.join(&rel);
        let _ = self.db.set("last", &slug);
        self.push_recent(&slug);
        self.reveal_in_sidebar(&slug);
        self.session = Some(Session {
            slug: slug.clone(),
            title,
            frontend_id,
            question: None,
            language,
            source: if slug.starts_with("cf:") { Source::Codeforces } else if self.config.source == Source::Codeforces { Source::LeetCode } else { self.config.source },
            rel,
            path,
            disk_mtime: None,
            cases: vec![],
            results: vec![],
            compile_error: None,
            selected_case: 0,
            running: false,
            run_id: 0,
            judge: None,
            hints_shown: 0,
            history: vec![],
            selected_commit: 0,
            remote_versions: vec![],
            history_status: None,
            history_loading: false,
            history_loaded: false,
            version_sequence: 0,
            history_sequence: 0,
        });
        self.statement.update(cx, |s, cx| {
            *s = Statement { slug: slug.clone().into(), status: Some("Loading…".into()), ..Default::default() };
            cx.notify();
        });
        self.editor.update(cx, |e, cx| e.set_value("", window, cx));
        cx.notify();

        let (db, client) = (self.db.clone(), self.client.clone());
        let workspace = self.config.workspace.clone();
        cx.spawn_in(window, async move |this, cx| {
            let fetch_slug = slug.clone();
            let loaded = cx
                .background_spawn(async move {
                    load_question(&db, &client, &workspace, &fetch_slug, language)
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.finish_loading(&slug, loaded, window, cx);
            });
        })
        .detach();
    }

    pub fn switch_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        if self.language_picker.loading { return; }
        let Some(session) = self.session.as_ref() else {
            self.config.preferred_language = language;
            self.activate_language(language, cx);
            self.save_config(window, cx);
            crate::language_picker::close(self, window, cx);
            return;
        };
        if session.language == language { crate::language_picker::close(self, window, cx); return; }
        if session.question.is_none() || session.running || matches!(session.judge, Some(Judge::Running { .. })) { return; }
        let slug = session.slug.clone();
        let previous = session.language;
        let tab = self.active_tab;
        let editor = self.editor.clone();
        self.save_now(cx);
        if self.session.as_ref().is_some_and(|session| std::fs::read_to_string(&session.path).ok().as_deref() != Some(self.editor.read(cx).value().as_ref())) {
            self.toast(Notification::error("Solution could not be saved; language kept"), window, cx);
            return;
        }
        self.language_picker.loading = true;
        let (db, client, directory) = (self.db.clone(), self.client.clone(), self.config.workspace.clone());
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let fetch_slug = slug.clone();
            let loaded = cx.background_spawn(async move { load_question(&db, &client, &directory, &fetch_slug, language) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.language_picker.loading = false;
                if this.active_tab != tab || this.editor != editor || !this.session.as_ref().is_some_and(|session| session.slug == slug && session.language == previous && !session.running && !matches!(session.judge, Some(Judge::Running { .. }))) { cx.notify(); return; }
                match loaded {
                    Ok(loaded) => {
                        this.save_now(cx);
                        if this.session.as_ref().is_some_and(|session| std::fs::read_to_string(&session.path).ok().as_deref() != Some(this.editor.read(cx).value().as_ref())) {
                            this.toast(Notification::error("Solution could not be saved; language kept"), window, cx);
                            return;
                        }
                        this.config.preferred_language = language;
                        this.save_config(window, cx);
                        if let Some(session) = this.session.as_mut() { session.language = language; session.compile_error = None; session.judge = None; }
                        this.new_editor(window, cx);
                        this.apply_loaded(loaded, window, cx);
                        this.attach_language_server(window, cx);
                        crate::language_picker::close(this, window, cx);
                        this.focus_editor(window, cx);
                    }
                    Err(error) => this.toast(Notification::error(format!("Language unavailable: {error}")), window, cx),
                }
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn apply_loaded(&mut self, l: Loaded, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.session.as_mut() else { return };
        let q = l.q;
        let mut cases: Vec<Case> = q
            .examples
            .iter()
            .enumerate()
            .map(|(i, input)| Case { id: i, input: input.clone(), expected: q.outputs.get(i).cloned(), custom: false })
            .collect();
        let n = cases.len();
        cases.extend(l.custom.into_iter().enumerate().map(|(i, t)| Case {
            id: n + i,
            input: t.input,
            expected: (!t.expected.trim().is_empty()).then_some(t.expected),
            custom: true,
        }));
        if let Some(saved) = l.saved_cases { cases = saved; }
        s.results = vec![None; cases.len()];
        s.cases = cases;
        s.frontend_id = q.frontend_id.parse().unwrap_or(0);
        s.title = q.title.clone();
        s.rel = l.rel;
        s.path = l.path;
        s.disk_mtime = l.mtime;
        s.history = l.history;
        let video = roadmap::entry(&q.slug).filter(|e| !e.video.is_empty()).map(|e| format!("https://www.youtube.com/watch?v={}", e.video));
        let statement = Statement {
            slug: q.slug.clone().into(),
            title: q.title.clone().into(),
            blocks: l.statement,
            language: s.language,
            db: Some(self.db.clone()),
            topics: q.topics.iter().map(|t| t.clone().into()).collect(),
            hints: l.hints.into_iter().map(Into::into).collect(),
            hints_shown: 0,
            video,
            status: None,
            ..Default::default()
        };
        s.question = Some(q);
        self.statement.update(cx, |s, cx| {
            *s = statement;
            cx.notify();
        });
        self.editor.update(cx, |e, cx| e.set_value(l.code, window, cx));
        if self.focus_area == Focus::Editor {
            self.focus_editor(window, cx);
        }
    }

    pub(crate) fn apply_imported_question(&mut self, q: Question, window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let Some(session) = self.session.as_ref().filter(|session| session.slug == q.slug && session.question.is_none()) else { return Ok(()); };
        let rel = session.rel.clone();
        let starter = q.starter(session.language).ok_or_else(|| anyhow::anyhow!("Imported question has no starter"))?;
        let path = ws::ensure_solution(&self.config.workspace, &rel, starter)?;
        let code = std::fs::read_to_string(&path)?;
        let mtime = std::fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        let statement = practice::description::parse(&q.content);
        let saved_cases = self.db.test_cases(&q.slug)?;
        self.apply_loaded(Loaded { q, rel, path, code, mtime, custom: vec![], saved_cases,
            history: vec![], statement, hints: vec![] }, window, cx);
        self.attach_language_server(window, cx);
        Ok(())
    }

    pub(crate) fn schedule_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(350)).await;
            let _ = this.update(cx, |this, cx| this.save_now(cx));
        });
    }

    /// Writes the editor to the Solution file when it differs.
    pub fn save_now(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.session.as_mut() else { return };
        if s.question.is_none() {
            return;
        }
        let text = self.editor.read(cx).value().to_string();
        if std::fs::read_to_string(&s.path).ok().as_deref() == Some(text.as_str()) {
            return;
        }
        if std::fs::write(&s.path, &text).is_ok() {
            s.disk_mtime = std::fs::metadata(&s.path).and_then(|m| m.modified()).ok();
            self.language_document_saved();
        }
    }

    /// Reloads the Solution when another editor changes it on disk.
    fn watch_disk(&self, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(800)).await;
                let alive = this.update_in(cx, |this, window, cx| {
                    let Some(s) = this.session.as_mut() else { return };
                    let mtime = std::fs::metadata(&s.path).and_then(|m| m.modified()).ok();
                    if mtime.is_none() || mtime == s.disk_mtime {
                        return;
                    }
                    s.disk_mtime = mtime;
                    if let Ok(text) = std::fs::read_to_string(&s.path) {
                        if this.editor.read(cx).value().as_ref() != text {
                            this.editor.update(cx, |e, cx| e.set_value(text, window, cx));
                            this.flash("Reloaded from disk", cx);
                        }
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
    }

    // ---- runs --------------------------------------------------------------------------

    pub fn run_tests(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !crate::case_editor::save(self, window, cx) { return; }
        self.save_now(cx);
        if self.session.as_ref().is_some_and(|session| session.language != Language::Python && !session.slug.starts_with("cf:")) {
            self.judge(false, window, cx);
            return;
        }
        let Some(s) = self.session.as_mut() else { return };
        let Some(q) = s.question.as_ref() else { return };
        if s.running || s.cases.is_empty() {
            return;
        }
        s.running = true;
        s.run_id += 1;
        s.compile_error = None;
        s.results = vec![None; s.cases.len()];
        self.bottom = true;
        let language = s.language;
        let is_codeforces = s.slug.starts_with("cf:");
        let (python, path, meta, cases) = (self.config.python.clone(), s.path.clone(), q.meta.clone(), s.cases.clone());
        let compare = Compare::for_statement(&q.content);
        let timeout = Duration::from_secs(self.config.test_timeout_secs);
        let slug = s.slug.clone();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let outcome = cx
                .background_spawn(async move {
                    let mut results = vec![];
                    let compile = if is_codeforces { practice::stdin_runner::run(language, &python, &path, &cases, timeout, |r| results.push(r)) } else { runner::run(&python, &path, &meta, &cases, compare, timeout, |r| results.push(r)) };
                    (compile, results)
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(s) = this.session_for_mut(&slug) else { return };
                s.running = false;
                let (compile, results) = outcome;
                match compile {
                    Err(err) => s.compile_error = Some(format!("Could not run solution: {err}")),
                    Ok(Some(err)) => s.compile_error = Some(err),
                    Ok(None) => {}
                }
                for r in results {
                    if let Some(i) = s.cases.iter().position(|c| c.id == r.id) {
                        s.results[i] = Some(r);
                    }
                }
                let total = s.cases.len();
                let passed = s.results.iter().flatten().filter(|r| r.verdict == Verdict::Pass).count();
                if let Some(first_bad) = s
                    .results
                    .iter()
                    .position(|r| r.as_ref().is_some_and(|r| !matches!(r.verdict, Verdict::Pass | Verdict::Ran)))
                {
                    s.selected_case = first_bad;
                }
                let note = if s.compile_error.is_some() {
                    Notification::error("Syntax error")
                } else if passed == total {
                    Notification::success(format!("All {total} tests passed · ctrl+alt+enter to submit"))
                } else {
                    Notification::warning(format!("{passed}/{total} tests passed"))
                };
                this.toast(note, window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn judge(&mut self, submission: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !crate::case_editor::save(self, window, cx) { return; }
        self.save_now(cx);
        if let Some(session) = self.session.as_ref().filter(|session| session.slug.starts_with("cf:")) {
            if submission {
                if let Ok((contest, index)) = practice::codeforces::problem_id(&session.slug) {
                    cx.write_to_clipboard(ClipboardItem::new_string(self.editor.read(cx).value().to_string()));
                    let _ = open::that_detached(format!("https://codeforces.com/contest/{contest}/submit/{index}"));
                    self.toast(Notification::info("Solution copied · submit in Codeforces"), window, cx);
                }
            } else { self.run_tests(window, cx); }
            return;
        }
        if !self.client.signed_in() {
            self.toast(
                Notification::warning("LeetCode sign-in needed. Open Settings → LeetCode account."),
                window,
                cx,
            );
            return;
        }
        let Some(s) = self.session.as_mut() else { return };
        let Some(q) = s.question.clone() else { return };
        if matches!(s.judge, Some(Judge::Running { .. })) {
            return;
        }
        s.judge = Some(Judge::Running { submission });
        self.bottom = true;
        let language = s.language;
        let code = self.editor.read(cx).value().to_string();
        let inputs: Vec<String> = s.cases.iter().map(|c| c.input.clone()).collect();
        let (client, db) = (self.client.clone(), self.db.clone());
        let (workspace, rel, slug) = (self.config.workspace.clone(), s.rel.clone(), s.slug.clone());
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let result = if submission { client.submit(&q, &code, language)? } else { client.run(&q, &code, &inputs, language)? };
                    let mut commit = None;
                    if submission && result.accepted() {
                        db.set_solved(&q.slug, true)?;
                        let message = format!("{}: Accepted · {} · {}", q.title, result.runtime, result.memory);
                        commit = git::commit(&workspace, &[rel.as_path()], &message)?;
                    }
                    let history = crate::history::local_versions(&workspace, q.frontend_id.parse().unwrap_or(0), &q.slug);
                    anyhow::Ok((result, commit, history))
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if matches!(&result, Ok((r, _, _)) if r.submission && r.accepted()) && this.solved.insert(slug.clone()) {
                    this.rebuild_library();
                }
                let Some(s) = this.session_for_mut(&slug) else { return };
                let note = match result {
                    Ok((r, commit, history)) => {
                        s.history = history;
                        let note = judge_note(&r, commit.is_some());
                        s.judge = Some(Judge::Done(r));
                        note
                    }
                    Err(err) => {
                        let note = Notification::error(err.to_string());
                        s.judge = Some(Judge::Failed(err.to_string()));
                        note
                    }
                };
                this.toast(note, window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn step_case(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(s) = &mut self.session {
            if !s.cases.is_empty() {
                s.selected_case = (s.selected_case as isize + delta).rem_euclid(s.cases.len() as isize) as usize;
                self.bottom = true;
                cx.notify();
            }
        }
    }

    pub fn step_problem(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let slugs: Vec<String> = self.rows.iter().filter_map(|row| match *row {
            Row::Problem(index) => Some(ENTRIES[index].slug.clone()),
            Row::Catalog(index) => self.active_catalog().get(index).map(|item| item.slug.clone()),
        }).collect();
        if slugs.is_empty() { return; }
        let current = self.session.as_ref().and_then(|session| slugs.iter().position(|slug| slug == &session.slug));
        let next = current.map_or(0, |index| (index as isize + delta).rem_euclid(slugs.len() as isize) as usize);
        self.center = Center::Editor;
        self.open_problem(slugs[next].clone(), window, cx);
    }

    pub fn save_test_case(&mut self, index: Option<usize>, input: String, expected: String, cx: &mut Context<Self>) -> bool {
        let Some(session) = self.session.as_ref() else { return false; };
        if session.running || matches!(session.judge, Some(Judge::Running { .. })) {
            self.flash("Wait for the current run to finish", cx); return false;
        }
        let mut cases = session.cases.clone();
        let selected = index.unwrap_or(cases.len());
        let expected = (!expected.trim().is_empty()).then_some(expected);
        if let Some(index) = index {
            let Some(case) = cases.get_mut(index) else { return false; };
            case.input = input; case.expected = expected;
        } else { cases.push(Case { id: selected, input, expected, custom: true }); }
        self.replace_test_cases(cases, selected, cx)
    }

    pub fn reset_test_cases(&mut self, cx: &mut Context<Self>) {
        let Some(question) = self.session.as_ref().and_then(|s| s.question.as_ref()) else { return; };
        let cases = question.examples.iter().enumerate().map(|(id, input)| Case {
            id, input: input.clone(), expected: question.outputs.get(id).cloned(), custom: false,
        }).collect();
        self.replace_test_cases(cases, 0, cx);
    }

    fn replace_test_cases(&mut self, mut cases: Vec<Case>, selected: usize, cx: &mut Context<Self>) -> bool {
        let Some(session) = self.session.as_ref() else { return false; };
        if session.running || matches!(session.judge, Some(Judge::Running { .. })) {
            self.flash("Wait for the current run to finish", cx); return false;
        }
        for (id, case) in cases.iter_mut().enumerate() { case.id = id; }
        if let Err(error) = self.db.save_test_cases(&session.slug, &cases) {
            self.flash(format!("Could not save test cases: {error}"), cx); return false;
        }
        let Some(session) = self.session.as_mut() else { return false; };
        session.selected_case = selected.min(cases.len().saturating_sub(1));
        session.results = vec![None; cases.len()]; session.cases = cases;
        session.compile_error = None; session.judge = None;
        self.case_edit = None;
        self.bottom = true;
        cx.notify();
        true
    }

    pub fn reveal_hint(&mut self, cx: &mut Context<Self>) {
        self.right = true;
        let Some(s) = &mut self.session else { return };
        let total = s.question.as_ref().map_or(0, |q| q.hints.len());
        if total == 0 { self.flash("No hints", cx); return; }
        s.hints_shown = if s.hints_shown >= total { 0 } else { s.hints_shown + 1 };
        let shown = s.hints_shown;
        self.statement.update(cx, |st, cx| {
            st.hints_shown = shown;
            st.reference_open = false;
            cx.notify();
        });
        self.flash(if shown == 0 { "Hints hidden".into() } else { format!("Hint {shown}/{total}") }, cx);
    }

    pub fn hide_hints(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &mut self.session { session.hints_shown = 0; }
        self.statement.update(cx, |statement, cx| { statement.hints_shown = 0; cx.notify(); });
        cx.notify();
    }

    // ---- prompts -----------------------------------------------------------------------

    pub fn prompt(&mut self, style: Style, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.session.as_ref() else { return };
        let Some(q) = s.question.as_ref() else { return };
        let code = self.editor.read(cx).value().to_string();
        let failures = s.failures();
        let url = crate::roadmap::problem_url(s.source, &q.slug);
        let text = prompts::build_custom(
            style,
            &prompts::Context {
                title: &q.title,
                difficulty: &q.difficulty,
                url: &url,
                statement_html: &q.content,
                code: &code,
                starter: q.starter(s.language).unwrap_or_default(),
                language: s.language,
                failures: &failures,
            },
            self.config.prompt_instructions.get(style.id()).map(String::as_str).unwrap_or(style.instructions()),
        );
        let provider = self.config.prompt_provider;
        let link = prompts::url(provider, &text);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        match open::that_detached(&link) {
            Ok(()) => self.toast(
                Notification::info(format!("{} · {} opened (prompt also copied)", style.label(), provider.label())),
                window,
                cx,
            ),
            Err(err) => self.toast(Notification::error(format!("Could not open browser: {err}")), window, cx),
        }
    }

    // ---- history -----------------------------------------------------------------------

    pub fn restore_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = &self.session { self.restore_version(session.selected_commit, window, cx); }
    }

    // ---- roadmap -----------------------------------------------------------------------

    /// Moves the roadmap selection to the nearest topic in the pressed direction.
    pub fn roadmap_step(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        let cur = &TOPICS[self.roadmap_sel];
        let (x0, y0) = (cur.col, cur.row as f32);
        let best = TOPICS
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.roadmap_sel)
            .filter_map(|(i, t)| {
                let (vx, vy) = (t.col - x0, t.row as f32 - y0);
                let along = vx * dx + vy * dy;
                if along <= 0.01 {
                    return None;
                }
                let across = (vx * dy - vy * dx).abs();
                Some((along + across * 2.5, i))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, i)) = best {
            self.roadmap_sel = i;
            self.roadmap.selected = 0;
            self.roadmap.scroll.scroll_to_item(0, ScrollStrategy::Top);
            cx.notify();
        }
    }

    pub fn roadmap_open_topic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.roadmap.details = true;
        self.roadmap.selected = 0;
        self.roadmap.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.focus_nav(Focus::RoadmapTopic, window, cx);
    }
}

fn load_question(db: &Db, client: &Client, workspace: &std::path::Path, slug: &str, language: Language) -> anyhow::Result<Loaded> {
    let mut q = match db.question(slug)? {
        Some(q) => q,
        None => {
            let q = if slug.starts_with("cf:") { practice::codeforces::cached_question(&db, slug)? } else { client.question(slug)? };
            db.save_question(&q)?;
            q
        }
    };
    if q.starter(language).is_none() {
        q = if slug.starts_with("cf:") { practice::codeforces::question(slug)? } else { client.question(slug)? };
        db.save_question(&q)?;
    }
    let frontend_id = q.frontend_id.parse().unwrap_or(0);
    let rel = ws::solution_rel(frontend_id, &q.slug, language);
    let starter = q.starter(language).ok_or_else(|| anyhow::anyhow!("{} has no {} starter", q.title, language.label()))?;
    let path = ws::ensure_solution(workspace, &rel, starter)?;
    let code = std::fs::read_to_string(&path)?;
    let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    let custom = db.custom_tests(&q.slug)?;
    let saved_cases = db.test_cases(&q.slug)?;
    let history = crate::history::local_versions(workspace, q.frontend_id.parse().unwrap_or(0), &q.slug);
    let statement = if q.meta["statementMarkdown"].as_bool() == Some(true) { vec![practice::description::Block::Markdown(q.content.clone())] } else { practice::description::parse(&q.content) };
    let hints: Vec<String> = q.hints.iter().map(|h| prompts::statement_markdown(h)).collect();
    anyhow::Ok(Loaded { q, rel, path, code, mtime, custom, saved_cases, history, statement, hints })
}

pub(crate) struct Loaded {
    q: Question,
    rel: PathBuf,
    path: PathBuf,
    code: String,
    mtime: Option<SystemTime>,
    custom: Vec<practice::db::CustomTest>,
    saved_cases: Option<Vec<Case>>,
    history: Vec<crate::history::LocalVersion>,
    statement: Vec<practice::description::Block>,
    hints: Vec<String>,
}

fn judge_note(r: &JudgeResult, committed: bool) -> Notification {
    let pct = |p: Option<f64>| p.map(|p| format!(" (beats {p:.0}%)")).unwrap_or_default();
    if r.submission && r.accepted() {
        let saved = if committed { " · saved to history" } else { "" };
        Notification::success(format!(
            "Accepted · {}{} · {}{}{saved}",
            r.runtime,
            pct(r.runtime_percentile),
            r.memory,
            pct(r.memory_percentile)
        ))
        .title("LeetCode")
    } else if r.accepted() {
        Notification::success(format!("Judge: all {} cases passed", r.answers.len().max(1))).title("LeetCode")
    } else {
        let counts = match (r.total_correct, r.total_testcases) {
            (Some(c), Some(t)) => format!(" · {c}/{t}"),
            _ => String::new(),
        };
        let status = if r.status_code == 10 { "Wrong Answer" } else { r.status.as_str() };
        Notification::error(format!("{status}{counts}")).title("LeetCode")
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
