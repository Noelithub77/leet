//! A cached problem view with semantic examples and lazy reference solutions.
use std::sync::Arc;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::text::{TextView, TextViewStyle};
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _, WindowExt as _, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::{db::Db, description::{Block, Part}, language::Language, solutions::{Article, Reference}};
use crate::view::key;

pub struct TagsVisible(pub bool);
impl Global for TagsVisible {}
pub struct Sections(pub [bool; 3]);
impl Default for Sections { fn default() -> Self { Self([true, false, false]) } }
impl Global for Sections {}
fn tag_icon(topic: &str) -> IconName {
    match topic {
        "Array" | "Hash Table" | "String" => IconName::Brackets,
        "Math" | "Geometry" => IconName::Calculator,
        "Sorting" => IconName::ArrowDownAZ,
        "Divide and Conquer" | "Recursion" => IconName::GitBranch,
        "Dynamic Programming" => IconName::Grid2x2,
        "Two Pointers" => IconName::ArrowLeftRight,
        "Sliding Window" => IconName::PanelTop,
        "Linked List" => IconName::Link,
        "Tree" | "Binary Tree" | "Binary Search Tree" => IconName::Network,
        "Graph" | "Breadth-First Search" | "Depth-First Search" => IconName::Share2,
        "Stack" | "Monotonic Stack" => IconName::Layers,
        "Heap (Priority Queue)" => IconName::ListOrdered,
        "Binary Search" => IconName::Search,
        "Backtracking" => IconName::Undo2,
        "Greedy" => IconName::Zap,
        "Bit Manipulation" => IconName::Binary,
        _ => IconName::Tags,
    }
}

#[derive(Default)]
pub struct Statement {
    pub(crate) focus: Option<FocusHandle>,
    pub(crate) scroll: ScrollHandle,
    pub slug: SharedString,
    pub title: SharedString,
    pub blocks: Vec<Block>,
    pub language: Language,
    pub db: Option<Arc<Db>>,
    pub topics: Vec<SharedString>,
    pub hints: Vec<SharedString>,
    pub hints_shown: usize,
    pub video: Option<String>,
    pub status: Option<SharedString>,
    pub(crate) reference_open: bool,
    pub(crate) reference: Option<Reference>,
    pub(crate) article: Option<Article>,
    pub(crate) reference_status: Option<String>,
    pub(crate) reference_loading: bool,
    pub(crate) reference_sequence: u64,
}
gpui_kit::actions!(statement, [PageUp, PageDown]);
pub fn bind_keys(cx: &mut App) { cx.bind_keys([KeyBinding::new("pageup", PageUp, Some("Statement")), KeyBinding::new("pagedown", PageDown, Some("Statement"))]); }
impl Statement {
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.get_or_insert_with(|| cx.focus_handle()).clone().focus(window, cx);
    }
    fn scroll_by(&self, delta: f32, cx: &mut Context<Self>) { let mut offset = self.scroll.offset(); offset.y += px(delta); self.scroll.set_offset(offset); cx.notify(); }
    pub fn toggle_reference(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reference_open = !self.reference_open;
        if self.reference_open && self.reference.is_none() { self.load_reference(window, cx); }
        cx.notify();
    }
    fn load_reference(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.reference_loading { return; }
        let Some(db) = self.db.clone() else { return; };
        self.reference_loading = true; self.reference_status = None; self.reference_sequence += 1;
        let sequence = self.reference_sequence; let slug = self.slug.clone(); let language = self.language;
        cx.spawn_in(window, async move |this, cx| {
            let query_slug = slug.clone();
            let result = cx.background_spawn(async move { (practice::solutions::article(&db, &query_slug, language), practice::solutions::get(&db, &query_slug, language)) }).await;
            let _ = this.update(cx, |this, cx| {
                if this.slug != slug || this.language != language || this.reference_sequence != sequence { return; }
                this.reference_loading = false;
                let (article, reference) = result;
                this.article = article.ok();
                match reference { Ok(reference) => this.reference = Some(reference), Err(error) if this.article.is_none() => this.reference_status = Some(error.to_string()), Err(_) => {} }
                cx.notify();
            });
        }).detach(); cx.notify();
    }
    fn select_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        if self.language == language { return; }
        self.language = language; self.reference = None; self.article = None; self.reference_loading = false; self.load_reference(window, cx);
    }
    fn markdown(&self, id: String, content: String, color: Hsla) -> impl IntoElement {
        TextView::markdown(SharedString::from(id), content).selectable(true)
            .style(TextViewStyle::default().paragraph_gap(rems(0.65)).inline_code(HighlightStyle { color: Some(color), ..Default::default() }))
    }
    fn render_blocks(&self, section: usize, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lavender = rgb(if theme.mode.is_dark() { 0xd8b4fe } else { 0x9561b7 });
        v_flex().gap_5().children(self.blocks.iter().enumerate().filter(|(_, block)| match block {
            Block::Markdown(_) => section == 0, Block::Example { .. } => section == 1, Block::Constraints(_) => section == 2,
        }).map(|(index, block)| {
            let id = format!("statement-{}-{index}", self.slug);
            match block {
                Block::Markdown(markdown) => self.markdown(id, markdown.clone(), theme.primary).into_any_element(),
                Block::Constraints(markdown) => v_flex().gap_2().p_3().rounded_lg().border_1().border_color(lavender.opacity(0.35)).bg(lavender.opacity(0.06)).child(div().text_color(lavender).font_weight(FontWeight::SEMIBOLD).child("Constraints"))
                    .child(self.markdown(id, markdown.clone(), lavender.into())).into_any_element(),
                Block::Example { title, parts } => v_flex().gap_2().child(div().font_weight(FontWeight::SEMIBOLD).child(title.clone()))
                    .child(v_flex().p_3().gap_3().rounded_lg().bg(theme.muted).children(parts.iter().enumerate().map(|(part_index, part)| {
                        let id = format!("{id}-{part_index}");
                        match part {
                            Part::Markdown(markdown) => self.markdown(id, markdown.clone(), theme.primary).into_any_element(),
                            Part::Field { label, value } => {
                                let color = match label.as_str() { "Input" => rgb(if theme.mode.is_dark() { 0xa0c4ff } else { 0x536aae }).into(), "Output" => rgb(if theme.mode.is_dark() { 0x99ffe4 } else { 0x26786b }).into(), _ => theme.muted_foreground };
                                let content = if label == "Explanation" { value.clone() } else { format!("```python\n{value}\n```") };
                                v_flex().gap_1().child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color).child(label.clone()))
                                    .child(self.markdown(id, content, color)).into_any_element()
                            }
                        }
                    }))).into_any_element(),
            }
        }))
    }
    fn render_sections(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = cx.global::<Sections>().0;
        v_flex().gap_3().children(["Description", "Examples", "Constraints"].into_iter().enumerate().filter_map(|(section, label)| {
            let exists = self.blocks.iter().any(|block| match block {
                Block::Markdown(_) => section == 0, Block::Example { .. } => section == 1, Block::Constraints(_) => section == 2,
            });
            exists.then(|| v_flex().gap_3()
                .child(Button::new(("statement-section", section)).ghost().small().label(label)
                    .icon(if sections[section] { IconName::ChevronDown } else { IconName::ChevronRight }).selected(sections[section])
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let mut sections = cx.global::<Sections>().0;
                        sections[section] = !sections[section];
                        let Some(db) = &this.db else { return; };
                        if let Err(error) = db.save_statement_sections(sections) {
                            window.push_notification(gpui_kit::component::notification::Notification::error(format!("Section preferences not saved: {error}")), cx);
                            return;
                        }
                        cx.set_global(Sections(sections));
                        cx.refresh_windows();
                    })))
                .when(sections[section], |view| view.child(self.render_blocks(section, cx))))
        }))
    }
    fn render_reference(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone(); let weak = cx.weak_entity(); let language = self.language;
        v_flex().gap_3().child(h_flex().gap_2()
            .child(Button::new("reference-language").ghost().small().label(language.label()).icon(IconName::ChevronDown)
                .dropdown_menu(move |mut menu, _, _| {
                    for choice in Language::ALL {
                        let weak = weak.clone();
                        menu = menu.item(PopupMenuItem::new(choice.label()).checked(language == choice).on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |this, cx| this.select_language(choice, window, cx));
                        }));
                    } menu
                }))
            .child(div().flex_1())
            .when_some(self.reference.clone(), |row, reference| {
                let url = reference.url.clone(); let code = reference.code;
                row.child(Button::new("copy-reference").ghost().xsmall().icon(IconName::Copy).accessibility_label("Copy reference solution").tooltip("Copy reference solution")
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(code.clone()))))
                    .child(Button::new("reference-source").ghost().xsmall().icon(IconName::ExternalLink).accessibility_label("NeetCode source").tooltip("NeetCode source")
                        .on_click(move |_, _, _| { let _ = open::that_detached(&url); }))
            }))
            .when(self.reference_loading, |view| view.child(div().text_color(theme.muted_foreground).child("Loading solution…")))
            .when_some(self.reference_status.clone(), |view, status| view.child(h_flex().gap_2().child(div().flex_1().text_color(theme.muted_foreground).child(status))
                .child(Button::new("retry-reference").ghost().xsmall().icon(IconName::RefreshCw).accessibility_label("Retry solution fetch").tooltip("Retry solution fetch")
                    .on_click(cx.listener(|this, _, window, cx| this.load_reference(window, cx))))))
            .when_some(self.article.as_ref(), |view, article| view
                .children(article.videos.iter().enumerate().map(|(index, url)| {
                    let url = url.clone();
                    Button::new(("solution-video", index)).outline().icon(IconName::Play).label("Video explanation")
                        .tooltip("Watch the complete explanation in your browser")
                        .on_click(move |_, _, _| { let _ = open::that_detached(&url); })
                }))
                .child(self.markdown(format!("solution-article-{}-{}", self.slug, language.id()), article.markdown.clone(), rgb(0xa0c4ff).into()))
                .child(Button::new("article-source").ghost().xsmall().label(if self.slug.starts_with("cf:") { "Codeforces · Open-R1 editorial" } else { "NeetCode · Full article" }).icon(IconName::ExternalLink)
                    .on_click({ let url = article.url.clone(); move |_, _, _| { let _ = open::that_detached(&url); } })))
            .when_some(self.reference.as_ref().filter(|_| self.article.is_none()), |view, reference| view
                .child(self.markdown(format!("reference-{}-{}", self.slug, language.id()), format!("````{}\n{}\n````", reference.language.id(), reference.code), theme.primary))
                .child(h_flex().gap_1().text_xs().text_color(theme.muted_foreground).child("NeetCode · MIT")
                    .child(Button::new("reference-license").ghost().xsmall().icon(IconName::Info).accessibility_label("Reference license").tooltip("Reference license")
                        .on_click(|_, window, cx| window.open_dialog(cx, |dialog, _, _| dialog.title("NeetCode reference license").w(px(600.)).child(
                            div().id("reference-license-text").max_h(px(440.)).overflow_y_scroll().child(TextView::markdown("neetcode-license", practice::solutions::LICENSE).selectable(true))))))))
    }
}
impl Render for Statement {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let show_tags = cx.try_global::<TagsVisible>().is_some_and(|visible| visible.0);
        let focus = self.focus.get_or_insert_with(|| cx.focus_handle()).clone();
        let body = v_flex().id("statement").key_context("Statement").track_focus(&focus).size_full().min_h_0()
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                this.scroll_by(f32::from(event.delta.pixel_delta(window.line_height()).y) * 2.5, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &crate::actions::Up, _, cx| this.scroll_by(90., cx)))
            .on_action(cx.listener(|this, _: &crate::actions::Down, _, cx| this.scroll_by(-90., cx)))
            .on_action(cx.listener(|this, _: &PageUp, _, cx| this.scroll_by(520., cx)))
            .on_action(cx.listener(|this, _: &PageDown, _, cx| this.scroll_by(-520., cx)));
        if self.slug.is_empty() { return body.child(div().text_color(theme.muted_foreground).child("Open a problem from the roadmap or search.")); }
        if let Some(status) = &self.status { return body.child(div().text_color(theme.muted_foreground).child(status.clone())); }
        body.child(v_flex().id("statement-content").flex_1().min_h_0().track_scroll(&self.scroll).overflow_y_scroll().p_4().gap_4()
            .child(div().text_size(px(24.)).font_weight(FontWeight::BOLD).child(self.title.clone()))
            .child(h_flex().gap_1()
                .child(Button::new("statement-question").ghost().small().selected(!self.reference_open).icon(IconName::FileText).accessibility_label("Question").tooltip("Question")
                    .on_click(cx.listener(|this, _, _, cx| { this.reference_open = false; cx.notify(); })))
                .child(Button::new("statement-solution").ghost().small().selected(self.reference_open).icon(IconName::Lightbulb).accessibility_label("Solution").tooltip_with_action("Solution", &crate::actions::ToggleReference, Some(crate::actions::WORKSPACE))
                    .on_click(cx.listener(|this, _, window, cx| { this.reference_open = true; if this.reference.is_none() { this.load_reference(window, cx); } cx.notify(); })))
                .when(!self.topics.is_empty(), |row| row.child(Button::new("statement-tags").ghost().small().selected(show_tags).icon(IconName::Tags).accessibility_label("Show or hide tags").tooltip_with_action("Show/hide tags", &crate::actions::ToggleTags, Some(crate::actions::WORKSPACE))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::actions::ToggleTags), cx))))
                .when_some(self.video.clone(), |row, url| row.child(Button::new("statement-video").ghost().small().icon(IconName::Play)
                    .accessibility_label("Watch video explanation").tooltip("Watch video explanation")
                    .on_click(move |_, _, _| { let _ = open::that_detached(&url); }))))
            .child(if self.reference_open { self.render_reference(cx).into_any_element() } else {
                v_flex().gap_4().when(show_tags, |view| view.child(h_flex().flex_wrap().gap_1().children(self.topics.iter().map(|topic| {
                    h_flex().gap_1().px_1p5().py_0p5().rounded_md().bg(theme.muted).text_xs().text_color(theme.muted_foreground)
                        .child(Icon::new(tag_icon(topic)).size_3().text_color(rgb(0xa0c4ff))).child(topic.clone())
                })))).child(self.render_sections(cx))
                    .children(self.hints.iter().take(self.hints_shown).enumerate().map(|(index, hint)| {
                        v_flex().p_3().gap_1().rounded_md().bg(theme.muted)
                            .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.primary).child(format!("Hint {}", index + 1)))
                            .child(self.markdown(format!("hint-{}-{index}", self.slug), hint.to_string(), theme.primary))
                    }))
                    .into_any_element()
            }))
            .when(!self.reference_open && self.hints_shown < self.hints.len(), |view| view.child(
                h_flex().flex_shrink_0().px_4().py_2().border_t_1().border_color(theme.border).text_xs().text_color(theme.muted_foreground)
                    .child(h_flex().gap_1().child(key("ctrl-alt-h")).child("hint"))))
    }
}
