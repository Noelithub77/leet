//! Competitive Companion's JSON protocol over a bounded loopback-only receiver.
use std::{net::TcpListener, sync::{Arc, Mutex}};
use anyhow::{Context, Result, bail};
use axum::{Router, Json, extract::{State, DefaultBodyLimit}, routing::post, http::StatusCode};
use futures::channel::mpsc;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    pub name: String, pub url: String, pub group: String,
    pub tests: Vec<Sample>,
    #[serde(default)] pub interactive: bool,
    pub time_limit: Option<u64>, pub memory_limit: Option<u64>,
}
#[derive(Clone, Deserialize)]
pub struct Sample { pub input: String, pub output: String }
impl Import {
    pub fn slug(&self) -> Result<String> {
        if self.interactive { bail!("Interactive problems need a custom judge and cannot use the sample runner"); }
        if self.name.trim().is_empty() || self.tests.is_empty() || self.tests.len() > 100 { bail!("Import needs a name and 1–100 sample tests"); }
        let url = url::Url::parse(&self.url)?;
        if !matches!(url.scheme(), "http" | "https") || !matches!(url.host_str(), Some("codeforces.com" | "www.codeforces.com")) {
            bail!("Browser imports currently support Codeforces problems");
        }
        let parts: Vec<_> = url.path_segments().context("Problem URL has no path")?.collect();
        let (contest, index) = match parts.as_slice() {
            ["problemset", "problem", contest, index] | ["contest", contest, "problem", index] => (*contest, *index),
            _ => bail!("Use a Codeforces problem page"),
        };
        let slug = format!("cf:{contest}:{index}"); crate::codeforces::problem_id(&slug)?;
        Ok(slug)
    }
    pub fn cache(&self, db: &crate::db::Db) -> Result<String> {
        let slug = self.slug()?;
        let cases: Vec<_> = self.tests.iter().enumerate().map(|(index, sample)| crate::runner::Case {
            id: index, custom: true, input: sample.input.clone(), expected: Some(sample.output.trim().into()),
        }).collect();
        db.save_test_cases(&slug, &cases)?;
        if db.question(&slug)?.is_none() {
            let escape = |text: &str| html_escape::encode_text(text).into_owned();
            let mut html = format!("<div class=\"problem-statement\"><div class=\"header\"><div class=\"title\">{}</div></div><p>Imported browser samples · {} ms · {} MB</p><p>Open the source for the complete statement.</p><div class=\"sample-test\">",
                escape(&self.name), self.time_limit.unwrap_or(0), self.memory_limit.unwrap_or(0));
            for sample in &self.tests { html.push_str(&format!("<div class=\"input\"><pre>{}</pre></div><div class=\"output\"><pre>{}</pre></div>", escape(&sample.input), escape(&sample.output))); }
            html.push_str("</div></div>");
            let mut q = crate::codeforces::parse_statement(&slug, &html)?;
            q.meta["statementSource"] = serde_json::json!("competitive-companion");
            db.save_question(&q)?;
        }
        Ok(slug)
    }
}

pub struct Stop {
    signal: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Stop {
    fn drop(&mut self) {
        if let Some(stop) = self.signal.take() { let _ = stop.send(()); }
        // Release the port before Settings starts a replacement receiver.
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}
type Queue = Arc<Mutex<mpsc::Sender<Import>>>;
async fn receive(State(queue): State<Queue>, Json(import): Json<Import>) -> (StatusCode, String) {
    if let Err(error) = import.slug() { return (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()); }
    match queue.lock().unwrap_or_else(|error| error.into_inner()).try_send(import) {
        Ok(()) => (StatusCode::ACCEPTED, "Queued for leet".into()),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Import queue is full or leet is closing".into()),
    }
}
pub fn start(port: u16) -> Result<(mpsc::Receiver<Import>, Stop)> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).context("Competitive Companion port is unavailable")?;
    listener.set_nonblocking(true)?;
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let (sender, events) = mpsc::channel(32);
    let app = Router::new().route("/", post(receive)).layer(DefaultBodyLimit::max(1 << 20))
        .with_state(Arc::new(Mutex::new(sender)));
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let thread = std::thread::Builder::new().name("companion-receiver".into()).spawn(move || {
        runtime.block_on(async move {
            match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => {
                    use std::future::IntoFuture;
                    // Cancel active connections too, so an incomplete browser request cannot hold the port open.
                    let server = axum::serve(listener, app).into_future();
                    futures::pin_mut!(server, stopped);
                    let _ = futures::future::select(server, stopped).await;
                }
                Err(error) => eprintln!("leet companion: {error}"),
            }
        });
    })?;
    Ok((events, Stop { signal: Some(stop), thread: Some(thread) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receiver_accepts_valid_posts_and_rejects_bad_methods_and_payloads() {
        let reservation = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = reservation.local_addr().unwrap().port(); drop(reservation);
        let (mut events, stop) = start(port).unwrap();
        let endpoint = format!("http://127.0.0.1:{port}/");
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(3))).build().into();
        let payload = serde_json::json!({"name":"A. Watermelon", "group":"Codeforces", "url":"https://codeforces.com/contest/4/problem/A", "tests":[{"input":"8\n", "output":"YES\n"}]});
        assert_eq!(agent.post(&endpoint).send_json(&payload).unwrap().status().as_u16(), 202);
        assert_eq!(futures::executor::block_on(futures::StreamExt::next(&mut events)).unwrap().slug().unwrap(), "cf:4:A");
        assert!(matches!(agent.get(&endpoint).call(), Err(ureq::Error::StatusCode(405))));
        assert!(matches!(agent.post(&endpoint).send_json(&serde_json::json!({})), Err(ureq::Error::StatusCode(422))));
        let mut bad = payload; bad["url"] = serde_json::json!("https://codeforces.com.evil.example/contest/4/problem/A");
        assert!(matches!(agent.post(&endpoint).send_json(&bad), Err(ureq::Error::StatusCode(422))));
        drop(stop);
    }
    #[test]
    fn imports_samples_without_replacing_a_statement_or_solution() {
        let import: Import = serde_json::from_value(serde_json::json!({"name":"A. Watermelon","url":"https://codeforces.com/problemset/problem/4/A","group":"Codeforces","tests":[{"input":"8\n","output":"YES\n"}]})).unwrap();
        let dir = tempfile::tempdir().unwrap(); let db = crate::db::Db::open_unseeded(&dir.path().join("private.sqlite")).unwrap();
        assert_eq!(import.cache(&db).unwrap(), "cf:4:A");
        assert_eq!(db.test_cases("cf:4:A").unwrap().unwrap()[0].input, "8\n");
        let mut full = db.question("cf:4:A").unwrap().unwrap(); full.content = "full statement".into(); db.save_question(&full).unwrap();
        import.cache(&db).unwrap(); assert_eq!(db.question("cf:4:A").unwrap().unwrap().content, "full statement");
        let mut bad = import.clone(); bad.url = "https://example.com/contest/4/problem/A".into(); assert!(bad.slug().is_err());
        bad = import; bad.interactive = true; assert!(bad.slug().is_err());
    }
    #[test]
    fn stopping_releases_the_port_before_restarting() {
        let reservation = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = reservation.local_addr().unwrap().port(); drop(reservation);
        for _ in 0..5 {
            let (_events, stop) = start(port).unwrap();
            assert!(start(port).is_err());
            drop(stop);
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
            drop(listener);
        }
    }
}
