//! Headless CPH listener; persists imports before launching the native window.
use std::{process::{Child, Stdio}, time::{Duration, Instant}};
use anyhow::Result;
use futures::StreamExt;
use practice::companion::{self, inbox};

pub fn ensure_started() -> Result<()> {
    if inbox::alive("receiver") { return Ok(()); }
    #[cfg(target_os = "linux")]
    if dirs::home_dir().is_some_and(|home| home.join(".config/systemd/user/leet-companion.service").exists() && std::env::var_os("XDG_CONFIG_HOME").is_none_or(|root| std::path::Path::new(&root) == home.join(".config"))) {
        let result = practice::background_process::command("systemctl").args(["--user", "start", "leet-companion.service"]).output()?;
        if result.status.success() { return Ok(()); }
    }
    practice::background_process::command(std::env::current_exe()?).arg("--companion-service").stdin(Stdio::null()).stdout(Stdio::null()).spawn()?;
    Ok(())
}
pub fn run() -> Result<()> {
    let Some(_lease) = inbox::claim_receiver()? else { return Ok(()); };
    let mut receivers = companion::start_defaults();
    let mut warned = false;
    inbox::publish_ports(&receivers.ports)?;
    eprintln!("leet CPH listening on {:?}", receivers.ports);
    let presence = inbox::Presence::claim("receiver")?;
    let mut child: Option<Child> = None;
    let mut last_launch = None;
    let mut upgrade_warned = false;
    futures::executor::block_on(async {
        loop {
            presence.refresh()?;
            let import = {
            let event = receivers.events.next(); let tick = practice::companion::tick(Duration::from_secs(1));
            futures::pin_mut!(event, tick);
            match futures::future::select(event, tick).await { futures::future::Either::Left((import, _)) => import, _ => None }
            };
            let _ = import; // HTTP acknowledgement already persisted the import.
            if receivers.ports.len() < 3 {
                let count = receivers.ports.len(); receivers.refill();
                if receivers.ports.len() != count { inbox::publish_ports(&receivers.ports)?; eprintln!("leet CPH listening on {:?}", receivers.ports); }
            }
            if receivers.ports.is_empty() && !warned {
                inbox::warning("CPH unavailable: all eligible default ports are occupied. Leet will keep retrying.")?;
                if !inbox::window_running() {
                    let _ = practice::background_process::command("notify-send").args(["--app-name=leet", "CPH unavailable", "All eligible default ports are occupied. Leet will keep retrying."]).status();
                }
                warned = true;
            } else if !receivers.ports.is_empty() { warned = false; }
            if let Some(process) = &mut child { if process.try_wait()?.is_some() { child = None; } }
            let pending = !inbox::pending()?.is_empty();
            if pending && inbox::window_running() && !inbox::alive("window") && !upgrade_warned {
                let _ = practice::background_process::command("notify-send").args(["--app-name=leet", "Restart Leet to receive imports", "This window uses an older build. Your CPH import is saved; restart into the installed update."]).status();
                upgrade_warned = true;
            }
            if pending && !inbox::window_running() && child.is_none() && last_launch.is_none_or(|time: Instant| time.elapsed() >= Duration::from_secs(5)) {
                let mut command = practice::background_process::command(std::env::current_exe()?);
                #[cfg(target_os = "linux")]
                if std::env::var_os("WAYLAND_DISPLAY").is_none() {
                    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
                        for display in ["wayland-1", "wayland-0"] {
                            if std::path::Path::new(&runtime).join(display).exists() { command.env("WAYLAND_DISPLAY", display); break; }
                        }
                    }
                }
                child = Some(command.stdin(Stdio::null()).spawn()?); last_launch = Some(Instant::now());
            }
        }
    })
}
