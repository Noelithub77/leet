//! Installs releases in the background and offers a restart for installed updates or local builds.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

use gpui_kit::base::Disableable as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::text::TextView;
use gpui_kit::component::accordion::Accordion;
use gpui_kit::component::scroll::ScrollableElement as _;
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
    launch_path(current, link)
}

fn launch_path(current: PathBuf, link: Option<PathBuf>) -> PathBuf {
    match link {
        Some(link) if std::fs::canonicalize(&link).ok().as_ref() == Some(&current) => link,
        _ => current,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UpdateAction { Update, Downloading, Restart, UpToDate }

pub struct State {
    open: bool,
    trigger_hovered: bool,
    categories: Vec<usize>,
    scroll: ScrollHandle,
    pub latest: Option<practice::updates::Latest>,
    pub checking: bool,
    pub downloading: bool,
    pub error: Option<String>,
    progress: Arc<AtomicU64>,
    restart_path: Option<PathBuf>,
    local_build_ready: bool,
}

impl Default for State {
    fn default() -> Self {
        Self { open: false, trigger_hovered: false,
            categories: vec![0], scroll: ScrollHandle::new(), latest: None, checking: false, downloading: false, error: None,
            progress: Arc::default(), restart_path: None, local_build_ready: false }
    }
}

pub fn check(window: &mut Window, cx: &mut Context<Workspace>) -> Task<()> {
    cx.spawn_in(window, async move |this, cx| {
        let _ = this.update(cx, |this, cx| { this.release_update.checking = true; cx.notify(); });
        let result = cx.background_spawn(async {
            practice::updates::check(env!("LEET_VERSION"), practice::updates::appimage_path().is_some())
        }).await;
        let _ = this.update_in(cx, |this, window, cx| {
            this.release_update.checking = false;
            match result {
                Ok(latest) => {
                    this.release_update.latest = Some(latest);
                    install(this, window, cx);
                },
                Err(error) => { eprintln!("leet: update check: {error:#}"); this.release_update.error = Some("Release check unavailable".into()); }
            }
            cx.notify();
        });
    })
}

fn install(this: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    if this.release_update.downloading || this.update_ready { return; }
    let Some(update) = this.release_update.latest.as_ref().and_then(|latest| latest.available.clone()) else { return; };
    let destination = practice::updates::appimage_path().map(Ok).unwrap_or_else(std::env::current_exe);
    let destination = match destination {
        Ok(path) => path,
        Err(error) => { this.release_update.error = Some(error.to_string()); cx.notify(); return; }
    };
    if this.release_update.restart_path.is_none() {
        this.release_update.restart_path = Some(launcher());
    }
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
        let _ = this.update_in(cx, |this, window, cx| {
            this.release_update.downloading = false;
            match result {
                Ok(()) => {
                    this.update_ready = true;
                    window.push_notification(Notification::info("Update installed · restart now or use it next launch").title("Update"), cx);
                },
                Err(error) => this.release_update.error = Some(format!("Update failed: {error}")),
            }
            cx.notify();
        });
    }).detach();
    cx.notify();
}

impl State {
    #[cfg(feature = "gui-test")]
    pub(crate) fn is_open(&self) -> bool { self.open }
    #[cfg(feature = "gui-test")]
    pub(crate) fn scroll_offset(&self) -> Pixels { self.scroll.offset().y }

    fn action(&self, ready: bool) -> UpdateAction {
        if self.downloading { UpdateAction::Downloading }
        else if ready { UpdateAction::Restart }
        else if self.latest.as_ref().is_some_and(|latest| latest.available.is_some()) { UpdateAction::Update }
        else { UpdateAction::UpToDate }
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
    }
}

fn trigger_hover(this: &mut Workspace, entered: bool, _: &mut Window, _: &mut Context<Workspace>) {
    this.release_update.trigger_hovered = entered;
}

pub fn button(this: &Workspace, cx: &mut Context<Workspace>) -> impl IntoElement {
    let available = this.update_ready || this.release_update.latest.as_ref().is_some_and(|latest| latest.available.is_some());
    div().id("release-update-hover").relative().flex_shrink_0()
        .on_hover(cx.listener(|this, entered, window, cx| trigger_hover(this, *entered, window, cx)))
        .child(Button::new("release-updates").ghost().small()
            .icon(gpui_kit::component::Icon::new(IconName::Download).text_color(cx.theme().primary))
            .accessibility_label("Changelog and updates")
            .on_click(cx.listener(|this, _, window, cx| {
                this.ai_chip.update(cx, |chip, cx| chip.hide(cx));
                crate::language_picker::close(this, window, cx);
                this.omni.open = false;
                if this.release_update.open { this.release_update.close(); }
                else { this.release_update.open = true; }
                cx.notify();
            })))
        .when(available, |el| el.child(div().absolute().top_0().right_0().size(px(5.)).rounded_full().bg(cx.theme().primary)))
}

pub fn panel(this: &Workspace, window: &Window, cx: &mut Context<Workspace>) -> Option<AnyElement> {
    if !this.release_update.open { return None; }
    let state = &this.release_update;
    let action = state.action(this.update_ready);
    let status = if state.local_build_ready { "New local build ready".into() }
        else if this.update_ready { "Update installed · ready for next launch".into() }
        else if state.downloading { format!("Downloading · {}%", state.progress.load(Ordering::Relaxed)) }
        else if state.checking { "Checking for updates…".into() }
        else if let Some(latest) = &state.latest {
            if latest.available.is_some() { format!("Update available · {}", latest.version) }
            else { format!("Up to date · {}", practice::updates::current_version(env!("LEET_VERSION"))) }
        } else { format!("leet {}", env!("LEET_VERSION")) };
    let notes = state.latest.as_ref().filter(|latest| latest.available.is_some() || !is_development_build(env!("LEET_VERSION")))
        .map(|latest| latest.notes.clone()).filter(|notes| !notes.trim().is_empty())
        .unwrap_or_else(|| include_str!("../../../docs/changelog.md").to_owned());
    let parsed = practice::release_notes::Changelog::parse(&notes);
    let notes = if parsed.is_empty() { practice::release_notes::Changelog::parse(include_str!("../../../docs/changelog.md")) } else { parsed };
    let bullets = |items: Vec<String>| items.into_iter().map(|item| format!("- {item}")).collect::<Vec<_>>().join("\n");
    let label = match action {
        UpdateAction::Restart => "Restart now",
        UpdateAction::Downloading => "Updating…",
        UpdateAction::Update => "Retry update",
        UpdateAction::UpToDate => "Up to date",
    };
    let notes_height = (window.viewport_size().height - px(190.)).max(px(40.)).min(px(300.));
    Some(v_flex().id("release-update-panel").absolute().bottom(px(36.)).right(px(12.)).w(px(380.)).max_w_full()
        .occlude().p_4().gap_3().rounded_lg().border_1().border_color(cx.theme().border)
        .bg(cx.theme().popover).text_color(cx.theme().foreground).shadow_lg()
        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
            if !this.release_update.trigger_hovered { this.release_update.close(); cx.notify(); }
        }))
        .child(div().id("release-update-status").test_support().text_sm().aria_label(status.clone()).child(status))
        .when_some(state.error.clone(), |el, error| el.child(div().text_xs().text_color(cx.theme().danger).child(error)))
        .child(div().id("release-notes-scroll").test_support().h(notes_height).flex_shrink_0().overflow_y_scroll().track_scroll(&state.scroll).vertical_scrollbar(&state.scroll)
            .child(Accordion::new("release-categories").multiple(true).bordered(false).small()
                .item(|item| item.title("Feat").open(state.categories.contains(&0))
                    .child(TextView::markdown("release-feat", bullets(notes.feat)).selectable(true)))
                .item(|item| item.title("Fix").open(state.categories.contains(&1))
                    .child(TextView::markdown("release-fix", bullets(notes.fix)).selectable(true)))
                .on_toggle_click(cx.listener(|this, open: &[usize], _, cx| { this.release_update.categories = open.to_vec(); cx.notify(); }))))
        .child(h_flex().gap_2()
            .when(matches!(action, UpdateAction::Restart), |el| el.child(Button::new("release-update-later").ghost().small().label("Later")
                .on_click(cx.listener(|this, _, _, cx| { this.release_update.close(); cx.notify(); }))))
            .child(Button::new("release-update-action").primary().small().label(label).flex_1()
            .disabled(matches!(action, UpdateAction::Downloading | UpdateAction::UpToDate))
            .on_click(cx.listener(|this, _, window, cx| match this.release_update.action(this.update_ready) {
                UpdateAction::Update => install(this, window, cx),
                UpdateAction::Restart => restart(this, cx),
                UpdateAction::Downloading | UpdateAction::UpToDate => {},
            }))))
        .into_any_element())
}

fn is_development_build(display: &str) -> bool {
    display.split_once('(').is_some_and(|(_, describe)| {
        describe.trim_end_matches(')') != format!("v{}", practice::updates::current_version(display))
    })
}

fn fingerprint(path: &Path) -> Option<(PathBuf, SystemTime)> {
    let target = std::fs::canonicalize(path).ok()?;
    let modified = std::fs::metadata(&target).and_then(|m| m.modified()).ok()?;
    Some((target, modified))
}

pub fn watch(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) -> Task<()> {
    let launcher = launcher();
    let started = fingerprint(&launcher);
    workspace.release_update.restart_path = Some(launcher.clone());
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let now = fingerprint(&launcher);
            if now.is_none() || now == started {
                continue;
            }
            let finished = this.update_in(cx, |this, window, cx| {
                if this.release_update.downloading { return false; }
                if this.update_ready { return true; }
                this.update_ready = true;
                this.release_update.local_build_ready = true;
                window.push_notification(
                    Notification::info("New leet build ready · ctrl+shift+r restarts").title("Update"),
                    cx,
                );
                cx.notify();
                true
            });
            if finished.unwrap_or(true) { break; }
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
    if practice::background_process::command(path).args(args).spawn().is_ok() {
        cx.quit();
    } else {
        this.release_update.error = Some("Update installed; click to retry restarting".into());
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn update_action_waits_for_explicit_restart() {
        let mut state = State::default();
        assert_eq!(state.action(false), UpdateAction::UpToDate);
        state.downloading = true;
        assert_eq!(state.action(true), UpdateAction::Downloading);
        state.downloading = false;
        assert_eq!(state.action(true), UpdateAction::Restart);
        assert_eq!(state.categories, [0]);
    }

    #[cfg(unix)]
    #[::core::prelude::v1::test]
    fn captured_launcher_survives_repointing_and_removing_old_build() {
        let directory = std::env::temp_dir().join(format!("leet-launcher-test-{}-{}", std::process::id(), SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&directory).unwrap();
        let old = directory.join("old-build");
        let new = directory.join("new-build");
        let link = directory.join("leet");
        std::fs::write(&old, "old").unwrap();
        std::fs::write(&new, "new").unwrap();
        std::os::unix::fs::symlink(&old, &link).unwrap();
        let captured = launch_path(old.clone(), Some(link.clone()));
        let started = fingerprint(&captured);
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&new, &link).unwrap();
        std::fs::remove_file(&old).unwrap();
        assert_eq!(captured, link);
        assert_ne!(fingerprint(&captured), started);
        assert_eq!(std::fs::canonicalize(captured).unwrap(), new);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
