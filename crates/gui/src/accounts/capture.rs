//! Validate webview messages before they can enter account validation.
use anyhow::{Result, bail, ensure};
use practice::creds::Account;
use serde::Deserialize;

pub(super) struct Capture {
    pub secret: String,
    pub user_agent: String,
}

pub(super) fn trusted(account: Account, raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else { return false; };
    url.scheme() == "https" && url.port_or_known_default() == Some(443)
        && url.username().is_empty() && url.password().is_none()
        && url.host_str() == Some(match account { Account::LeetCode => "leetcode.com", Account::NeetCode => "neetcode.io", Account::TakeUForward => "takeuforward.org" })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Message {
    user_agent: String,
    refresh_token: Option<String>,
    user_id: Option<String>,
}

pub(super) fn message(account: Account, origin: &str, raw: &str) -> Result<Capture> {
    ensure!(trusted(account, origin), "Untrusted sign-in page");
    ensure!(raw.len() <= 64 * 1024, "Sign-in response too large");
    let value: Message = serde_json::from_str(raw).map_err(|_| anyhow::anyhow!("Invalid sign-in response"))?;
    ensure!(!value.user_agent.is_empty() && value.user_agent.len() <= 4096
        && !value.user_agent.contains(['\r', '\n']), "Invalid browser identity");
    let secret = match account {
        Account::LeetCode | Account::TakeUForward => String::new(),
        Account::NeetCode => {
            let (Some(token), Some(id)) = (value.refresh_token, value.user_id) else { bail!("Missing NeetCode session"); };
            ensure!(!token.is_empty() && !id.is_empty(), "Missing NeetCode session");
            serde_json::json!({"refreshToken": token, "userID": id}).to_string()
        }
    };
    Ok(Capture { secret, user_agent: value.user_agent })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_other_origins_and_oversized_or_malformed_messages() {
        for origin in ["http://neetcode.io", "https://neetcode.io.evil.test", "https://neetcode.io:444", "https://evil.test", "https://user@neetcode.io"] {
            assert!(!trusted(Account::NeetCode, origin));
        }
        assert!(!trusted(Account::LeetCode, "https://neetcode.io"));
        assert!(message(Account::NeetCode, "https://neetcode.io/", "{}").is_err());
        assert!(message(Account::NeetCode, "https://neetcode.io/", &" ".repeat(65537)).is_err());
        assert!(message(Account::LeetCode, "https://leetcode.com/", r#"{"userAgent":"a\r\nb"}"#).is_err());
    }

    #[test]
    fn exports_only_the_expected_neetcode_fields() {
        let result = message(Account::NeetCode, "https://neetcode.io/practice", r#"{"userAgent":"Browser","refreshToken":"fixture-token","userId":"fixture-user","password":"discard"}"#).unwrap();
        assert_eq!(result.user_agent, "Browser");
        let value: serde_json::Value = serde_json::from_str(&result.secret).unwrap();
        assert_eq!(value, serde_json::json!({"refreshToken":"fixture-token", "userID":"fixture-user"}));
    }
}
