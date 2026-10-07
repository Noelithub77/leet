//! The status-line AI chip: the active agent and model with a live stopwatch, and a compact
//! picker for agent, model, reasoning, Fast tier, and the web chat fallback.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::agents::{AgentKind, Model, Selection};
use practice::prompts::Provider;

use crate::assist::{Assist, Loadable, Target};
use crate::workspace::Workspace;

gpui_kit::actions!(ai, [CloseAi]);
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CloseAi, Some("AiPicker")),
        KeyBinding::new("escape", CloseAi, Some("AiPicker > Input")),
    ]);
}

pub struct Chip {
    workspace: WeakEntity<Workspace>,
    assist: Entity<Assist>,
    picker: Option<Entity<Picker>>,
    return_focus: Option<crate::workspace::Focus>,
    _observe: Subscription,
}

impl Chip {
    pub fn new(workspace: WeakEntity<Workspace>, assist: Entity<Assist>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&assist, |_, _, cx| cx.notify());
        Self { workspace, assist, picker: None, return_focus: None, _observe: observe }
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| {
            let _ = workspace.update(cx, |ws, cx| { ws.release_update.close(); crate::language_picker::close(ws, window, cx); });
        });
        let chip = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = chip.update(cx, |this, cx| {
                this.return_focus = this.workspace.upgrade().map(|ws| ws.read(cx).focus_area);
                let (workspace, assist, chip) = (this.workspace.clone(), this.assist.clone(), cx.entity().downgrade());
                let picker = cx.new(|cx| Picker::new(workspace, assist, chip, window, cx));
                let focus = picker.read(cx).focus.clone();
                this.picker = Some(picker);
                focus.focus(window, cx);
                cx.notify();
            });
        });
    }

    /// Closes the picker without moving focus, for when another popup opens.
    pub fn hide(&mut self, cx: &mut Context<Self>) {
        self.picker = None;
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picker = None;
        let focus = self.return_focus.take();
        let _ = self.workspace.update(cx, |ws, cx| {
            match focus {
                Some(crate::workspace::Focus::Editor) if ws.debug_mode => ws.debugger.update(cx, |debugger, cx| debugger.focus(window, cx)),
                Some(crate::workspace::Focus::Editor | crate::workspace::Focus::Omnibar) if ws.session.is_some() => ws.focus_editor(window, cx),
                Some(focus) => ws.focus_nav(focus, window, cx),
                None => ws.focus_nav(crate::workspace::Focus::Home, window, cx),
            }
        });
        cx.notify();
    }
}

impl Render for Chip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(workspace) = self.workspace.upgrade() else { return div().into_any_element() };
        let config = workspace.read(cx).config.clone();
        let assist = self.assist.read(cx);
        let target = assist.target(&config);
        let (icon, label) = match &target {
            Target::Agent(agent, selection) => {
                let model = selection.as_ref().map(|s| s.model.clone())
                    .or_else(|| assist.catalog(agent.kind).and_then(|c| c.default_model.clone()))
                    .unwrap_or_else(|| agent.kind.label().into());
                let short = model.rsplit('/').next().unwrap_or(&model).to_owned();
                (crate::brand::agent_icon(agent.kind), short)
            }
            Target::Web(provider) => (crate::brand::icon(*provider), provider.label().into()),
        };
        let running = assist.running().map(|(run, time)| (crate::assist::accent(run.action(), &theme), time));
        let picker = self.picker.clone();
        Popover::new("status-ai-popup").anchor(Anchor::BottomRight).offset(px(8.))
            .open(picker.is_some())
            .on_open_change(cx.listener(|this, open, window, cx| { if *open { this.open(window, cx) } else { this.close(window, cx) } }))
            .trigger(Button::new("status-ai").ghost().xsmall().icon(icon.xsmall()).label(match running {
                Some((_, time)) => format!("{label} · {:.1}s", time.as_secs_f32()),
                None => label,
            }).when_some(running, |el, (color, _)| el.text_color(color))
                .accessibility_label("AI model")
                .tooltip_with_action("AI agent and model", &crate::actions::ConfigureAi, Some(crate::actions::WORKSPACE)))
            .content(move |_, _, _| div().w(px(380.)).max_w_full().children(picker.clone()))
            .into_any_element()
    }
}

pub struct Picker {
    workspace: WeakEntity<Workspace>,
    assist: Entity<Assist>,
    chip: WeakEntity<Chip>,
    focus: FocusHandle,
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl Picker {
    fn new(workspace: WeakEntity<Workspace>, assist: Entity<Assist>, chip: WeakEntity<Chip>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search models"));
        let subscriptions = vec![
            cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.observe(&assist, |_, _, cx| cx.notify()),
        ];
        let this = Self { workspace, assist, chip, focus: cx.focus_handle(), search, _subscriptions: subscriptions };
        this.ensure_catalog(cx);
        this
    }

    fn ensure_catalog(&self, cx: &mut Context<Self>) {
        let Some(config) = self.workspace.upgrade().map(|ws| ws.read(cx).config.clone()) else { return };
        if let Target::Agent(agent, _) = self.assist.read(cx).target(&config) {
            self.assist.update(cx, |assist, cx| assist.load_catalog(agent.kind, cx));
        }
    }

    fn update_config(&self, window: &mut Window, cx: &mut Context<Self>, change: impl FnOnce(&mut practice::config::Config)) {
        let _ = self.workspace.update(cx, |ws, cx| { change(&mut ws.config); ws.save_config(window, cx); cx.notify(); });
        self.ensure_catalog(cx);
        self.assist.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn choose_agent(&mut self, agent: Option<AgentKind>, window: &mut Window, cx: &mut Context<Self>) {
        self.update_config(window, cx, |config| { config.agent = agent; config.ai_web = agent.is_none(); });
        let label = agent.map_or("Web chat", AgentKind::label);
        let _ = self.workspace.update(cx, |ws, cx| ws.flash(format!("AI: {label}"), cx));
    }

    fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.update_config(window, cx, |config| { config.agent = Some(selection.agent); config.ai_web = false; config.remember(selection); });
    }
}

impl Focusable for Picker {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() }
}

impl Render for Picker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(workspace) = self.workspace.upgrade() else { return div().into_any_element() };
        let config = workspace.read(cx).config.clone();
        let missing = match self.assist.read(cx).target(&config) { Target::Agent(agent, _) => !self.assist.read(cx).catalogs.contains_key(&agent.kind), Target::Web(_) => false };
        if missing {
            let picker = cx.entity().downgrade();
            cx.defer(move |cx| { let _ = picker.update(cx, |this, cx| this.ensure_catalog(cx)); });
        }
        let assist = self.assist.read(cx);
        let target = assist.target(&config);
        let current = match &target { Target::Agent(agent, _) => Some(agent.kind), Target::Web(_) => None };

        let section = |text: &'static str| div().text_xs().text_color(theme.muted_foreground).child(text);

        let mut agents = h_flex().gap_1().flex_wrap();
        for kind in AgentKind::ALL {
            if let Some(agent) = assist.detected(kind) {
                let tip = format!("{}{}", kind.label(), agent.version.as_ref().map(|v| format!(" · {v}")).unwrap_or_default());
                agents = agents.child(Button::new(SharedString::from(format!("agent-{kind:?}"))).ghost().icon(crate::brand::agent_icon(kind))
                    .selected(current == Some(kind)).tooltip(tip).accessibility_label(kind.label())
                    .on_click(cx.listener(move |this, _, window, cx| this.choose_agent(Some(kind), window, cx))));
            } else if kind.installable() {
                let installing = assist.installing == Some(kind);
                agents = agents.child(Button::new(SharedString::from(format!("install-{kind:?}"))).ghost()
                    .icon(crate::brand::agent_icon(kind)).loading(installing).opacity(0.55)
                    .tooltip(if installing { format!("Installing {}…", kind.label()) } else { format!("Install {} with its official installer", kind.label()) })
                    .accessibility_label(format!("Install {}", kind.label()))
                    .on_click(cx.listener(move |this, _, window, cx| this.assist.update(cx, |assist, cx| assist.install(kind, window, cx)))));
            }
        }
        agents = agents.child(Button::new("agent-web").ghost().icon(IconName::Globe).selected(current.is_none())
            .tooltip("Web chat").accessibility_label("Web chat")
            .on_click(cx.listener(|this, _, window, cx| this.choose_agent(None, window, cx))));
        if assist.detecting { agents = agents.child(gpui_kit::component::spinner::Spinner::new().xsmall()); }

        let body = match &target {
            Target::Web(provider) => v_flex().gap_1p5().child(section("Web chat"))
                .child(h_flex().gap_1().children(Provider::ALL.into_iter().map(|p| {
                    Button::new(SharedString::from(format!("web-{p:?}"))).ghost().icon(crate::brand::icon(p)).label(p.label()).small().selected(p == *provider)
                        .on_click(cx.listener(move |this, _, window, cx| this.update_config(window, cx, |config| config.web_chat = p)))
                })))
                .when(assist.agents.is_empty() && !assist.detecting, |el| el.child(div().text_xs().text_color(theme.muted_foreground)
                    .child("No local agent found. Install OpenCode or Antigravity above for free models.")))
                .into_any_element(),
            Target::Agent(agent, selection) => match assist.catalogs.get(&agent.kind) {
                None | Some(Loadable::Loading) => h_flex().gap_2().text_xs().text_color(theme.muted_foreground)
                    .child(gpui_kit::component::spinner::Spinner::new().xsmall()).child("Loading models").into_any_element(),
                Some(Loadable::Failed(error)) => {
                    let kind = agent.kind;
                    v_flex().gap_1().child(div().text_xs().text_color(theme.danger).child(error.clone()))
                        .child(Button::new("catalog-retry").ghost().xsmall().icon(IconName::RefreshCw).label("Retry")
                            .on_click(cx.listener(move |this, _, _, cx| this.assist.update(cx, |assist, cx| assist.reload_catalog(kind, cx)))))
                        .into_any_element()
                }
                Some(Loadable::Ready(catalog)) => {
                    let chosen = selection.clone().or_else(|| catalog.default_model.as_ref().map(|model| Selection { agent: agent.kind, model: model.clone(), effort: None, fast: false }));
                    let model = chosen.as_ref().and_then(|s| catalog.models.iter().find(|m| m.id == s.model));
                    let query = self.search.read(cx).value().to_lowercase();
                    let mut models: Vec<&Model> = catalog.models.iter().filter(|m| query.is_empty() || m.id.to_lowercase().contains(&query) || m.label.to_lowercase().contains(&query)).collect();
                    models.sort_by_key(|m| (!m.free || !catalog.models.iter().any(|m| m.free), false));
                    let kind = agent.kind;
                    let rows = models.into_iter().enumerate().map(|(index, m)| {
                        let selected = chosen.as_ref().is_some_and(|s| s.model == m.id);
                        let pick = Selection { agent: kind, model: m.id.clone(), effort: m.default_effort.clone(), fast: false };
                        h_flex().id(("model", index)).px_2().py_1p5().gap_2().rounded_md().cursor_pointer().items_center()
                            .when(selected, |el| el.bg(theme.list_active)).hover(|s| s.bg(theme.list_hover))
                            .child(v_flex().min_w_0().flex_1()
                                .child(div().text_sm().truncate().child(m.label.clone()))
                                .when(!m.description.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).truncate().child(m.description.clone()))))
                            .when(m.free, |el| el.child(div().px_1p5().rounded_full().bg(theme.success.opacity(0.15)).text_size(px(10.)).text_color(theme.success).child("free")))
                            .when(m.fast, |el| el.child(Icon::new(IconName::Zap).size_3().text_color(theme.warning)))
                            .when(catalog.default_model.as_deref() == Some(m.id.as_str()), |el| el.child(div().text_size(px(10.)).text_color(theme.muted_foreground).child("default")))
                            .when(selected, |el| el.child(Icon::new(IconName::Check).size_3p5().text_color(theme.primary)))
                            .on_click(cx.listener(move |this, _, window, cx| this.select(pick.clone(), window, cx)))
                    });
                    v_flex().gap_2()
                        .when(catalog.models.len() > 8, |el| el.child(Input::new(&self.search).small().prefix(Icon::new(IconName::Search).xsmall())))
                        .child(v_flex().id("model-list").max_h(px(240.)).overflow_y_scroll().gap_0p5().children(rows))
                        .when_some(catalog.sign_in_hint.clone(), |el, hint| el.child(div().text_xs().text_color(theme.warning).child(hint)))
                        .when_some(model.filter(|m| !m.efforts.is_empty()).zip(chosen.clone()), |el, (m, chosen)| {
                            let effort = chosen.effort.clone().or_else(|| m.default_effort.clone());
                            el.child(section("Reasoning")).child(h_flex().gap_1().flex_wrap().children(m.efforts.iter().map(|e| {
                                let mut next = chosen.clone();
                                next.effort = Some(e.id.clone());
                                Button::new(SharedString::from(format!("effort-{}", e.id))).ghost().xsmall().label(e.id.clone()).tooltip(e.label.clone())
                                    .selected(effort.as_deref() == Some(e.id.as_str()))
                                    .on_click(cx.listener(move |this, _, window, cx| this.select(next.clone(), window, cx)))
                            })))
                        })
                        .when_some(model.filter(|m| m.fast).zip(chosen.clone()), |el, (_, chosen)| {
                            let mut next = chosen.clone();
                            next.fast = !chosen.fast;
                            el.child(Button::new("fast-tier").ghost().small().icon(Icon::new(IconName::Zap)).label(if chosen.fast { "Fast tier on" } else { "Fast tier off" })
                                .selected(chosen.fast).tooltip("Faster responses; uses more of your plan")
                                .on_click(cx.listener(move |this, _, window, cx| this.select(next.clone(), window, cx))))
                        })
                        .into_any_element()
                }
            },
        };
        v_flex().key_context("AiPicker").track_focus(&self.focus).gap_3()
            .on_action(cx.listener(|this, _: &CloseAi, window, cx| { let _ = this.chip.update(cx, |chip, cx| chip.close(window, cx)); }))
            .child(h_flex().justify_between().items_center()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("AI"))
                .child(Button::new("agents-refresh").ghost().xsmall().icon(IconName::RefreshCw).tooltip("Detect agents and refresh models")
                    .on_click(cx.listener(|this, _, _, cx| this.assist.update(cx, |assist, cx| { assist.catalogs.clear(); assist.detect(cx); }) ))))
            .child(agents)
            .child(body)
            .into_any_element()
    }
}
