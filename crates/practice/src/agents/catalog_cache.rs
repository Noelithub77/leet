//! Persistent catalogs with one shared fetch per agent per application session.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Result, anyhow, ensure};
use crate::db::Db;
use super::{AgentKind, Catalog, Detected};

type SessionCatalog = Arc<OnceLock<Result<Catalog, String>>>;

pub struct CatalogCache {
    db: Arc<Db>,
    sessions: Mutex<HashMap<AgentKind, SessionCatalog>>,
}

impl CatalogCache {
    pub fn new(db: Arc<Db>) -> Self { Self { db, sessions: Mutex::new(HashMap::new()) } }

    pub fn cached(&self, kind: AgentKind) -> Option<Catalog> {
        let value = self.db.get(&key(kind)).ok().flatten()?;
        let catalog: Catalog = serde_json::from_str(&value).ok()?;
        (catalog.agent == kind && !catalog.models.is_empty()).then_some(catalog)
    }

    pub fn fetch(&self, agent: &Detected) -> Result<Catalog> {
        self.fetch_with(agent.kind, || super::catalog(agent))
    }

    fn fetch_with(&self, kind: AgentKind, load: impl FnOnce() -> Result<Catalog>) -> Result<Catalog> {
        let session = self.sessions.lock().map_err(|_| anyhow!("Model catalog cache lock failed"))?
            .entry(kind).or_default().clone();
        session.get_or_init(|| (|| {
            let catalog = load()?;
            ensure!(catalog.agent == kind && !catalog.models.is_empty(), "Agent returned an invalid or empty model catalog");
            self.db.set(&key(kind), &serde_json::to_string(&catalog)?)?;
            Ok(catalog)
        })().map_err(|error: anyhow::Error| error.to_string()))
            .clone().map_err(|error| anyhow!(error))
    }
}

fn key(kind: AgentKind) -> String { format!("agent-models:v1:{}", kind.binary()) }

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use super::super::Model;

    fn catalog(id: &str) -> Catalog {
        Catalog { agent: AgentKind::Antigravity, default_model: Some(id.into()), sign_in_hint: None,
            models: vec![Model { id: id.into(), label: id.into(), description: String::new(), efforts: vec![], default_effort: None, fast: false, free: false }] }
    }

    #[test]
    fn concurrent_consumers_fetch_once_and_next_session_revalidates() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open(&dir.path().join("cache.sqlite")).unwrap());
        let cache = Arc::new(CatalogCache::new(db.clone()));
        let calls = Arc::new(AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let (cache, calls) = (cache.clone(), calls.clone());
                scope.spawn(move || {
                    let result = cache.fetch_with(AgentKind::Antigravity, || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        Ok(catalog("flash-low"))
                    }).unwrap();
                    assert_eq!(result.default_model.as_deref(), Some("flash-low"));
                });
            }
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let next = CatalogCache::new(db);
        assert_eq!(next.cached(AgentKind::Antigravity), Some(catalog("flash-low")));
        next.fetch_with(AgentKind::Antigravity, || Ok(catalog("new-flash-low"))).unwrap();
        assert_eq!(next.cached(AgentKind::Antigravity), Some(catalog("new-flash-low")));
    }

    #[test]
    fn failed_refresh_preserves_disk_cache_and_is_not_repeated() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open(&dir.path().join("cache.sqlite")).unwrap());
        CatalogCache::new(db.clone()).fetch_with(AgentKind::Antigravity, || Ok(catalog("old"))).unwrap();
        let cache = CatalogCache::new(db.clone());
        assert!(cache.fetch_with(AgentKind::Antigravity, || Err(anyhow!("offline"))).is_err());
        assert!(cache.fetch_with(AgentKind::Antigravity, || panic!("must not retry this session")).is_err());
        assert_eq!(cache.cached(AgentKind::Antigravity), Some(catalog("old")));
        CatalogCache::new(db).fetch_with(AgentKind::Antigravity, || Ok(catalog("new"))).unwrap();
    }
}
