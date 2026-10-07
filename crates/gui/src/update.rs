//! Picks up new development builds: `./ops local:deploy` repoints the `leet` symlink, and the
//! running app offers a restart instead of updating under the user.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::*;

use crate::workspace::Workspace;

/// The command users launch: the installed symlink when present, else this executable.
fn launcher() -> PathBuf {
    let link = dirs::executable_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/bin")))
        .map(|d| d.join("leet"));
    match link {
        Some(link) if link.exists() => link,
        _ => std::env::current_exe().unwrap_or_else(|_| PathBuf::from("leet")),
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
pub fn restart(cx: &mut App) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if std::process::Command::new(launcher()).args(args).spawn().is_ok() {
        cx.quit();
    }
}
