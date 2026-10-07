//! Picks up new development builds: `./ops local:deploy` repoints the `leet` symlink, and the
//! running app offers a restart instead of updating under the user.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::text::TextView;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;

use crate::workspace::Workspace;

/// The command users launch: the installed symlink when present, else this executable.
fn launcher() -> PathBuf {
    if let Some(path) = practice::updates::appimage_path() { return path; }
    let current = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("leet"));
    let link = dirs::executable_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/bin")))
        .map(|d| d.join("leet"));
    match link {
        Some(link) if std::fs::canonicalize(&link).ok().as_ref() == Some(&current) => link,
        _ => current,
    }
}

#[derive(Default)]
pub struct State {
    pub latest: Option<practice::updates::Latest>,
    pub checking: bool,
    pub downloading: bool,
    pub error: Option<String>,
    progress: Arc<AtomicU64>,
    restart_path: Option<PathBuf>,
}

pub fn check(window: &mut Window, cx: &mut Context<Workspace>) -> Task<()> {
    cx.spawn_in(window, async move |this, cx| {
        let _ = this.update(cx, |this, cx| { this.release_update.checking = true; cx.notify(); });
        let result = cx.background_spawn(async {
            practice::updates::check(env!("LEET_VERSION"), practice::updates::appimage_path().is_some())
        }).await;
        let _ = this.update(cx, |this, cx| {
            this.release_update.checking = false;
            match result {
                Ok(latest) => this.release_update.latest = Some(latest),
                Err(error) => { eprintln!("leet: update check: {error:#}"); this.release_update.error = Some("Release check unavailable".into()); }
            }
            cx.notify();
        });
    })
}

fn install(this: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    if this.release_update.downloading { return; }
    let Some(update) = this.release_update.latest.as_ref().and_then(|latest| latest.available.clone()) else { return; };
    let destination = practice::updates::appimage_path().map(Ok).unwrap_or_else(std::env::current_exe);
    let destination = match destination {
        Ok(path) => path,
        Err(error) => { this.release_update.error = Some(error.to_string()); cx.notify(); return; }
    };
    this.release_update.restart_path = Some(launcher());
    this.release_update.downloading = true;
    this.release_update.error = None;
    this.release_update.progress.store(0, Ordering::Relaxed);
    let progress = this.release_update.progress.clone();
    cx.spawn_in(window, async move |this, cx| {
        let mut task = cx.background_spawn(async move {
            practice::updates::install(update, env!("LEET_VERSION"), &destination, move |downloaded, total| {
                if let Some(total) = total.filter(|total| *total > 0) {
                    progress.store(downloaded.saturating_mul(100).checked_div(total).unwrap_or(0).min(100), Ordering::Relaxed);
                }
            })
        });
        let result = loop {
            let tick = cx.background_executor().timer(Duration::from_millis(150));
            match futures::future::select(task, tick).await {
                futures::future::Either::Left((result, _)) => break result,
                futures::future::Either::Right((_, pending)) => {
                    task = pending;
                    if this.update(cx, |_, cx| cx.notify()).is_err() { return; }
                }
            }
        };
        let _ = this.update(cx, |this, cx| {
            this.release_update.downloading = false;
            match result {
                Ok(()) => {
                    this.update_ready = true;
                    restart(this, cx);
                },
                Err(error) => this.release_update.error = Some(format!("Update failed: {error}")),
            }
            cx.notify();
        });
    }).detach();
    cx.notify();
}

pub fn button(this: &Workspace, cx: &mut Context<Workspace>) -> impl IntoElement {
    let available = this.update_ready || this.release_update.latest.as_ref().is_some_and(|latest| latest.available.is_some());
    let workspace = cx.entity().downgrade();
    div().id("release-update-hover").relative().flex_shrink_0()
        .hoverable_tooltip(move |_, cx| cx.new(|cx| {
            if let Some(entity) = workspace.upgrade() {
                cx.observe(&entity, |_, _, cx| cx.notify()).detach();
            }
            Changelog { workspace: workspace.clone() }
        }).into())
        .child(Button::new("release-updates").ghost().small()
            .icon(gpui_kit::component::Icon::new(IconName::Download).text_color(cx.theme().primary))
            .accessibility_label(if this.update_ready { "Restart updated app" } else { "Download and install update" })
            .on_click(cx.listener(|this, _, window, cx| {
                if this.release_update.downloading { return; }
                if this.update_ready { restart(this, cx); }
                else { install(this, window, cx); }
            })))
        .when(available, |el| el.child(div().absolute().top_0().right_0().size(px(5.)).rounded_full().bg(cx.theme().primary)))
}

struct Changelog {
    workspace: WeakEntity<Workspace>,
}

impl Render for Changelog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.workspace.upgrade();
        let this = workspace.as_ref().map(|workspace| workspace.read(cx));
        let state = this.map(|this| &this.release_update);
        let status = if this.is_some_and(|this| this.update_ready) { "Update ready · click to restart".into() }
            else if state.is_some_and(|state| state.downloading) {
                format!("Downloading update ({}%)", state.map(|state| state.progress.load(Ordering::Relaxed)).unwrap_or(0))
            } else if state.is_some_and(|state| state.checking) { "Checking for updates…".into() }
            else if let Some(latest) = state.and_then(|state| state.latest.as_ref()) {
                if latest.available.is_some() { format!("{} · click to update", latest.version) }
                else { format!("Up to date · {}", latest.version) }
            } else { format!("leet {}", env!("LEET_VERSION")) };
        let error = state.and_then(|state| state.error.clone());
        let notes = state.and_then(|state| state.latest.as_ref()).map(|latest| latest.notes.clone())
            .filter(|notes| !notes.trim().is_empty())
            .unwrap_or_else(|| include_str!("../../../docs/changelog.md").to_owned());
        v_flex().w(px(380.)).p_4().gap_3().rounded_lg().border_1()
            .border_color(cx.theme().border).bg(cx.theme().popover).text_color(cx.theme().foreground).shadow_lg()
            .child(div().text_sm().child(status))
            .when_some(error, |el, error| el.child(div().text_xs().text_color(cx.theme().danger).child(error)))
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Changelog"))
            .child(div().id("release-notes-scroll").max_h(px(300.)).overflow_y_scroll()
                .child(TextView::markdown("release-notes", notes).selectable(true)))
    }
}

fn fingerprint(path: &PathBuf) -> Option<(PathBuf, SystemTime)> {
    let target = std::fs::canonicalize(path).ok()?;
    let modified = std::fs::metadata(&target).and_then(|m| m.modified()).ok()?;
    Some((target, modified))
}

pub fn watch(window: &mut Window, cx: &mut Context<Workspace>) -> Task<()> {
    let launcher = launcher();
    let started = fingerprint(&launcher);
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let now = fingerprint(&launcher);
            if now.is_none() || now == started {
                continue;
            }
            let _ = this.update_in(cx, |this, window, cx| {
                this.update_ready = true;
                window.push_notification(
                    Notification::info("New leet build ready · ctrl+shift+r restarts").title("Update"),
                    cx,
                );
                cx.notify();
            });
            break;
        }
    })
}

/// Starts the current build with the same arguments, then quits this one.
pub fn restart(this: &mut Workspace, cx: &mut Context<Workspace>) {
    if this.release_update.downloading { return; }
    this.save_now(cx);
    if let Some(session) = this.session.as_ref().filter(|session| session.question.is_some()) {
        let text = this.editor.read(cx).value();
        if std::fs::read_to_string(&session.path).ok().as_deref() != Some(text.as_ref()) {
            this.release_update.error = Some("Could not save solution; restart canceled".into());
            cx.notify();
            return;
        }
    }
    // current_exe can refer to a deleted/renamed file after an in-place update.
    let path = this.release_update.restart_path.clone().unwrap_or_else(launcher);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if std::process::Command::new(path).args(args).spawn().is_ok() {
        cx.quit();
    } else {
        this.release_update.error = Some("Update installed; click to retry restarting".into());
        cx.notify();
    }
}
