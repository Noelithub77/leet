//! Source selection with a default NeetCode action and hover list choices.
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::language::Source;
use practice::roadmap::List;
use crate::workspace::{Focus, Workspace};

const LISTS: [List; 3] = [List::NeetCode150, List::NeetCode250, List::All];

#[derive(Default)]
pub struct SourceMenuState {
    anchor: Option<SourceMenuAnchor>,
    menu: Option<Entity<SourceMenu>>,
    return_focus: Option<Focus>,
}

impl SourceMenuState {
    pub fn hide(&mut self) { self.anchor = None; self.menu = None; self.return_focus = None; }
}

fn close(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let focus = ws.sources.menu.return_focus;
    ws.sources.menu.hide();
    if let Some(focus) = focus {
        match focus {
            Focus::Editor | Focus::Omnibar => ws.focus_editor(window, cx),
            focus => ws.focus_nav(focus, window, cx),
        }
    }
    cx.notify();
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SourceMenuAnchor { Home, Explorer }

pub fn render(ws: &Workspace, anchor: SourceMenuAnchor, label: String, cx: &mut Context<Workspace>) -> impl IntoElement {
    let menu = ws.sources.menu.menu.clone();
    let current = ws.config.source;
    let id = match anchor { SourceMenuAnchor::Home => "home-practice-source-popup", SourceMenuAnchor::Explorer => "explorer-practice-source-popup" };
    Popover::new(id).anchor(Anchor::TopLeft).offset(px(6.))
        .open(ws.sources.menu.anchor == Some(anchor))
        .on_open_change(cx.listener(move |ws, open: &bool, window, cx| {
            if !*open {
                if ws.sources.menu.anchor == Some(anchor) { close(ws, window, cx); }
                return;
            }
            ws.sources.menu.return_focus = Some(ws.focus_area);
            let workspace = cx.weak_entity();
            let selection = MenuSelection::new(ws.config.source, ws.config.roadmap_list);
            let menu = cx.new(|cx| SourceMenu { workspace, focus: cx.focus_handle(), selection });
            let focus = menu.read(cx).focus.clone();
            ws.sources.menu.menu = Some(menu);
            ws.sources.menu.anchor = Some(anchor);
            cx.notify();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }))
        .trigger(Button::new("practice-source").ghost().small().label(label).icon(crate::brand::source_icon(current).mr_1())
            .child(Icon::new(IconName::ChevronDown).xsmall())
            .accessibility_label("Practice source").tooltip_with_action("Practice source", &crate::actions::CycleSource, Some(crate::actions::WORKSPACE)))
        .content(move |_, _, _| div().children(menu.clone()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice { Source(Source), List(List) }

struct MenuSelection {
    source: usize,
    list: usize,
    lists_open: bool,
    in_lists: bool,
}

impl MenuSelection {
    fn new(source: Source, list: List) -> Self {
        Self { source: Source::ALL.iter().position(|item| *item == source).unwrap_or(0),
            list: LISTS.iter().position(|item| *item == list).unwrap_or(0), lists_open: false, in_lists: false }
    }

    fn hover_source(&mut self, index: usize) {
        self.source = index;
        self.lists_open = Source::ALL[index] == Source::NeetCode;
        self.in_lists = false;
    }

    fn choice(&self) -> Choice {
        if self.in_lists { Choice::List(LISTS[self.list]) } else { Choice::Source(Source::ALL[self.source]) }
    }

    fn step(&mut self, delta: isize) {
        if self.in_lists { self.list = (self.list as isize + delta).rem_euclid(LISTS.len() as isize) as usize; }
        else {
            self.source = (self.source as isize + delta).rem_euclid(Source::ALL.len() as isize) as usize;
            self.lists_open = false;
        }
    }
}

struct SourceMenu {
    workspace: WeakEntity<Workspace>,
    focus: FocusHandle,
    selection: MenuSelection,
}

impl SourceMenu {
    fn navigate(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        match key {
            "up" => self.selection.step(-1),
            "down" => self.selection.step(1),
            "right" if Source::ALL[self.selection.source] == Source::NeetCode => {
                self.selection.lists_open = true; self.selection.in_lists = true;
            }
            "left" => { self.selection.lists_open = false; self.selection.in_lists = false; }
            "enter" => self.choose(self.selection.choice(), window, cx),
            "escape" => { let _ = self.workspace.update(cx, |ws, cx| close(ws, window, cx)); }
            _ => {},
        }
        cx.stop_propagation(); cx.notify();
    }

    fn choose(&self, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.workspace.update(cx, |ws, cx| {
            let source = match choice {
                Choice::Source(source) => source,
                Choice::List(list) => {
                    ws.config.roadmap_list = list;
                    ws.rebuild_library();
                    Source::NeetCode
                }
            };
            ws.choose_source(source, window, cx);
        });
    }
}

impl Render for SourceMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.workspace.upgrade();
        let config = workspace.as_ref().map(|ws| &ws.read(cx).config);
        let current = config.map_or(Source::NeetCode, |config| config.source);
        let list = config.map_or(List::NeetCode150, |config| config.roadmap_list);
        let theme = cx.theme().clone();
        h_flex().items_start().gap_2().key_context("PracticeSourceMenu").track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &crate::actions::Up, window, cx| this.navigate("up", window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::Down, window, cx| this.navigate("down", window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::Right, window, cx| this.navigate("right", window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::Left, window, cx| this.navigate("left", window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::Confirm, window, cx| this.navigate("enter", window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::Back, window, cx| this.navigate("escape", window, cx)))
            .child(v_flex().w(px(170.)).gap_0p5().children(Source::ALL.into_iter().enumerate().map(|(index, source)| {
                Button::new(("source-option", index)).ghost().small().w_full().justify_start().h(px(28.))
                    .icon(crate::brand::source_icon(source))
                    .label(if source == Source::NeetCode { list.label() } else { source.label() })
                    .selected(!self.selection.in_lists && self.selection.source == index)
                    .child(div().flex_1())
                    .when(source == Source::NeetCode, |button| button.child(Icon::new(IconName::ChevronRight).xsmall()))
                    .when(current == source && source != Source::NeetCode, |button| button.child(Icon::new(IconName::Check).xsmall()))
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        if *hovered { this.selection.hover_source(index); cx.notify(); }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| this.choose(Choice::Source(source), window, cx)))
            })))
            .when(self.selection.lists_open, |row| row.child(v_flex().w(px(155.)).gap_0p5().pl_2().border_l_1().border_color(theme.border)
                .children(LISTS.into_iter().enumerate().map(|(index, choice)| {
                    Button::new(("neetcode-list-option", index)).ghost().small().w_full().justify_start().h(px(28.))
                        .label(choice.label()).selected(self.selection.in_lists && self.selection.list == index)
                        .child(div().flex_1())
                        .when(list == choice, |button| button.child(Icon::new(IconName::Check).xsmall()))
                        .on_hover(cx.listener(move |this, hovered, _, cx| {
                            if *hovered { this.selection.list = index; this.selection.in_lists = true; cx.notify(); }
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| this.choose(Choice::List(choice), window, cx)))
                }))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn hovering_neetcode_keeps_click_on_saved_source_and_right_arrow_selects_saved_list() {
        for list in LISTS {
            let mut menu = MenuSelection::new(Source::CodeChef, list);
            menu.hover_source(0);
            assert!(menu.lists_open);
            assert_eq!(menu.choice(), Choice::Source(Source::NeetCode));
            menu.in_lists = true;
            assert_eq!(menu.choice(), Choice::List(list));
            menu.hover_source(2);
            assert!(!menu.lists_open);
            assert_eq!(menu.choice(), Choice::Source(Source::Codeforces));
        }
    }

    #[::core::prelude::v1::test]
    fn arrows_wrap_in_each_column_without_selecting_a_list_when_moving_sources() {
        let mut menu = MenuSelection::new(Source::NeetCode, List::NeetCode150);
        menu.step(-1);
        assert_eq!(menu.choice(), Choice::Source(Source::CodeChef));
        menu.step(1);
        assert_eq!(menu.choice(), Choice::Source(Source::NeetCode));
        assert!(!menu.lists_open);
        menu.in_lists = true;
        menu.step(-1);
        assert_eq!(menu.choice(), Choice::List(List::All));
        menu.step(1);
        assert_eq!(menu.choice(), Choice::List(List::NeetCode150));
    }
}
