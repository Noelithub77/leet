#![forbid(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod accounts;
mod companion;
mod companion_service;
mod ai;
mod assist;
mod debug_view;
#[cfg(feature = "gui-test")]
mod gui_test;
#[path = "gen-ui/mod.rs"]
mod gen_ui;
mod brand;
mod assets;
mod actions;
mod commands;
mod dialogs;
mod case_editor;
mod library;
mod roadmap;
mod onboarding;
mod language_server;
mod language_picker;
mod sources;
mod source_menu;
mod home;
mod contests;
mod history;
mod omnibar;
mod settings;
mod snippets;
mod agent_question;
mod statement;
mod rich_text;
mod theme;
mod tour;
mod update;
mod view;
mod workspace;

use gpui_kit::*;
use practice::config::Config;

fn main() {
    if std::env::args().nth(1).as_deref() == Some("snippets") {
        if let Err(error) = practice::snippets::cli::cli(&std::env::args().skip(2).collect::<Vec<_>>()) { eprintln!("leet snippets: {error:#}"); std::process::exit(1); }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--setup-tools") {
        if let Err(error) = practice::tool_setup::cli(&std::env::args().skip(2).collect::<Vec<_>>()) { eprintln!("leet tool setup: {error:#}"); std::process::exit(1); }
        return;
    }
    #[cfg(feature = "gui-test")]
    if std::env::args().nth(1).as_deref() == Some("--gui-test") {
        if let Err(error) = gui_test::run() { eprintln!("leet GUI test: {error:#}"); std::process::exit(1); }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--companion-service") {
        if let Err(error) = companion_service::run() { eprintln!("leet CPH: {error}"); std::process::exit(1); }
        return;
    }
    let mut config = Config::load().unwrap_or_else(|err| {
        eprintln!("leet: {err}; using defaults");
        Config::default()
    });
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("leet {}", env!("LEET_VERSION"));
            return;
        }
        Some("--1337") => {
            println!("1337 · leet\nSolve. Learn. Repeat.\nThere are 10 kinds of people: those who understand binary and those who don't.");
            return;
        }
        Some("--help" | "-h") => {
            println!("leet [WORKSPACE]\n\nNative coding practice. --version identifies the installed build.\n--setup-tools --language python|cpp|c installs private pinned editor tools; add --help for options.");
            return;
        }
        // `leet <dir>` uses that directory as the workspace for this launch.
        Some(dir) => config.workspace = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.into()),
        None => {}
    }

    gpui_kit::application().with_assets(assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        theme::init(&config.theme, config.zoom, &config.font_family, cx);
        let bindings = cx.key_bindings().borrow().bindings().cloned().collect();
        cx.set_global(actions::ComponentBindings(bindings));
        actions::bind_keys(&config, cx);
        omnibar::bind_keys(cx);
        accounts::bind_keys(cx);
        ai::bind_keys(cx);
        assist::bind_keys(cx);
        debug_view::bind_keys(cx);
        tour::bind_keys(cx);
        case_editor::bind_keys(cx);
        onboarding::bind_keys(cx);
        statement::bind_keys(cx);
        snippets::expansion::bind_keys(cx);
        snippets::editor_bind_keys(cx);
        cx.activate(true);

        let options = WindowOptions {
            app_id: Some("leet".into()),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
            window_min_size: Some(size(px(720.), px(480.))),
            titlebar: Some(TitlebarOptions { title: Some("leet".into()), ..Default::default() }),
            ..Default::default()
        };
        let (_, workspace) = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| workspace::Workspace::new(config.clone(), window, cx))
        })
        .expect("open the leet window");
        let _ = workspace;
    });
}
