//! Caret-scoped language-server signatures, sharing the completion surface.
use std::{ops::Range, time::Duration};
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::highlighter::SyntaxHighlighter;
use gpui_kit::component::text::TextView;
use gpui_kit::prelude::FluentBuilder as _;
use lsp_wire::{SignatureHelp, SignatureHelpContext, SignatureHelpTriggerKind, Documentation};
use practice::{language::Language, lsp::signature};
use super::expansion::EditorPane;
gpui_kit::actions!(signature_help, [PreviousSignature, NextSignature]);

#[derive(Clone, PartialEq)]
pub(super) struct Input {
    source: String,
    offset: usize,
    selection: Range<usize>,
    language: Language,
    call: Option<usize>,
}

pub(super) struct Hints {
    pub focused: bool,
    pub input: Option<Input>,
    help: Option<SignatureHelp>,
    pending: bool,
    epoch: u64,
    task: Task<()>,
    dismissed: Option<usize>,
    parser: SyntaxHighlighter,
    parsed: String,
    language: Language,
    #[cfg(feature = "gui-test")]
    pub fixture: Option<std::rc::Rc<dyn Fn(String, usize, SignatureHelpContext, &mut App) -> Task<anyhow::Result<Option<SignatureHelp>>>>>,
}

impl Hints {
    pub fn new(language: Language, focused: bool) -> Self {
        Self { focused, input: None, help: None, pending: false, epoch: 0, task: Task::ready(()), dismissed: None, parser: SyntaxHighlighter::new(language.id()), parsed: String::new(), language,
            #[cfg(feature = "gui-test")] fixture: None }
    }
    pub(super) fn has_overloads(&self) -> bool { self.focused && self.help.as_ref().is_some_and(|help| help.signatures.len() > 1) }
    fn call(&mut self, source: &str, offset: usize, language: Language) -> Option<usize> {
        if self.language != language { self.parser = SyntaxHighlighter::new(language.id()); self.language = language; self.parsed.clear(); }
        if self.parsed != source {
            if !self.parser.update(None, &ropey::Rope::from_str(source), Some(Duration::from_millis(3))) { self.parsed.clear(); return None; }
            self.parsed = source.to_owned();
        }
        if let Some(call) = self.call_in_tree(source, offset) { return Some(call); }
        if !self.parser.tree()?.root_node().has_error() { return None; }
        // Recover pasted or temporarily incomplete calls without altering the document.
        let recovered = format!("{}){}", source.get(..offset)?, source.get(offset..)?);
        if !self.parser.update(None, &ropey::Rope::from_str(&recovered), Some(Duration::from_millis(3))) { self.parsed.clear(); return None; }
        self.parsed = recovered.clone();
        self.call_in_tree(&recovered, offset)
    }
    fn call_in_tree(&self, source: &str, offset: usize) -> Option<usize> {
        let start = source.get(..offset)?.char_indices().last().map(|(ix, _)| ix).unwrap_or(0);
        let mut node = self.parser.tree()?.root_node().descendant_for_byte_range(start, offset);
        while let Some(current) = node {
            if current.kind().contains("comment") { return None; }
            if matches!(current.kind(), "call" | "call_expression" | "method_invocation" | "object_creation_expression") {
                if let Some(arguments) = current.child_by_field_name("arguments") {
                    let end = arguments.end_byte();
                    let missing_close = !source.get(arguments.start_byte()..end)?.ends_with(')');
                    if offset > arguments.start_byte() && (offset < end || (missing_close && offset == end)) { return Some(arguments.start_byte()); }
                }
            }
            node = current.parent();
        }
        None
    }
}

impl EditorPane {
    pub(super) fn hide_signature(&mut self, dismiss: bool, cx: &mut Context<Self>) -> bool {
        let visible = self.signature.help.is_some() || self.signature.pending;
        if dismiss { self.signature.dismissed = self.signature.input.as_ref().and_then(|input| input.call); }
        self.signature.epoch = self.signature.epoch.wrapping_add(1);
        self.signature.task = Task::ready(());
        self.signature.help = None;
        self.signature.pending = false;
        if visible { cx.notify(); }
        visible
    }

    pub(super) fn refresh_signature(&mut self, manual: bool, cx: &mut Context<Self>) {
        let document = self.language_document.clone().filter(|document| document.server.signature_help_options().is_some());
        #[cfg(feature = "gui-test")]
        let fixture = self.signature.fixture.clone();
        let available = document.is_some();
        #[cfg(feature = "gui-test")]
        let available = available || fixture.is_some();
        if !available || !self.signature.focused { self.hide_signature(false, cx); return; }
        let editor = self.state.read(cx);
        let source = editor.value().to_string(); let offset = editor.cursor(); let selection = editor.selected_range();
        if !selection.is_empty() { self.hide_signature(false, cx); self.signature.input = None; return; }
        let unchanged = self.signature.input.as_ref().is_some_and(|input| input.source == source && input.offset == offset && input.selection == selection && input.language == self.language);
        if unchanged && !manual { return; }
        let call = if unchanged { self.signature.input.as_ref().and_then(|input| input.call) } else { self.signature.call(&source, offset, self.language) };
        let input = Input { source, offset, selection, language: self.language, call };
        let same_call = self.signature.input.as_ref().is_some_and(|previous| previous.call == call);
        self.signature.input = Some(input.clone());
        if manual { self.signature.dismissed = None; }
        else if self.signature.dismissed != call { self.signature.dismissed = None; }
        if !manual && (call.is_none() || self.signature.dismissed == call) { self.hide_signature(false, cx); return; }
        if !same_call { self.hide_signature(false, cx); }
        self.signature.epoch = self.signature.epoch.wrapping_add(1);
        let epoch = self.signature.epoch;
        self.signature.pending = true;
        let active = self.signature.help.clone();
        let trigger = input.source[..offset].chars().last().map(|ch| ch.to_string());
        let options = document.as_ref().and_then(|document| document.server.signature_help_options());
        let triggered = !manual && trigger.as_ref().is_some_and(|trigger| options.as_ref().is_some_and(|options|
            options.trigger_characters.as_ref().is_some_and(|characters| characters.contains(trigger))
            || options.retrigger_characters.as_ref().is_some_and(|characters| characters.contains(trigger))));
        let context = SignatureHelpContext { trigger_kind: if manual { SignatureHelpTriggerKind::INVOKED } else if triggered { SignatureHelpTriggerKind::TRIGGER_CHARACTER } else { SignatureHelpTriggerKind::CONTENT_CHANGE },
            trigger_character: triggered.then_some(trigger).flatten(), is_retrigger: active.is_some() && same_call, active_signature_help: active };
        self.signature.task = cx.spawn(async move |this, cx| {
            if !manual { cx.background_executor().timer(Duration::from_millis(120)).await; }
            let source = input.source.clone();
            let result = if let Some(document) = document {
                cx.background_spawn(async move { document.signature_help(source, offset, context).await }).await
            } else {
                #[cfg(feature = "gui-test")]
                { let task = this.update(cx, |_, cx| fixture.as_ref().map(|provider| provider(source, offset, context, cx))).ok().flatten();
                    match task { Some(task) => task.await, None => Ok(None) } }
                #[cfg(not(feature = "gui-test"))]
                { Ok(None) }
            };
            let _ = this.update(cx, |pane, cx| {
                if pane.signature.epoch != epoch || !pane.signature.focused || pane.signature.input.as_ref() != Some(&input) { return; }
                let editor = pane.state.read(cx);
                if editor.value().as_str() != input.source || editor.cursor() != offset || !editor.selected_range().is_empty() { return; }
                pane.signature.help = result.ok().flatten().filter(|help| !help.signatures.is_empty());
                pane.signature.pending = false;
                cx.notify();
            });
        });
    }

    pub(super) fn step_signature(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(help) = &mut self.signature.help {
            let current = signature::active_signature(help);
            help.active_signature = Some((current as isize + delta).rem_euclid(help.signatures.len() as isize) as u32);
            self.state.read(cx).focus_handle(cx).focus(window, cx); cx.notify();
        }
    }
}

pub(super) fn height(pane: &EditorPane, window: &Window) -> Pixels {
    if pane.signature.help.is_some() { rems(4.5).to_pixels(window.rem_size()) } else { px(0.) }
}

pub(super) fn render_content(pane: &EditorPane, cx: &mut Context<EditorPane>) -> Option<AnyElement> {
    let help = pane.signature.help.as_ref()?;
    let ix = signature::active_signature(help); let information = help.signatures.get(ix)?;
    let theme = cx.theme().clone();
    let highlights = signature::parameter_range(help, information).map(|range| (range, HighlightStyle { color: Some(theme.primary), font_weight: Some(FontWeight::BOLD), ..Default::default() }));
    let documentation = signature::active_parameter(help, information).and_then(|parameter| parameter.documentation.as_ref()).or(information.documentation.as_ref());
    let documentation = documentation.and_then(|doc| {
        let text = match doc { Documentation::String(text) => text, Documentation::MarkupContent(markup) => &markup.value };
        if text.trim().is_empty() { return None; }
        Some(match doc {
            Documentation::MarkupContent(markup) if markup.kind == lsp_wire::MarkupKind::Markdown => TextView::markdown("signature-documentation", text.clone()).into_any_element(),
            _ => div().text_size(rems(11. / 16.)).text_color(theme.muted_foreground).child(text.clone()).into_any_element(),
        })
    });
    Some(v_flex().id("parameter-hints").test_support().w_full().min_w_0().gap_1().p_2()
        .child(h_flex().w_full().gap_1()
            .child(div().flex_1().text_size(rems(10. / 16.)).text_color(theme.muted_foreground).child("Parameters"))
            .when(help.signatures.len() > 1, |row| row
                .child(Button::new("previous-signature").ghost().xsmall().icon(gpui_kit::assets::IconName::ChevronLeft).accessibility_label("Previous signature").tooltip_with_action("Previous signature", &PreviousSignature, Some("SignatureHints")).on_click(|_, window, cx| window.dispatch_action(Box::new(PreviousSignature), cx)))
                .child(div().text_size(rems(10. / 16.)).text_color(theme.muted_foreground).child(format!("{}/{}", ix + 1, help.signatures.len())))
                .child(Button::new("next-signature").ghost().xsmall().icon(gpui_kit::assets::IconName::ChevronRight).accessibility_label("Next signature").tooltip_with_action("Next signature", &NextSignature, Some("SignatureHints")).on_click(|_, window, cx| window.dispatch_action(Box::new(NextSignature), cx)))))
        .child(div().id("signature-label").test_support().w_full().min_w_0().font_family(theme.mono_font_family.clone()).text_size(rems(11. / 16.)).text_color(theme.foreground)
            .child(StyledText::new(information.label.clone()).with_highlights(highlights)))
        .when_some(documentation, |view, documentation| view.child(documentation))
        .into_any_element())
}

#[cfg(feature = "gui-test")]
#[path = "signature_fixture.rs"]
pub(crate) mod fixture;

pub(super) fn render_popup(pane: &EditorPane, window: &mut Window, cx: &mut Context<EditorPane>) -> Option<AnyElement> {
    if pane.menu.as_ref().is_some_and(|menu| !menu.content.items.is_empty()) || !pane.signature.focused || pane.predictions.as_ref().is_some_and(|predictions| predictions.alt.get()) { return None; }
    let content = render_content(pane, cx)?;
    let theme = cx.theme().clone(); let editor = pane.state.read(cx);
    let (caret, line_height) = editor.cursor_layout()?; let input = editor.input_bounds();
    let position = editor.scroll_offset() + caret.origin - input.origin;
    let gap = rems(0.25).to_pixels(window.rem_size());
    let width = px(300.).min(input.size.width);
    let above = (position.y - gap).max(px(0.)); let below = position.y + line_height + gap;
    let below_space = (input.size.height - below).max(px(0.));
    let desired = height(pane, window); let use_below = below_space >= desired || below_space >= above;
    let height = desired.min(if use_below { below_space } else { above });
    if height < rems(1.5).to_pixels(window.rem_size()) { return None; }
    let left = (position.x + px(12.)).min((input.size.width - width).max(px(0.)));
    let top = if use_below { below } else { above - height };
    Some(deferred(anchored().position(input.origin + point(left, top)).child(div().id("signature-popup").test_support()
        .w(width).h(height).overflow_hidden().bg(theme.popover.opacity(0.97)).border_1().border_color(theme.border).rounded_md().shadow_md()
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(div().id("signature-scroll").size_full().overflow_y_scroll().child(content)))).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn call_context_follows_nested_calls_and_excludes_definitions_and_strings() {
        for (language, source) in [
            (Language::Python, "outer(inner(1), 2)"),
            (Language::Cpp, "int main() { outer(inner(1), 2); }"),
            (Language::C, "void f() { outer(inner(1), 2); }"),
            (Language::Java, "class A { void f() { outer(inner(1), 2); } }"),
            (Language::Go, "package main\nfunc f() { outer(inner(1), 2) }"),
        ] {
            let mut hints = Hints::new(language, true);
            let inner = source.find("inner(").unwrap() + 5; let outer = source.find("outer(").unwrap() + 5;
            assert_eq!(hints.call(source, inner + 1, language), Some(inner), "{language:?}");
            assert_eq!(hints.call(source, source.find(", 2").unwrap() + 2, language), Some(outer), "{language:?}");
        }
        let mut hints = Hints::new(Language::Python, true);
        assert_eq!(hints.call("def f(value): pass", 8, Language::Python), None);
        assert_eq!(hints.call("text = 'fake(1)'", 13, Language::Python), None);
        assert_eq!(hints.call("f(1)", 4, Language::Python), None);
        assert_eq!(hints.call("f(", 2, Language::Python), Some(1));
        assert_eq!(hints.call("def f(", 6, Language::Python), None);
        let source = "int main() { combine(";
        assert_eq!(hints.call(source, source.len(), Language::Cpp), Some(source.len() - 1));
    }
}
