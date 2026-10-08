//! Competitive Companion's JSON protocol over a bounded loopback-only receiver.
use std::{net::TcpListener, sync::{Arc, Mutex}};
use anyhow::{Context, Result, bail};
use axum::{Router, Json, extract::{State, DefaultBodyLimit}, routing::post, http::StatusCode};
use futures::channel::mpsc;
use serde::{Deserialize, Serialize};
pub mod inbox;
pub async fn tick(duration: std::time::Duration) { async_io::Timer::after(duration).await; }
pub const DEFAULT_PORTS: [u16; 6] = [1327, 4244, 6174, 10042, 10043, 10045];

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    pub name: String, pub url: String, pub group: String,
    pub tests: Vec<Sample>,
    #[serde(default)] pub interactive: bool,
    pub time_limit: Option<u64>, pub memory_limit: Option<u64>,
    #[serde(default)] pub batch: Option<Batch>,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Batch { pub id: String, pub size: usize }
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Sample { pub input: String, pub output: String }
impl Import {
    pub fn slug(&self) -> Result<String> {
        if self.interactive { bail!("Interactive problems need a custom judge and cannot use the sample runner"); }
        if self.name.trim().is_empty() || self.tests.is_empty() || self.tests.len() > 100 { bail!("Import needs a name and 1–100 sample tests"); }
        let url = url::Url::parse(&self.url)?;
        if matches!(url.host_str(), Some("codechef.com" | "www.codechef.com")) { return crate::codechef::slug_from_url(&url); }
        if !matches!(url.scheme(), "http" | "https") || !matches!(url.host_str(), Some("codeforces.com" | "www.codeforces.com")) {
            bail!("Browser imports support Codeforces and CodeChef problems");
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
            let mut q = if slug.starts_with("cc:") {
                crate::codechef::imported_question(&slug, &self.name, html, self.tests.iter().map(|s| s.input.clone()).collect(), self.tests.iter().map(|s| s.output.clone()).collect(), None)
            } else { crate::codeforces::parse_statement(&slug, &html)? };
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
        // Release listeners before a replacement receiver starts.
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}
struct QueueState { sender: mpsc::Sender<Import>, recent: std::collections::VecDeque<(Import, std::time::Instant)>, persistent: bool }
type Queue = Arc<Mutex<QueueState>>;
async fn receive(State(queue): State<Queue>, Json(import): Json<Import>) -> (StatusCode, String) {
    if let Err(error) = import.slug() { return (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()); }
    let mut queue = queue.lock().unwrap_or_else(|error| error.into_inner());
    let now = std::time::Instant::now();
    queue.recent.retain(|(_, time)| now.duration_since(*time) < std::time::Duration::from_secs(5));
    if queue.recent.iter().any(|(previous, _)| previous == &import) { return (StatusCode::ACCEPTED, "Already queued for leet".into()); }
    if queue.persistent {
        if let Err(error) = inbox::enqueue(&import) { return (StatusCode::SERVICE_UNAVAILABLE, error.to_string()); }
        let _ = queue.sender.try_send(import.clone());
        if queue.recent.len() == 32 { queue.recent.pop_front(); }
        queue.recent.push_back((import, now));
        return (StatusCode::ACCEPTED, "Saved for leet".into());
    }
    match queue.sender.try_send(import.clone()) {
        Ok(()) => { if queue.recent.len() == 32 { queue.recent.pop_front(); } queue.recent.push_back((import, now)); (StatusCode::ACCEPTED, "Queued for leet".into()) },
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Import queue is full or leet is closing".into()),
    }
}
pub fn start(port: u16) -> Result<(mpsc::Receiver<Import>, Stop)> {
    let (sender, events) = mpsc::channel(32);
    let queue = Arc::new(Mutex::new(QueueState { sender, recent: Default::default(), persistent: false }));
    Ok((events, listen(port, queue)?))
}
pub struct Receivers {
    pub events: mpsc::Receiver<Import>,
    pub ports: Vec<u16>,
    stops: Vec<Stop>,
    queue: Queue,
    candidates: Vec<u16>,
}
impl Receivers {
    pub fn refill(&mut self) {
        for &port in &self.candidates {
            if self.stops.len() == 3 { break; }
            if self.ports.contains(&port) { continue; }
            if let Ok(stop) = listen(port, self.queue.clone()) { self.stops.push(stop); self.ports.push(port); }
        }
    }
}
pub fn start_defaults() -> Receivers { start_ports(&DEFAULT_PORTS, true) }
fn start_ports(ports: &[u16], persistent: bool) -> Receivers {
    let (sender, events) = mpsc::channel(32);
    let queue = Arc::new(Mutex::new(QueueState { sender, recent: Default::default(), persistent }));
    let mut receivers = Receivers { events, ports: vec![], stops: vec![], queue, candidates: ports.to_vec() };
    receivers.refill(); receivers
}
fn listen(port: u16, queue: Queue) -> Result<Stop> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).context("Competitive Companion port is unavailable")?;
    listener.set_nonblocking(true)?;
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let app = Router::new().route("/", post(receive)).layer(DefaultBodyLimit::max(1 << 20))
        .with_state(queue);
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
    Ok(Stop { signal: Some(stop), thread: Some(thread) })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn network_guard() -> std::fs::File {
        let file = std::fs::File::options().read(true).write(true).create(true).truncate(false).open(std::env::temp_dir().join("leet-companion-network-tests.lock")).unwrap();
        file.lock().unwrap();
        file
    }
    fn reserve() -> TcpListener {
        static NEXT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(20000);
        // Stay outside the client ephemeral range so parallel HTTP tests cannot reuse a released listener port.
        loop {
            let port = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            assert!(port < 30000);
            if let Ok(listener) = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)) { return listener; }
        }
    }
    #[test]
    fn receiver_accepts_valid_posts_and_rejects_bad_methods_and_payloads() {
        let _guard = network_guard();
        let reservation = reserve();
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
        let mut misleading = import.clone(); misleading.group = "CodeChef".into();
        assert_eq!(misleading.slug().unwrap(), "cf:4:A");
        assert!(crate::language::Source::for_problem(&misleading.slug().unwrap()) == crate::language::Source::Codeforces);
        assert_eq!(import.cache(&db).unwrap(), "cf:4:A");
        assert_eq!(db.test_cases("cf:4:A").unwrap().unwrap()[0].input, "8\n");
        let mut full = db.question("cf:4:A").unwrap().unwrap(); full.content = "full statement".into(); db.save_question(&full).unwrap();
        import.cache(&db).unwrap(); assert_eq!(db.question("cf:4:A").unwrap().unwrap().content, "full statement");
        let mut bad = import.clone(); bad.url = "https://example.com/contest/4/problem/A".into(); assert!(bad.slug().is_err());
        bad = import; bad.interactive = true; assert!(bad.slug().is_err());
    }
    #[test]
    fn fallback_keeps_acquiring_until_three_ports_are_owned() {
        let _guard = network_guard();
        let mut reservations: Vec<_> = (0..6).map(|_| Some(reserve())).collect();
        let ports: Vec<_> = reservations.iter().map(|listener| listener.as_ref().unwrap().local_addr().unwrap().port()).collect();
        let mut receivers = start_ports(&ports, false); assert!(receivers.ports.is_empty());
        reservations[0] = None; receivers.refill(); assert_eq!(receivers.ports, [ports[0]]);
        reservations[2] = None; reservations[4] = None; receivers.refill(); assert_eq!(receivers.ports, [ports[0], ports[2], ports[4]]);
        reservations[1] = None; receivers.refill(); assert_eq!(receivers.ports.len(), 3);
        assert!(TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, ports[1])).is_ok());
        assert!(!DEFAULT_PORTS.contains(&27121));
    }
    #[test]
    fn broadcasts_are_deduplicated_but_new_clicks_are_imported() {
        let _guard = network_guard();
        use futures::FutureExt;
        let reservations: Vec<_> = (0..2).map(|_| reserve()).collect();
        let ports: Vec<_> = reservations.iter().map(|listener| listener.local_addr().unwrap().port()).collect(); drop(reservations);
        let mut receivers = start_ports(&ports, false);
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(3))).build().into();
        let mut payload = serde_json::json!({"name":"Number Mirror","group":"CodeChef","url":"https://www.codechef.com/problems/START01","tests":[{"input":"123\n","output":"123\n"}],"batch":{"id":"first-click","size":1}});
        for port in &ports { assert_eq!(agent.post(format!("http://127.0.0.1:{port}/")).send_json(&payload).unwrap().status().as_u16(), 202); }
        assert_eq!(futures::executor::block_on(futures::StreamExt::next(&mut receivers.events)).unwrap().slug().unwrap(), "cc:START01");
        assert!(futures::StreamExt::next(&mut receivers.events).now_or_never().is_none());
        payload["batch"]["id"] = serde_json::json!("second-click");
        agent.post(format!("http://127.0.0.1:{}/", ports[0])).send_json(&payload).unwrap();
        assert!(futures::executor::block_on(futures::StreamExt::next(&mut receivers.events)).is_some());
    }
    #[test]
    fn stopping_releases_the_port_before_restarting() {
        let _guard = network_guard();
        let reservation = reserve();
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
