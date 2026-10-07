//! Window-owned browser imports; solution files always retain the current draft.
use futures::StreamExt;
use gpui_kit::*;
use gpui_kit::component::notification::Notification;
use crate::workspace::Workspace;

impl Workspace {
    pub fn start_companion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.companion_task = None;
        if !self.config.companion_enabled { return; }
        let (mut events, stop) = match practice::companion::start(self.config.companion_port) {
            Ok(receiver) => receiver,
            Err(error) => { eprintln!("leet: browser import unavailable: {error}"); return; }
        };
        self.companion_task = Some(cx.spawn_in(window, async move |this, cx| {
            let _stop = stop;
            while let Some(import) = events.next().await {
                let db = match this.update(cx, |this, _| this.db.clone()) { Ok(db) => db, Err(_) => break };
                let result = cx.background_spawn(async move { let slug = import.cache(&db)?; anyhow::Ok((slug, db.question(&import.slug()?)?.expect("import saved question"))) }).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    match result {
                        Ok((slug, q)) => {
                            this.save_now(cx);
                            this.register_codeforces_question(&q);
                            this.open_problem(slug.clone(), window, cx);
                            if let Err(error) = this.apply_imported_question(q.clone(), window, cx) {
                                this.toast(Notification::error(format!("Open browser import: {error}")), window, cx);
                                return;
                            }
                            let cases = this.db.test_cases(&slug).ok().flatten().unwrap_or_default();
                            if let Some(session) = &mut this.session {
                                if session.slug == slug && session.question.is_some() && !session.running {
                                    session.results = vec![None; cases.len()]; session.cases = cases; session.selected_case = 0;
                                    session.compile_error = None; session.judge = None;
                                    session.question = Some(q.clone());
                                    this.statement.update(cx, |statement, cx| { statement.status = None; statement.blocks = practice::description::parse(&q.content); cx.notify(); });
                                }
                            }
                            this.bottom = true;
                            this.toast(Notification::success("Browser samples imported"), window, cx);
                        }
                        Err(error) => this.toast(Notification::error(format!("Browser import: {error}")), window, cx),
                    }
                    cx.notify();
                });
            }
        }));
    }

    pub(crate) fn register_codeforces_question(&mut self, q: &practice::leetcode::Question) {
        if self.sources.by_slug.contains_key(&q.slug) { return; }
        let Ok((id, _)) = practice::codeforces::problem_id(&q.slug) else { return; };
        self.sources.by_slug.insert(q.slug.clone(), self.sources.catalog.len());
        self.sources.catalog.push(practice::leetcode::CatalogItem { slug: q.slug.clone(), frontend_id: id,
            title: q.title.clone(), level: 2, paid_only: false, ac_rate: 0., status: None });
        self.rebuild_rows(); self.omni.stale = true;
    }
}
