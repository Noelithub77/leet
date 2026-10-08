//! Install and inspect the Linux login listener without changing app/account settings.
use std::{path::Path, process::Command};
use anyhow::{Context, Result, bail};
use serde_json::json;
fn run(args: &[&str]) -> Result<()> {
    let output = Command::new("systemctl").arg("--user").args(args).output()?;
    if !output.status.success() { bail!("systemctl {}: {}", args.join(" "), String::from_utf8_lossy(&output.stderr).trim()); }
    Ok(())
}
fn main() -> Result<()> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "status".into());
    if command == "--help" { println!("companion <install|status|stop> [--json]\nLocal Linux user service. install enables the always-on CPH listener at login; stop disables it without removing imports or solutions."); return Ok(()); }
    if !cfg!(target_os = "linux") { bail!("Login listener installation currently supports Linux systemd user sessions"); }
    let home = dirs::home_dir().context("Home directory unavailable")?;
    let binary = home.join(".local/bin/leet");
    let unit = home.join(".config/systemd/user/leet-companion.service");
    match command.as_str() {
        "install" => {
            if !binary.exists() { bail!("Install the desktop app with ./ops local:deploy first"); }
            std::fs::create_dir_all(unit.parent().context("Service directory unavailable")?)?;
            std::fs::write(&unit, format!("[Unit]\nDescription=Leet CPH browser import listener\n\n[Service]\nType=exec\nExecStart={} --companion-service\nRestart=on-failure\nRestartSec=10\nKillMode=process\n\n[Install]\nWantedBy=default.target\n", systemd_path(&binary)?))?;
            let vars: Vec<_> = ["DISPLAY", "WAYLAND_DISPLAY", "XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE"].into_iter().filter(|name| std::env::var_os(name).is_some()).collect();
            if !vars.is_empty() { let mut args = vec!["import-environment"]; args.extend(vars); run(&args)?; }
            run(&["daemon-reload"])?; run(&["enable", "leet-companion.service"])?; run(&["restart", "leet-companion.service"])?;
        },
        "stop" => run(&["disable", "--now", "leet-companion.service"])? ,
        "status" => {},
        _ => bail!("Unknown companion command {command}"),
    }
    let active = Command::new("systemctl").args(["--user", "is-active", "leet-companion.service"]).output()?;
    let enabled = Command::new("systemctl").args(["--user", "is-enabled", "leet-companion.service"]).output()?;
    let report = json!({"environment":"local-user-service","command":command,"unit":unit,"binary":binary,"active":active.status.success(),"enabled":enabled.status.success(),"candidate_ports":practice::companion::DEFAULT_PORTS,"target_listeners":3,"listening_ports":practice::companion::inbox::listening_ports()});
    println!("{report}");
    if command == "install" && !active.status.success() { bail!("CPH listener was installed but failed to start"); }
    Ok(())
}
fn systemd_path(path: &Path) -> Result<String> {
    let value = path.to_str().context("Executable path is not UTF-8")?;
    Ok(format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%")))
}
