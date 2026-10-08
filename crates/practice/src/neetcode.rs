//! Native Firebase session validation and NeetCode account progress.
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use ureq::Agent;

use crate::creds::{self, Account, Creds};

const KEY: &str = "AIzaSyD4emZpWF1MIsu6Z8O6yaMMcPxJ2Z38L8g";
pub const EXPORT_SCRIPT: &str = include_str!("../assets/neetcode-export.js");
pub type Progress = BTreeMap<String, Vec<String>>;

pub struct Client {
    agent: Agent,
    session: Mutex<Option<(Creds, Instant)>>,
    token_url: String,
    lookup_url: String,
    api_url: String,
    load_session: fn(Account) -> Result<Creds>,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            agent: Agent::config_builder().timeout_global(Some(Duration::from_secs(30)))
                .http_status_as_error(false).build().into(),
            session: Mutex::new(None),
            token_url: format!("https://securetoken.googleapis.com/v1/token?key={KEY}"),
            lookup_url: format!("https://identitytoolkit.googleapis.com/v1/accounts:lookup?key={KEY}"),
            api_url: "https://neetcode.io/api/callableFunctionHttp".into(),
            load_session: creds::load,
        }
    }
}

#[derive(Deserialize)]
struct Token {
    id_token: String,
    refresh_token: String,
    user_id: String,
    expires_in: String,
}

#[derive(Deserialize)]
struct Import {
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(rename = "userID")]
    user_id: String,
}

impl Client {
    fn refresh(&self, mut session: Creds) -> Result<(Creds, Instant)> {
        let mut response = self.agent.post(&self.token_url)
            .send_form([("grant_type", "refresh_token"), ("refresh_token", session.refresh_token.as_str())])
            .map_err(|_| anyhow!("NeetCode token refresh unavailable; retry"))?;
        if !response.status().is_success() {
            let status = response.status();
            if status.as_u16() == 400 {
                let error: Value = response.body_mut().with_config().limit(1 << 20).read_json().unwrap_or_default();
                if matches!(error["error"]["message"].as_str(), Some("TOKEN_EXPIRED" | "USER_DISABLED" | "USER_NOT_FOUND" | "INVALID_REFRESH_TOKEN" | "PROJECT_NUMBER_MISMATCH")) {
                    bail!("NeetCode session rejected; import a new browser session");
                }
            }
            bail!("NeetCode token refresh unavailable (HTTP {status}); retry");
        }
        let token: Token = response.body_mut().with_config().limit(1 << 20).read_json()
            .map_err(|_| anyhow!("Invalid NeetCode token response"))?;
        let seconds: u64 = token.expires_in.parse().map_err(|_| anyhow!("Invalid token lifetime"))?;
        if seconds == 0 || seconds > 86400 || token.id_token.is_empty() || token.refresh_token.is_empty()
            || token.user_id.is_empty() || session.user_id != token.user_id {
            bail!("Invalid refreshed NeetCode identity");
        }
        session.id_token = token.id_token;
        session.refresh_token = token.refresh_token;
        Ok((session, Instant::now() + Duration::from_secs(seconds)))
    }

    pub fn import(&self, raw: &str) -> Result<Creds> {
        let imported = parse_import(raw)?;
        let mut guard = self.session.lock().map_err(|_| anyhow!("NeetCode session lock unavailable"))?;
        let (mut session, expiry) = self.refresh(imported)?;
        session.username = self.identity(&session)?;
        creds::save(Account::NeetCode, session.clone()).map_err(|_| anyhow!("Could not persist NeetCode session"))?;
        *guard = Some((session.clone(), expiry));
        Ok(session)
    }

    /// Resolve a human identity for existing sessions that only retained an internal ID.
    pub fn identity(&self, session: &Creds) -> Result<String> {
        let mut response = self.agent.post(&self.lookup_url)
            .send_json(json!({"idToken": session.id_token}))
            .map_err(|_| anyhow!("NeetCode identity lookup unavailable; retry"))?;
        if !response.status().is_success() { bail!("NeetCode identity rejected"); }
        let lookup: Value = response.body_mut().with_config().limit(1 << 20).read_json()
            .map_err(|_| anyhow!("Invalid NeetCode identity response"))?;
        let users = lookup["users"].as_array().ok_or_else(|| anyhow!("Missing NeetCode identity"))?;
        if users.len() != 1 || users[0]["localId"].as_str() != Some(&session.user_id)
            || users[0]["disabled"].as_bool() == Some(true) { bail!("NeetCode identity rejected"); }
        Ok(profile_name(&users[0]).to_owned())
    }

    fn call<T: DeserializeOwned>(&self, function: &str, args: Value) -> Result<(T, Creds)> {
        // Serialize refresh and calls so rotated credentials cannot race one another.
        let mut guard = self.session.lock().map_err(|_| anyhow!("NeetCode session lock unavailable"))?;
        let current = (self.load_session)(Account::NeetCode)?;
        if guard.as_ref().is_none_or(|(session, expiry)| session.user_id != current.user_id
            || session.refresh_token != current.refresh_token
            || expiry.saturating_duration_since(Instant::now()) < Duration::from_secs(300)) {
            let refreshed = self.refresh(current)?;
            creds::save(Account::NeetCode, refreshed.0.clone()).map_err(|_| anyhow!("Could not persist refreshed session"))?;
            *guard = Some(refreshed);
        }
        for attempt in 0..2 {
            let (session, _) = guard.as_ref().ok_or_else(|| anyhow!("NeetCode sign-in required"))?;
            let mut data = args.clone();
            data["functionId"] = function.into();
            let mut response = self.agent.post(&self.api_url)
                .header("Authorization", &format!("Bearer {}", session.id_token))
                .send_json(json!({"data": data}))
                .map_err(|_| anyhow!("NeetCode request unavailable; retry"))?;
            let status = response.status();
            let body: Value = if matches!(status.as_u16(), 401 | 403) { Value::Null } else {
                response.body_mut().with_config().limit(4 << 20).read_json()
                    .map_err(|_| anyhow!("Invalid NeetCode response"))?
            };
            if status.as_u16() == 401 || status.as_u16() == 403 || body["error"]["status"] == "UNAUTHENTICATED" {
                if attempt == 0 {
                    let refreshed = self.refresh(session.clone())?;
                    creds::save(Account::NeetCode, refreshed.0.clone()).map_err(|_| anyhow!("Could not persist refreshed session"))?;
                    *guard = Some(refreshed);
                    continue;
                }
                bail!("NeetCode sign-in required; import a new browser session");
            }
            if !status.is_success() || !body["error"].is_null() { bail!("NeetCode operation failed; retry"); }
            if body.get("data").is_none() { bail!("Missing NeetCode acknowledgement"); }
            return serde_json::from_value(body["data"].clone()).map(|data| (data, session.clone()))
                .map_err(|_| anyhow!("Invalid NeetCode account response"));
        }
        bail!("NeetCode sign-in required")
    }

    pub fn progress(&self) -> Result<(Progress, Creds)> {
        let (progress, session) = self.call::<Option<Progress>>("getCompletedProblems", json!({}))?;
        Ok((progress.unwrap_or_default(), session))
    }

    pub fn saved_code(&self, problem: &str) -> Result<(Value, Creds)> {
        self.call("getUserCode", json!({"problemId":problem}))
    }

    pub fn mark(&self, topic: &str, slug: &str, solved: bool) -> Result<()> {
        self.call::<Value>(if solved { "markProblemComplete" } else { "markProblemIncomplete" },
            json!({"topic": topic, "problem": format!("{slug}/")}))?;
        Ok(())
    }
}

fn profile_name(user: &Value) -> &str {
    ["displayName", "email"].into_iter().filter_map(|field| user[field].as_str())
        .map(str::trim).find(|name| !name.is_empty() && Some(*name) != user["localId"].as_str()).unwrap_or("")
}

fn parse_import(raw: &str) -> Result<Creds> {
    if raw.len() > 1 << 20 { bail!("NeetCode session export is too large"); }
    let imported: Import = serde_json::from_str(raw).map_err(|_| anyhow!("Paste the JSON copied by the NeetCode browser script"))?;
    if imported.refresh_token.is_empty() || imported.user_id.is_empty() { bail!("Missing NeetCode session identity"); }
    Ok(Creds { refresh_token: imported.refresh_token, user_id: imported.user_id, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_labels_prefer_names_then_email_and_never_ids() {
        assert_eq!(profile_name(&json!({"localId":"uid", "displayName":" Name ", "email":"mail@example.com"})), "Name");
        assert_eq!(profile_name(&json!({"localId":"uid", "displayName":" ", "email":"mail@example.com"})), "mail@example.com");
        assert_eq!(profile_name(&json!({"localId":"uid", "displayName":"uid"})), "");
        assert_eq!(profile_name(&json!({"localId":"uid"})), "");
    }
    #[test]
    fn import_requires_identity_and_does_not_trust_browser_id_tokens() {
        assert!(parse_import(r#"{"refreshToken":"token"}"#).is_err());
        assert!(parse_import(r#"{"refreshToken":"","userID":"u"}"#).is_err());
        let session = parse_import(r#"{"refreshToken":"r","userID":"u","idToken":"untrusted"}"#).unwrap();
        assert_eq!(session.user_id, "u");
        assert!(session.id_token.is_empty());
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    fn response(status: u16, body: Value) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut request = String::new();
            let mut length = 0;
            let mut chunked = false;
            {
                let mut reader = BufReader::new(&mut stream);
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" { break; }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                    if line.to_lowercase().contains("transfer-encoding: chunked") { chunked = true; }
                    request.push_str(&line);
                }
                let mut payload = Vec::new();
                if chunked {
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        let size = usize::from_str_radix(line.trim(), 16).unwrap();
                        if size == 0 { break; }
                        let mut chunk = vec![0; size];
                        reader.read_exact(&mut chunk).unwrap();
                        payload.extend(chunk);
                        let mut newline = [0; 2];
                        reader.read_exact(&mut newline).unwrap();
                    }
                } else {
                    payload.resize(length, 0);
                    reader.read_exact(&mut payload).unwrap();
                }
                request.push_str("\r\n");
                request.push_str(&String::from_utf8(payload).unwrap());
            }
            let body = body.to_string();
            write!(stream, "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            request
        });
        (url, task)
    }

    fn signed_client(url: String) -> Client {
        Client {
            api_url: url,
            load_session: |_| Ok(Creds { id_token: "test-id".into(), ..Default::default() }),
            session: Mutex::new(Some((Creds { id_token: "test-id".into(), ..Default::default() }, Instant::now() + Duration::from_secs(3600)))),
            ..Default::default()
        }
    }

    #[test]
    fn identity_lookups_verify_the_owner_and_resolve_email() {
        for user in ["expected", "other"] {
            let (url, server) = response(200, json!({"users":[{"localId":user,"email":"person@example.com"}]}));
            let client = Client { lookup_url: url, ..Default::default() };
            let session = Creds { user_id: "expected".into(), id_token: "test-id".into(), ..Default::default() };
            let result = client.identity(&session);
            if user == "expected" { assert_eq!(result.unwrap(), "person@example.com"); }
            else { assert!(result.is_err()); }
            assert!(server.join().unwrap().contains("test-id"));
        }
    }

    #[test]
    fn progress_and_completion_use_the_native_callable_protocol() {
        let (url, server) = response(200, json!({"data": {"Arrays & Hashing": ["two-sum/"]}}));
        let (progress, _) = signed_client(url).progress().unwrap();
        assert_eq!(progress["Arrays & Hashing"], ["two-sum/"]);
        let request = server.join().unwrap();
        assert!(request.contains("getCompletedProblems"));
        for (solved, function) in [(true, "markProblemComplete"), (false, "markProblemIncomplete")] {
            let (url, server) = response(200, json!({"data": null}));
            signed_client(url).mark("Arrays & Hashing", "two-sum", solved).unwrap();
            let request = server.join().unwrap();
            let payload: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(payload["data"]["functionId"], function);
            assert_eq!(payload["data"]["problem"], "two-sum/");
            assert_eq!(payload["data"]["topic"], "Arrays & Hashing");
        }
    }

    #[test]
    fn completion_requires_an_acknowledgement() {
        let (url, server) = response(200, json!({}));
        assert!(signed_client(url).mark("Arrays & Hashing", "two-sum", true).is_err());
        server.join().unwrap();
    }

    #[test]
    fn token_errors_distinguish_revoked_sessions_from_outages() {
        for (status, body, expected) in [
            (400, json!({"error": {"message": "INVALID_REFRESH_TOKEN"}}), "session rejected"),
            (503, json!({"error": {"message": "private-details"}}), "retry"),
        ] {
            let (url, server) = response(status, body);
            let client = Client { token_url: url, ..Default::default() };
            let error = client.refresh(Creds::default()).unwrap_err().to_string();
            assert!(error.contains(expected));
            assert!(!error.contains("private-details"));
            server.join().unwrap();
        }
    }

    #[test]
    fn refresh_rejects_an_account_switch_and_accepts_rotated_tokens() {
        for user in ["other", "expected"] {
            let (url, server) = response(200, json!({"id_token":"new-id", "refresh_token":"new-refresh", "user_id":user, "expires_in":"3600"}));
            let client = Client { token_url: url, ..Default::default() };
            let result = client.refresh(Creds { user_id: "expected".into(), refresh_token: "old-refresh".into(), ..Default::default() });
            if user == "other" { assert!(result.is_err()); }
            else {
                let (session, expiry) = result.unwrap();
                assert_eq!(session.refresh_token, "new-refresh");
                assert!(expiry > Instant::now());
            }
            let request = server.join().unwrap();
            assert!(request.contains("grant_type=refresh_token") && request.contains("refresh_token=old-refresh"));
        }
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Explicit read-only account check; token refresh may rotate the saved session.
    #[test]
    #[ignore]
    fn neetcode_progress() {
        creds::load(Account::NeetCode).expect("saved NeetCode session required");
        let (progress, _) = Client::default().progress().expect("read NeetCode progress");
        eprintln!("NeetCode progress verified: {} topics", progress.len());
    }
}
