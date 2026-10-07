use std::{process::Command,time::Duration};
use anyhow::{Result,bail};
use super::{AgentKind,Cancel,transport::Process};
/// Run only the vendor's installer. Installer failures include bounded diagnostics.
pub fn install(kind:AgentKind)->Result<()>{
    let docs=if kind==AgentKind::Antigravity {"https://antigravity.google/docs/cli/install"}else{"https://opencode.ai/docs/"};
    let mut command;
    match kind {
        AgentKind::OpenCode=>{
            #[cfg(windows)] {command=Command::new("cmd");command.args(["/C","npm","i","-g","opencode-ai"]);}
            #[cfg(not(windows))] {command=Command::new("bash");command.args(["-o","pipefail","-c","curl -fsSL https://opencode.ai/install | bash"]);}
        }
        AgentKind::Antigravity=>{
            #[cfg(windows)] {command=Command::new("powershell");command.args(["-NoProfile","-Command","$ErrorActionPreference = 'Stop'; irm https://antigravity.google/cli/install.ps1 | iex"]);}
            #[cfg(not(windows))] {command=Command::new("bash");command.args(["-o","pipefail","-c","curl -fsSL https://antigravity.google/cli/install.sh | bash"]);}
        }
        _=>bail!("{} has no supported automatic installer",kind.label()),
    }
    let mut process=Process::spawn(&mut command,Duration::from_secs(900)).map_err(|error|anyhow::anyhow!("Start {} installer ({docs}): {error}",kind.label()))?;process.close_input();while process.line(&Cancel::default()).map_err(|error|anyhow::anyhow!("{} installer failed; see {docs}: {error}",kind.label()))?.is_some(){}Ok(())
}
