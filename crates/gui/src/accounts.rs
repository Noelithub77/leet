//! Account forms and progress work stay off the UI thread.
use std::collections::HashSet;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::creds::{self, Account, Creds};
use practice::leetcode::Client;
use practice::neetcode;

use crate::workspace::Workspace;

gpui_kit::actions!(accounts, [SaveAccount]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-enter", SaveAccount, Some("AccountForm > Input")),
        KeyBinding::new("enter", SaveAccount, Some("AccountForm > Input")),
        KeyBinding::new("shift-enter", SaveAccount, Some("AccountForm > Input")),
    ]);
}

struct AccountForm {
    focus: FocusHandle,
    account: Account,
    secret: Entity<InputState>,
    user_agent: Entity<InputState>,
    workspace: WeakEntity<Workspace>,
    busy: bool,
    has_session: bool,
    error: Option<String>,
}

pub fn open(account: Account, has_session: bool, window: &mut Window, cx: &mut Context<Workspace>) {
    let workspace = cx.weak_entity();
    let form = cx.new(|cx| {
        let secret = cx.new(|cx| InputState::new(window, cx).masked(true)
            .placeholder(match account { Account::LeetCode => "Cookie header", Account::NeetCode => "Browser session JSON" }));
        let user_agent = cx.new(|cx| InputState::new(window, cx).placeholder("User-Agent from the same request"));
        AccountForm { focus: cx.focus_handle(), account, secret, user_agent, workspace, busy: false, has_session, error: None }
    });
    let restore = cx.weak_entity();
    let focus = form.read(cx).focus.clone();
    let input = form.read(cx).secret.clone();
    window.open_dialog(cx, move |dialog, _, cx| { let restore = restore.clone(); let busy = form.read(cx).busy; dialog.title(format!("{} account", account.label())).w(px(580.))
        .close_button(!busy).overlay_closable(!busy).keyboard(!busy).on_close(move |_, window, cx| {
        let _ = restore.update(cx, |ws, cx| {
            if let Some(setup) = ws.onboarding.as_ref().filter(|_| ws.center == crate::workspace::Center::Onboarding) { let focus = setup.read(cx).focus.clone(); focus.focus(window, cx); }
            else { ws.focus_nav(crate::workspace::Focus::Settings, window, cx); }
        });
    }).child(form.clone()) });
    window.defer(cx, move |window, cx| {
        if has_session { focus.focus(window, cx); }
        else { input.update(cx, |input, cx| input.focus(window, cx)); }
    });
}

impl AccountForm {
    fn account_work_in_flight(&self, cx: &mut App) -> bool {
        self.workspace.update(cx, |ws, _| ws.syncing || ws.neetcode_syncing).unwrap_or(true)
    }

    fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy { return; }
        if self.account_work_in_flight(cx) {
            self.error = Some("Wait for the current account sync to finish.".into());
            cx.notify();
            return;
        }
        self.busy = true;
        let account = self.account;
        let workspace = self.workspace.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { creds::remove(account) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        let _ = workspace.update(cx, |ws, cx| {
                            ws.clear_remote_history();
                            ws.account_names[account.index()] = "Signed out".into();
                            match account {
                                Account::LeetCode => ws.client = Arc::new(Client::new(None)),
                                Account::NeetCode => {
                                    ws.neetcode = Arc::new(neetcode::Client::default());
                                    ws.neetcode_solved.clear();
                                    ws.rebuild_library();
                                }
                            }
                            if ws.center != crate::workspace::Center::Onboarding { ws.open_settings(Some(if matches!(account, Account::LeetCode) { crate::settings::Setting::LeetCode } else { crate::settings::Setting::NeetCode }), window, cx); } else { cx.notify(); }
                        });
                        window.close_dialog(cx);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy { return; }
        if self.account_work_in_flight(cx) {
            self.error = Some("Wait for the current account sync to finish.".into());
            cx.notify();
            return;
        }
        let raw = self.secret.read(cx).value().trim().to_string();
        let user_agent = self.user_agent.read(cx).value().trim().to_string();
        if raw.is_empty() { self.error = Some("Paste a browser session first.".into()); cx.notify(); return; }
        self.busy = true;
        self.error = None;
        let account = self.account;
        let workspace = self.workspace.clone();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move {
                match account {
                    Account::LeetCode => {
                        let mut session = Creds { cookie: raw, user_agent, ..Default::default() };
                        if session.cookie_value("LEETCODE_SESSION").is_none() || session.cookie_value("csrftoken").is_none() {
                            anyhow::bail!("Cookie must include LEETCODE_SESSION and csrftoken");
                        }
                        session.username = Client::new(Some(session.clone())).validate_session()?;
                        creds::save(account, session.clone())?;
                        Ok(session)
                    }
                    Account::NeetCode => neetcode::Client::default().import(&raw),
                }
            }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(session) => {
                        this.secret.update(cx, |input, cx| input.set_value("", window, cx));
                        let _ = workspace.update(cx, |ws, cx| {
                            ws.clear_remote_history();
                            ws.account_names[account.index()] = session.display_name().unwrap_or("Account").to_owned();
                            if matches!(account, Account::LeetCode) {
                                ws.client = Arc::new(Client::new(Some(session)));
                                ws.refresh_catalog(true, window, cx);
                            } else {
                                ws.neetcode = Arc::new(neetcode::Client::default());
                                ws.refresh_neetcode(window, cx);
                            }
                            if ws.center != crate::workspace::Center::Onboarding { ws.open_settings(Some(if matches!(account, Account::LeetCode) { crate::settings::Setting::LeetCode } else { crate::settings::Setting::NeetCode }), window, cx); } else { cx.notify(); }
                        });
                        window.close_dialog(cx);
                        window.push_notification(Notification::success(format!("{} signed in", account.label())), cx);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }).detach();
    }
}

impl Render for AccountForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let account = self.account;
        let instructions = match account {
            Account::LeetCode => "Sign in in your browser. From a leetcode.com/graphql Network request, copy Cookie and User-Agent.",
            Account::NeetCode => "Sign in on neetcode.io. Copy the export script, run it in that page’s browser console, then paste its output.",
        };
        if self.has_session {
            return v_flex().key_context("AccountForm").track_focus(&self.focus).gap_3()
                .child(format!("Log out of {}?", account.label()))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child("This removes the shared local login from leet and verd. Your browser login and solutions are kept."))
                .when_some(self.error.clone(), |form, error| form.child(div().text_sm().text_color(cx.theme().danger).child(error)))
                .child(h_flex().justify_end().gap_2()
                    .child(Button::new("cancel-log-out").label("Cancel").disabled(self.busy).on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("log-out").danger().disabled(self.busy).label(if self.busy { "Logging out…" } else { "Log out" })
                        .on_click(cx.listener(|this, _, window, cx| this.sign_out(window, cx)))))
                .into_any_element();
        }
        v_flex().key_context("AccountForm").gap_3()
            .on_action(cx.listener(|this, _: &SaveAccount, window, cx| this.save(window, cx)))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(instructions))
            .child(h_flex().gap_2()
                .child(Button::new("open-account").label("Open browser").on_click(move |_, window, cx| {
                    let url = match account { Account::LeetCode => "https://leetcode.com/accounts/login/", Account::NeetCode => "https://neetcode.io" };
                    if let Err(error) = open::that_detached(url) { window.push_notification(Notification::error(error.to_string()), cx); }
                }))
                .when(matches!(account, Account::NeetCode), |row| row.child(Button::new("copy-export").label("Copy export script").on_click(|_, window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(neetcode::EXPORT_SCRIPT.to_owned()));
                    window.push_notification(Notification::info("Export script copied"), cx);
                }))))
            .child(Input::new(&self.secret))
            .when(matches!(account, Account::LeetCode), |form| form.child(Input::new(&self.user_agent)))
            .when_some(self.error.clone(), |form, error| form.child(div().text_sm().text_color(cx.theme().danger).child(error)))
            .child(Button::new("save-account").primary().label(if self.busy { "Logging in…" } else { "Log in" })
                .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))))
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Ctrl+Enter to save · Escape to cancel"))
            .into_any_element()
    }
}

impl Workspace {
    pub fn refresh_neetcode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.neetcode_syncing { return; }
        self.neetcode_syncing = true;
        let (client, db) = (self.neetcode.clone(), self.db.clone());
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move {
                let (progress, account) = client.progress()?;
                let slugs: HashSet<String> = progress.values().flatten().map(|s| s.trim_matches('/').to_owned()).collect();
                db.set(&format!("neetcode-progress:{}", account.user_id), &slugs.iter().cloned().collect::<Vec<_>>().join("\n"))?;
                let name = account.display_name().map(str::to_owned).or_else(|| client.identity(&account).ok().filter(|name| !name.is_empty())).unwrap_or_else(|| "Account".into());
                anyhow::Ok((slugs, name))
            }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.neetcode_syncing = false;
                match result {
                    Ok((slugs, name)) => {
                        this.neetcode_solved = slugs;
                        this.account_names[1] = name;
                        this.rebuild_library();
                    }
                    Err(error) => this.toast(Notification::error(format!("NeetCode sync: {error}")), window, cx),
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn mark_neetcode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.neetcode_syncing { return; }
        let selected = || {
            match self.rows.get(self.sidebar_sel) {
                Some(crate::workspace::Row::Problem(i)) => Some(practice::roadmap::ENTRIES[*i].slug.clone()),
                _ => None,
            }
        };
        let slug = if self.focus_area == crate::workspace::Focus::Sidebar { selected() } else { self.session.as_ref().map(|s| s.slug.clone()) };
        let Some(slug) = slug else { return; };
        let Some(entry) = practice::roadmap::entry(&slug) else { return; };
        let topic = entry.topic.clone();
        let solved = !self.neetcode_solved.contains(&slug);
        let client = self.neetcode.clone();
        self.neetcode_syncing = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { client.mark(&topic, &slug, solved).map(|()| (slug, solved)) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.neetcode_syncing = false;
                match result {
                    Ok((slug, solved)) => {
                        if solved { this.neetcode_solved.insert(slug); } else { this.neetcode_solved.remove(&slug); }
                        this.rebuild_library();
                        this.refresh_neetcode(window, cx);
                        this.flash(if solved { "NeetCode complete" } else { "NeetCode incomplete" }, cx);
                    }
                    Err(error) => this.toast(Notification::error(error.to_string()), window, cx),
                }
                cx.notify();
            });
        }).detach();
    }
}
