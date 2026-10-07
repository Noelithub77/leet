#![forbid(unsafe_code)]

mod accounts;
mod companion;
mod ai;
mod brand;
mod assets;
mod actions;
mod commands;
mod dialogs;
mod library;
mod roadmap;
mod onboarding;
mod language_server;
mod sources;
mod home;
mod history;
mod omnibar;
mod settings;
mod statement;
mod theme;
mod update;
mod view;
mod workspace;

use gpui_kit::*;
use practice::config::Config;

fn main() {
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
            println!("leet [WORKSPACE]\n\nNative coding practice. --version identifies the installed build.");
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
        settings::bind_keys(cx);
        ai::bind_keys(cx);
        onboarding::bind_keys(cx);
        statement::bind_keys(cx);
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
