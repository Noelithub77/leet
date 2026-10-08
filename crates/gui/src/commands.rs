//! Wires every action to the workspace. Global actions live on the root; arrow/enter/escape
//! navigation only on the sidebar, roadmap, and settings (the `VgNav` context).

use std::sync::Arc;

use gpui_kit::component::notification::Notification;
use gpui_kit::*;
use practice::creds::{self, Account};
use practice::leetcode::Client;

use crate::actions::*;
use practice::assist::Action;
use crate::omnibar::Scope;
use crate::workspace::{Center, Focus, Workspace};

impl Workspace {
    fn zoom(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.config.zoom = crate::theme::set_zoom(self.config.zoom + delta, cx);
        self.save_config(window, cx);
        let percent = (self.config.zoom * 100.).round();
        self.flash(format!("Zoom {percent}%"), cx);
    }

    pub fn register(div: Div, cx: &mut Context<Self>) -> Div {
        div.key_context(WORKSPACE)
            .on_action(cx.listener(|this, _: &ShowHome, window, cx| this.show_home(window, cx)))
            .on_action(cx.listener(|this, _: &GuidedTour, window, cx| this.start_tour(window, cx)))
            .on_action(cx.listener(|this, _: &CycleTabs, window, cx| this.cycle_tab(1, window, cx)))
            .on_action(cx.listener(|this, _: &PreviousTab, window, cx| this.cycle_tab(-1, window, cx)))
            .on_action(cx.listener(|this, _: &CloseProblem, window, cx| {
                if this.center == Center::Editor { this.close_problem(window, cx); }
            }))
            .on_action(cx.listener(|this, _: &ToggleLeft, window, cx| {
                if this.center == Center::Home {
                    this.home.sidebar = !this.home.sidebar;
                    this.zen = false;
                    this.focus_nav(if this.home.sidebar { if this.config.source == practice::language::Source::NeetCode { Focus::Explorer } else { Focus::Sidebar } } else { Focus::Home }, window, cx);
                    return;
                }
                this.left = !this.left;
                this.zen = false;
                if this.left {
                    this.focus_nav(if this.config.source == practice::language::Source::NeetCode { Focus::Explorer } else { Focus::Sidebar }, window, cx);
                } else {
                    this.focus_editor(window, cx);
                }
                this.save_layout();
                this.flash(if this.left { "Explorer" } else { "Explorer hidden" }, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTags, _, cx| {
                this.config.show_tags = !this.config.show_tags;
                if let Err(error) = this.config.save() { eprintln!("leet: save tag preference: {error}"); }
                cx.set_global(crate::statement::TagsVisible(this.config.show_tags));
                cx.refresh_windows();
            }))
            .on_action(cx.listener(|this, _: &ToggleReference, window, cx| {
                if this.session.is_none() { return; }
                this.back_to_editor(window, cx); this.description = true;
                this.statement.update(cx, |statement, cx| statement.toggle_reference(window, cx));
            }))
            .on_action(cx.listener(|this, _: &ToggleRight, window, cx| {
                this.right = !this.right;
                this.zen = false;
                if !this.right { this.focus_editor(window, cx); }
                this.save_layout();
                this.flash(if this.right { "AI chat" } else { "AI chat hidden" }, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleDescription, window, cx| {
                this.description = !this.description;
                this.zen = false;
                if !this.description { this.focus_editor(window, cx); }
                this.save_layout();
                this.flash(if this.description { "Description" } else { "Description hidden" }, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleBottom, _, cx| {
                this.bottom = !this.bottom;
                this.zen = false;
                this.save_layout();
                this.flash(if this.bottom { "Results" } else { "Results hidden" }, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleHistory, window, cx| {
                this.history_mode = !this.history_mode;
                if this.history_mode {
                    this.description = true;
                    this.zen = false;
                    this.center = Center::Editor;
                    if let Some(s) = &mut this.session {
                        s.selected_commit = 0;
                    }
                    if this.session.as_ref().is_some_and(|session| !session.history_loaded) { this.load_remote_versions(window, cx); }
                    this.focus_nav(Focus::History, window, cx);
                } else {
                    this.focus_editor(window, cx);
                }
                this.flash(if this.history_mode { "Solution history" } else { "Roadmap" }, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleZen, _, cx| {
                this.zen = !this.zen;
                this.flash(if this.zen { "Zen" } else { "Zen off" }, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleRoadmap, window, cx| {
                if this.center == Center::Roadmap {
                    this.back_to_editor(window, cx);
                } else {
                    this.show_roadmap(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                if this.center == Center::Settings {
                    this.back_to_editor(window, cx);
                } else {
                    this.open_settings(None, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &crate::settings::NextSettingsTab, _, cx| {
                if this.center == Center::Settings { this.settings_tab_step(1, cx); }
            }))
            .on_action(cx.listener(|this, _: &crate::settings::PreviousSettingsTab, _, cx| {
                if this.center == Center::Settings { this.settings_tab_step(-1, cx); }
            }))
            .on_action(cx.listener(|this, _: &FocusSidebar, window, cx| {
                if this.center == Center::Home { this.home.sidebar = true; }
                this.left = true;
                this.zen = false;
                this.focus_nav(if this.config.source == practice::language::Source::NeetCode { Focus::Explorer } else { Focus::Sidebar }, window, cx);
                this.flash("Explorer", cx);
            }))
            .on_action(cx.listener(|this, _: &FocusStatement, window, cx| {
                if this.session.is_none() { return; }
                this.back_to_editor(window, cx); this.description = true;
                let statement = this.statement.clone(); window.defer(cx, move |window, cx| statement.update(cx, |statement, cx| statement.focus(window, cx)));
            }))
            .on_action(cx.listener(|this, _: &FocusEditor, window, cx| {
                if this.center == Center::Editor { this.focus_editor(window, cx); } else { this.back_to_editor(window, cx); }
                this.flash("Editor", cx);
            }))
            .on_action(cx.listener(|this, _: &Search, window, cx| this.omni_open(Scope::All, window, cx)))
            .on_action(cx.listener(|this, _: &ShortcutHelp, window, cx| this.omni_open(Scope::Shortcuts, window, cx)))
            .on_action(cx.listener(|this, _: &ShowContests, window, cx| {
                this.show_home(window, cx);
                this.select_home_list(crate::home::HomeList::Contests, window, cx);
            }))
            .on_action(cx.listener(|this, _: &TogglePastContests, window, cx| {
                this.show_home(window, cx);
                this.select_home_list(crate::home::HomeList::Contests, window, cx);
                this.contests.past_open = !this.contests.past_open; cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FindProblem, window, cx| this.omni_open(Scope::Problems, window, cx)))
            .on_action(cx.listener(|this, _: &PickTheme, window, cx| this.omni_open(Scope::Themes, window, cx)))
            .on_action(cx.listener(|this, _: &AddCustomTest, window, cx| crate::dialogs::open_custom_test(this, window, cx)))
            .on_action(cx.listener(|this, _: &EditTestCase, window, cx| crate::case_editor::begin(this, crate::case_editor::Field::Input, window, cx)))
            .on_action(cx.listener(|this, _: &ResetTestCases, _, cx| this.reset_test_cases(cx)))
            .on_action(cx.listener(|this, _: &CycleList, window, cx| {
                this.config.roadmap_list = this.config.roadmap_list.next();
                this.save_config(window, cx);
                this.rebuild_library();
                let label = this.config.roadmap_list.label();
                this.flash(label, cx);
            }))
            .on_action(cx.listener(|this, _: &RunTests, window, cx| {
                this.flash("Run tests", cx);
                this.run_tests(window, cx);
            }))
            .on_action(cx.listener(|this, _: &JudgeRun, window, cx| {
                this.flash("Run on LeetCode", cx);
                this.judge(false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Submit, window, cx| {
                this.flash("Submit", cx);
                this.judge(true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextProblem, window, cx| this.step_problem(1, window, cx)))
            .on_action(cx.listener(|this, _: &PrevProblem, window, cx| this.step_problem(-1, window, cx)))
            .on_action(cx.listener(|this, _: &NextCase, _, cx| this.step_case(1, cx)))
            .on_action(cx.listener(|this, _: &PrevCase, _, cx| this.step_case(-1, cx)))
            .on_action(cx.listener(|this, _: &AssistHints, window, cx| this.run_assist(Action::Hints, window, cx)))
            .on_action(cx.listener(|this, _: &AssistStuck, window, cx| this.run_assist(Action::Stuck, window, cx)))
            .on_action(cx.listener(|this, _: &AssistBugs, window, cx| this.run_assist(Action::Bugs, window, cx)))
            .on_action(cx.listener(|this, _: &AssistTests, window, cx| this.run_assist(Action::Tests, window, cx)))
            .on_action(cx.listener(|this, _: &AssistAnalyze, window, cx| this.run_assist(Action::Analyze, window, cx)))
            .on_action(cx.listener(|this, _: &AssistOptimize, window, cx| this.run_assist(Action::Optimize, window, cx)))
            .on_action(cx.listener(|this, _: &AssistVisualize, window, cx| this.run_assist(Action::Visualize, window, cx)))
            .on_action(cx.listener(|this, _: &AssistDryRun, window, cx| this.run_assist(Action::DryRun, window, cx)))
            .on_action(cx.listener(|this, _: &AssistPattern, window, cx| this.run_assist(Action::Pattern, window, cx)))
            .on_action(cx.listener(|this, _: &AssistExplain, window, cx| this.run_assist(Action::Explain, window, cx)))
            .on_action(cx.listener(|this, _: &AssistSolve, window, cx| this.run_assist(Action::Solve, window, cx)))
            .on_action(cx.listener(|this, _: &StopAssist, _, cx| { this.assist.update(cx, |assist, cx| assist.stop_all(cx)); this.flash("AI stopped", cx); }))
            .on_action(cx.listener(|this, _: &ToggleDebug, window, cx| {
                if this.center != Center::Editor { this.back_to_editor(window, cx); }
                let on = !this.debug_mode;
                this.set_debug(on, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ConfigureAi, window, cx| this.ai_chip.update(cx, |chip, cx| chip.open(window, cx))))
            .on_action(cx.listener(|this, _: &CycleAgent, window, cx| {
                let agents: Vec<Option<practice::agents::AgentKind>> = this.assist.read(cx).agents.iter().map(|a| Some(a.kind)).chain([None]).collect();
                let current = match this.assist.read(cx).target(&this.config) { crate::assist::Target::Agent(agent, _) => Some(agent.kind), crate::assist::Target::Web(_) => None };
                let next = agents.iter().position(|a| *a == current).map_or(0, |i| (i + 1) % agents.len());
                this.config.agent = agents[next];
                this.config.ai_web = this.config.agent.is_none();
                this.save_config(window, cx);
                this.flash(format!("AI: {}", agents[next].map_or("Web chat", practice::agents::AgentKind::label)), cx);
                this.ai_chip.update(cx, |_, cx| cx.notify());
            }))
            .on_action(cx.listener(|this, _: &OpenExternal, window, cx| {
                this.save_now(cx);
                let Some(s) = &this.session else { return };
                let editor = this.config.external_editor.clone();
                match std::process::Command::new(&editor).arg(&s.path).spawn() {
                    Ok(_) => this.flash(format!("Opened in {editor}"), cx),
                    Err(err) => this.toast(Notification::error(format!("{editor}: {err}")), window, cx),
                }
            }))
            .on_action(cx.listener(|this, _: &OpenInBrowser, window, cx| {
                let Some(s) = &this.session else { return };
                let url = crate::roadmap::problem_url(s.source, &s.slug);
                match open::that_detached(url) {
                    Ok(()) => this.flash("Opened problem source", cx),
                    Err(err) => this.toast(Notification::error(err.to_string()), window, cx),
                }
            }))
            .on_action(cx.listener(|this, _: &CycleSource, window, cx| this.choose_source(this.config.source.next(), window, cx)))
            .on_action(cx.listener(|this, _: &RefreshCatalog, window, cx| {
                this.client = Arc::new(Client::new(creds::load(Account::LeetCode).ok()));
                this.flash("Refreshing…", cx);
                if this.config.source == practice::language::Source::Codeforces { this.refresh_codeforces(true, window, cx); return; }
                this.refresh_catalog(true, window, cx);
                if this.account_names[1] != "Signed out" { this.refresh_neetcode(window, cx); }
            }))
            .on_action(cx.listener(|this, _: &MarkNeetCode, window, cx| this.mark_neetcode(window, cx)))
            .on_action(cx.listener(|this, _: &RevealHint, _, cx| this.reveal_hint(cx)))
            .on_action(cx.listener(|this, _: &HideHints, _, cx| this.hide_hints(cx)))
            .on_action(cx.listener(|this, _: &ResetSolution, window, cx| {
                let Some(q) = this.session.as_ref().and_then(|s| s.question.as_ref()) else { return };
                let language = this.session.as_ref().map_or(practice::language::Language::Python, |session| session.language);
                let Some(starter) = q.starter(language) else { return; };
                let starter = format!("{}\n", starter.trim_end());
                this.editor.update(cx, |e, cx| e.replace_all(starter, window, cx));
                this.toast(Notification::info("Reset to starter code · ctrl+z undoes"), window, cx);
            }))
            .on_action(cx.listener(|this, _: &ZoomIn, window, cx| this.zoom(0.1, window, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, window, cx| this.zoom(-0.1, window, cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, window, cx| {
                this.config.zoom = 1.0;
                this.zoom(0.0, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Restart, _, cx| {
                crate::update::restart(this, cx);
            }))
            .on_action(cx.listener(|this, _: &Quit, _, cx| {
                this.save_now(cx);
                cx.quit();
            }))
    }

    pub fn register_nav(div: Div, cx: &mut Context<Self>) -> Div {
        div.key_context(NAV)
            .on_action(cx.listener(|this, _: &Up, _, cx| match this.focus_area {
                Focus::Home => this.home_move(-1, cx),
                Focus::Explorer => this.explorer_move(-crate::sources::TOPIC_COLUMNS, cx),
                Focus::Roadmap => this.roadmap_step(0.0, -1.0, cx),
                Focus::RoadmapTopic => this.roadmap_move_problem(-1, cx),
                Focus::Settings => this.settings_move(-1, cx),
                _ => this.sidebar_move(-1, cx),
            }))
            .on_action(cx.listener(|this, _: &Down, _, cx| match this.focus_area {
                Focus::Home => this.home_move(1, cx),
                Focus::Explorer => this.explorer_move(crate::sources::TOPIC_COLUMNS, cx),
                Focus::Roadmap => this.roadmap_step(0.0, 1.0, cx),
                Focus::RoadmapTopic => this.roadmap_move_problem(1, cx),
                Focus::Settings => this.settings_move(1, cx),
                _ => this.sidebar_move(1, cx),
            }))
            .on_action(cx.listener(|this, _: &Left, window, cx| match this.focus_area {
                Focus::Home => this.select_home_list(crate::home::HomeList::Recent, window, cx),
                Focus::Explorer => this.explorer_move(-1, cx),
                Focus::Roadmap => this.roadmap_step(-1.0, 0.0, cx),
                Focus::RoadmapTopic => this.focus_nav(Focus::Roadmap, window, cx),
                Focus::Settings => this.settings_cycle(-1, window, cx),
                Focus::History => {}
                _ => this.sidebar_expand(false, window, cx),
            }))
            .on_action(cx.listener(|this, _: &Right, window, cx| match this.focus_area {
                Focus::Home => this.select_home_list(crate::home::HomeList::Contests, window, cx),
                Focus::Explorer => this.explorer_move(1, cx),
                Focus::Roadmap if this.roadmap.details => this.focus_nav(Focus::RoadmapTopic, window, cx),
                Focus::Roadmap => this.roadmap_step(1.0, 0.0, cx),
                Focus::RoadmapTopic => {},
                Focus::Settings => this.settings_cycle(1, window, cx),
                Focus::History => {}
                _ => this.sidebar_expand(true, window, cx),
            }))
            .on_action(cx.listener(|this, _: &Confirm, window, cx| match this.focus_area {
                Focus::Home => this.home_open(window, cx),
                Focus::Explorer => this.sidebar_activate(window, cx),
                Focus::Roadmap => this.roadmap_open_topic(window, cx),
                Focus::RoadmapTopic => this.roadmap_open_problem(window, cx),
                Focus::Settings => this.settings_confirm(window, cx),
                Focus::History => this.restore_commit(window, cx),
                _ => this.sidebar_activate(window, cx),
            }))
            .on_action(cx.listener(|this, _: &Back, window, cx| {
                if this.center == Center::Roadmap && this.roadmap.details {
                    this.roadmap_close_topic(window, cx);
                } else if this.settings.editing.is_some() {
                    this.settings_cancel_edit(window, cx);
                } else if this.center == Center::Settings && this.settings.tab == crate::settings::SettingsTab::Keybindings {
                    this.open_settings(None, window, cx);
                } else {
                    this.back_to_editor(window, cx);
                }
            }))
    }
}
