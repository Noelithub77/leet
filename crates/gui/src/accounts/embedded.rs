//! An ephemeral native login window. Only validated captures leave this module.
use std::{cell::RefCell, rc::Rc, time::{Duration, Instant}};
use anyhow::{Result, anyhow};
use futures::channel::oneshot;
use gpui_kit::*;
use practice::creds::Account;
use super::capture::{self, Capture};

struct LoginWindow {
    webview: Option<wry::WebView>,
    reply: Option<oneshot::Sender<Result<Capture>>>,
}

pub(super) fn open(account: Account, cx: &mut App) -> Result<oneshot::Receiver<Result<Capture>>> {
    let (send, receive) = oneshot::channel();
    let options = WindowOptions {
        // Native web content needs a physical initial viewport; subsequent sizes follow the window.
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(820.), px(760.)), cx))),
        window_min_size: Some(size(px(500.), px(480.))),
        titlebar: Some(TitlebarOptions { title: Some(format!("Sign in to {}", account.label()).into()), ..Default::default() }),
        ..Default::default()
    };
    let mut sender = Some(send);
    gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| {
        let inbox = Rc::new(RefCell::new(None));
        let incoming = inbox.clone();
        let webview = wry::WebViewBuilder::new()
            .with_url(match account { Account::LeetCode => "https://leetcode.com/accounts/login/", Account::NeetCode => "https://neetcode.io", Account::TakeUForward => "https://takeuforward.org/get-started" })
            .with_incognito(true)
            .with_devtools(false)
            .with_navigation_handler(|raw| url::Url::parse(&raw).is_ok_and(|url| url.scheme() == "https"))
            .with_ipc_handler(move |request| {
                if let Ok(capture) = capture::message(account, &request.uri().to_string(), request.body()) {
                    *incoming.borrow_mut() = Some(capture);
                }
            })
            .build(window);
        let mut view = LoginWindow { webview: None, reply: sender.take() };
        match webview {
            Ok(webview) => view.webview = Some(webview),
            Err(_) => {
                if let Some(reply) = view.reply.take() { let _ = reply.send(Err(anyhow!("Could not open the sign-in browser. Use manual import instead."))); }
                window.defer(cx, |window, _| window.remove_window());
                return view;
            }
        }
        cx.spawn_in(window, async move |this, cx| {
            let deadline = Instant::now() + Duration::from_secs(300);
            loop {
                cx.background_executor().timer(Duration::from_millis(750)).await;
                let running = this.update_in(cx, |this, window, _| {
                    let captured = inbox.borrow_mut().take();
                    let Some(webview) = this.webview.as_ref() else { return false; };
                    if let Some(mut capture) = captured {
                        let ready = match account {
                            Account::NeetCode => true,
                            Account::TakeUForward => {
                                let cookies = webview.cookies_for_url("https://backend-go.takeuforward.org/api/v1/auth/me").unwrap_or_default();
                                capture.secret = cookies.iter().map(|cookie| format!("{}={}", cookie.name(), cookie.value())).collect::<Vec<_>>().join("; ");
                                !capture.secret.is_empty()
                            }
                            Account::LeetCode => {
                                let cookies = webview.cookies_for_url("https://leetcode.com/graphql/").unwrap_or_default();
                                let has = |name| cookies.iter().any(|cookie| cookie.name() == name && !cookie.value().is_empty());
                                if has("LEETCODE_SESSION") && has("csrftoken") {
                                    capture.secret = cookies.iter().map(|cookie| format!("{}={}", cookie.name(), cookie.value())).collect::<Vec<_>>().join("; ");
                                    true
                                } else { false }
                            }
                        };
                        if ready {
                            if let Some(reply) = this.reply.take() { let _ = reply.send(Ok(capture)); }
                            this.webview.take();
                            window.remove_window();
                            return false;
                        }
                    }
                    if Instant::now() >= deadline {
                        if let Some(reply) = this.reply.take() { let _ = reply.send(Err(anyhow!("Sign-in timed out. Try again or use manual import."))); }
                        this.webview.take();
                        window.remove_window();
                        return false;
                    }
                    let current_url = webview.url().unwrap_or_default();
                    if let Ok(url) = url::Url::parse(&current_url) {
                        if let Some(host) = url.host_str() { window.set_window_title(&format!("Sign in to {} · {host}", account.label())); }
                    }
                    if capture::trusted(account, &current_url) {
                        let script = match account {
                            Account::LeetCode => "if (window === window.top && location.origin === 'https://leetcode.com') window.ipc.postMessage(JSON.stringify({userAgent: navigator.userAgent}));",
                            Account::NeetCode => include_str!("neetcode-login.js"),
                            Account::TakeUForward => include_str!("takeuforward-login.js"),
                        };
                        let _ = webview.evaluate_script(script);
                    }
                    true
                });
                if !matches!(running, Ok(true)) { break; }
            }
        }).detach();
        view
    }))?;
    Ok(receive)
}

impl Render for LoginWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div().size_full() }
}
