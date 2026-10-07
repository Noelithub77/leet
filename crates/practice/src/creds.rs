//! Account sessions shared with the Go `verd` command: the same Secret Service entries
//! (service `verd`) and the same 0600 fallback files, so one sign-in serves both apps.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use secret_service::EncryptionType;
use secret_service::blocking::SecretService;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "verd";

#[derive(Clone, Copy, Debug)]
pub enum Account {
    LeetCode,
    NeetCode,
}

impl Account {
    pub fn label(self) -> &'static str {
        match self { Self::LeetCode => "LeetCode", Self::NeetCode => "NeetCode" }
    }
    pub fn index(self) -> usize {
        match self { Self::LeetCode => 0, Self::NeetCode => 1 }
    }
    fn name(self) -> &'static str {
        match self {
            Account::LeetCode => "leetcode",
            Account::NeetCode => "neetcode",
        }
    }

    fn file(self) -> PathBuf {
        dirs::config_dir()
            .unwrap_or_default()
            .join("verd")
            .join(format!("credentials-{}.json", self.name()))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Creds {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cookie: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub refresh_token: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id_token: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub username: String,
    #[serde(default)]
    pub user_agent: String,
    #[serde(default)]
    pub saved_at: i64,
    /// Fields only verd uses (e.g. `expires_at`), kept so saving never drops them.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Creds {
    fn valid(&self) -> bool {
        !self.cookie.is_empty() || !self.refresh_token.is_empty()
    }

    /// The value of one cookie in the stored Cookie header.
    pub fn cookie_value(&self, name: &str) -> Option<&str> {
        self.cookie.split(';').find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then_some(value)
        })
    }
}

fn attributes(account: Account) -> HashMap<&'static str, &'static str> {
    HashMap::from([("service", SERVICE), ("username", account.name())])
}

fn keyring_load(account: Account) -> Result<Option<Creds>> {
    let ss = SecretService::connect(EncryptionType::Dh)?;
    let found = ss.search_items(attributes(account))?;
    let Some(item) = found.unlocked.first().or(found.locked.first()) else {
        return Ok(None);
    };
    item.ensure_unlocked()?;
    let creds: Creds = serde_json::from_slice(&item.get_secret()?)?;
    Ok(creds.valid().then_some(creds))
}

fn file_load(account: Account) -> Option<Creds> {
    let bytes = std::fs::read(account.file()).ok()?;
    serde_json::from_slice::<Creds>(&bytes).ok().filter(Creds::valid)
}

/// Loads the newest valid session from the keyring or the fallback file.
pub fn load(account: Account) -> Result<Creds> {
    let keyed = keyring_load(account).ok().flatten();
    let filed = file_load(account);
    match (keyed, filed) {
        (Some(k), Some(f)) => Ok(if f.saved_at > k.saved_at { f } else { k }),
        (Some(c), None) | (None, Some(c)) => Ok(c),
        (None, None) => Err(anyhow!("not signed in to {}", account.name())),
    }
}

/// Removes the shared local session; never changes browser cookies or remote accounts.
pub fn remove(account: Account) -> Result<()> {
    // A keyring failure must not be reported as a successful sign-out.
    let ss = SecretService::connect(EncryptionType::Dh).context("Credential store unavailable; retry sign-out")?;
    let found = ss.search_items(attributes(account))?;
    for item in found.unlocked.iter().chain(found.locked.iter()) {
        item.ensure_unlocked()?;
        item.delete()?;
    }
    match std::fs::remove_file(account.file()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Saves to the keyring, falling back to a 0600 file. Returns where it was stored.
pub fn save(account: Account, mut creds: Creds) -> Result<&'static str> {
    creds.saved_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as i64;
    let json = serde_json::to_vec(&creds)?;
    let keyed = (|| -> Result<()> {
        let ss = SecretService::connect(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        collection.ensure_unlocked()?;
        let label = format!("Password for '{}' on '{SERVICE}'", account.name());
        collection.create_item(&label, attributes(account), &json, true, "text/plain")?;
        Ok(())
    })();
    if keyed.is_ok() {
        let _ = std::fs::remove_file(account.file());
        return Ok("OS keyring");
    }
    let path = account.file();
    std::fs::create_dir_all(path.parent().context("credential dir")?)?;
    write_private(&path, &json)?;
    Ok("private file")
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(bytes)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cookie_values() {
        let creds = Creds {
            cookie: "csrftoken=abc; LEETCODE_SESSION=xyz=1".into(),
            ..Default::default()
        };
        assert_eq!(creds.cookie_value("csrftoken"), Some("abc"));
        assert_eq!(creds.cookie_value("LEETCODE_SESSION"), Some("xyz=1"));
        assert_eq!(creds.cookie_value("missing"), None);
    }
}
