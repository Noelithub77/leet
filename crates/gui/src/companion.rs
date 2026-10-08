//! Window-owned browser imports; solution files always retain the current draft.
use gpui_kit::*;
use gpui_kit::component::notification::Notification;
use crate::workspace::Workspace;

impl Workspace {
    pub fn start_companion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.companion_task = None;
        let presence = match practice::companion::inbox::Presence::claim("window") { Ok(presence) => presence, Err(_) => return };
        if let Err(error) = crate::companion_service::ensure_started() {
            self.toast(Notification::error(format!("CPH listener: {error}")), window, cx);
        }
        self.companion_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                let _ = presence.refresh();
                if let Some(warning) = practice::companion::inbox::take_warning() {
                    let _ = this.update_in(cx, |this, window, cx| this.toast(Notification::warning(warning), window, cx));
                }
                let files = cx.background_spawn(async { practice::companion::inbox::pending().unwrap_or_default() }).await;
                for file in files.into_iter().take(32) {
                let import = match practice::companion::inbox::read(&file) {
                    Ok(import) => import,
                    Err(error) => {
                        let _ = practice::companion::inbox::finish(&file, false);
                        let _ = this.update_in(cx, |this, window, cx| this.toast(Notification::error(format!("CPH import: {error}")), window, cx));
                        continue;
                    }
                };
                let db = match this.update(cx, |this, _| this.db.clone()) { Ok(db) => db, Err(_) => break };
                let result = cx.background_spawn(async move { let slug = import.cache(&db)?; anyhow::Ok((slug, db.question(&import.slug()?)?.expect("import saved question"))) }).await;
                let cached = result.is_ok();
                let applied = this.update_in(cx, |this, window, cx| {
                    match result {
                        Ok((slug, q)) => {
                            this.save_now(cx);
                            this.register_browser_question(&q);
                            let source = practice::language::Source::for_problem(&slug);
                            if this.config.source != source {
                                this.config.source = source; this.contests.selected = None;
                                this.save_config(window, cx); this.rebuild_rows(); this.omni.stale = true;
                            }
                            let already_open = this.session.as_ref().is_some_and(|session| session.slug == slug) || this.tabs.iter().flatten().any(|tab| tab.session.slug == slug);
                            this.open_problem(slug.clone(), window, cx);
                            if let Err(error) = this.apply_imported_question(q.clone(), window, cx) {
                                this.toast(Notification::error(format!("Open CPH import: {error}")), window, cx);
                                return false;
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
                            if let Some(session) = &mut this.session { session.source = source; }
                            if already_open && q.meta["statementSource"] == "competitive-companion" { this.load_problem(slug.clone(), window, cx); }
                            this.bottom = true;
                            cx.activate(true);
                            window.activate_window();
                            this.toast(Notification::success("CPH samples imported"), window, cx);
                        }
                        Err(error) => this.toast(Notification::error(format!("CPH import: {error}")), window, cx),
                    }
                    cx.notify();
                    cached
                });
                match applied {
                    Ok(success) => { let _ = practice::companion::inbox::finish(&file, success); }
                    Err(_) => break,
                }
                }
                cx.background_executor().timer(practice::companion::inbox::POLL).await;
                if this.update(cx, |_, _| ()).is_err() { break; }
            }
        }));
    }

    pub(crate) fn register_browser_question(&mut self, q: &practice::leetcode::Question) {
        if q.slug.starts_with("cc:") {
            if !self.sources.codechef.iter().any(|item| item.slug == q.slug) {
                self.sources.codechef.push(practice::leetcode::CatalogItem { slug: q.slug.clone(), frontend_id: 0, title: q.title.clone(), level: 1, paid_only: false, ac_rate: 0., status: None });
                self.rebuild_rows(); self.omni.stale = true;
            }
            return;
        }
        if self.sources.by_slug.contains_key(&q.slug) { return; }
        let Ok((id, _)) = practice::codeforces::problem_id(&q.slug) else { return; };
        self.sources.by_slug.insert(q.slug.clone(), self.sources.catalog.len());
        self.sources.catalog.push(practice::leetcode::CatalogItem { slug: q.slug.clone(), frontend_id: id,
            title: q.title.clone(), level: 2, paid_only: false, ac_rate: 0., status: None });
        self.rebuild_rows(); self.omni.stale = true;
    }
}
