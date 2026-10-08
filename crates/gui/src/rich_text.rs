//! Shared selectable Markdown with native inline and display math.
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex};
use gpui_kit::*;
use gpui_kit::base::text::{markdown_ast, InlineElement, InlineRenderContext, MarkdownNode, MarkdownParseContext, MarkdownPlugin};
use gpui_kit::component::text::TextView;
use gpui_kit::component::ActiveTheme as _;

#[derive(Clone, PartialEq)]
struct Key { latex: String, size: u32, scale: u32, color: [u32; 4] }
#[derive(Clone)]
struct FormulaImage { image: Arc<RenderImage>, width: f32, height: f32 }
type Cache = VecDeque<(Key, Option<FormulaImage>)>;
static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(VecDeque::new()));

fn render_formula(latex: &str, size: f32, scale: f32, color: Hsla) -> Option<FormulaImage> {
    if latex.len() > 4096 { return None; }
    let key = Key { latex: latex.into(), size: size.to_bits(), scale: scale.to_bits(), color: [color.h.to_bits(), color.s.to_bits(), color.l.to_bits(), color.a.to_bits()] };
    {
        let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(index) = cache.iter().position(|(previous, _)| previous == &key) {
            let entry = cache.remove(index).unwrap(); let result = entry.1.clone(); cache.push_back(entry); return result;
        }
    }
    let rendered = ratex_gpui::render::render_latex(latex, size, scale, color)
        .filter(|image| image.width <= 4096. && image.height <= 2048.)
        .map(|image| FormulaImage { image: image.image, width: image.width, height: image.height });
    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    if cache.len() >= 256 { cache.pop_front(); }
    cache.push_back((key, rendered.clone()));
    rendered
}

fn formula_element(image: FormulaImage) -> AnyElement {
    let pad = ratex_gpui::render::PAD;
    div().w(px((image.width - pad * 2.).max(1.))).h(px((image.height - pad * 2.).max(1.))).overflow_hidden()
        .child(img(image.image).w(px(image.width)).h(px(image.height)).ml(px(-pad)).mt(px(-pad))).into_any_element()
}

struct MathPlugin { block: bool }
impl MarkdownPlugin for MathPlugin {
    fn name(&self) -> &str { if self.block { "display-math" } else { "inline-math" } }
    fn is_block(&self) -> bool { self.block }
    fn parse(&self, node: &markdown_ast::Node, context: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let latex = match node {
            markdown_ast::Node::Math(math) if self.block => &math.value,
            markdown_ast::Node::InlineMath(math) if !self.block => &math.value,
            markdown_ast::Node::Code(code) if self.block && matches!(code.lang.as_deref(), Some("math" | "latex" | "tex")) => &code.value,
            _ => return None,
        };
        Some(MarkdownNode::new(self.name().to_owned(), latex.clone()).text(latex.clone())
            .markdown(context.node_source(node).unwrap_or(latex).to_owned()).accessibility_label(latex.clone()))
    }
    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = cx.theme().foreground;
        let size: f32 = window.text_style().font_size.to_pixels(window.rem_size()).into();
        match render_formula(node.as_text(), size, window.scale_factor(), color) {
            Some(image) => div().id(SharedString::from(format!("math-{}", node.source_range().map_or(0, |range| range.start))))
                .max_w_full().overflow_x_scroll().py_2().child(formula_element(image)).into_any_element(),
            None => div().whitespace_normal().child(node.as_markdown().to_owned()).into_any_element(),
        }
    }
    fn render_inline(&self, node: &MarkdownNode, context: &InlineRenderContext, window: &mut Window, _cx: &mut App) -> Option<InlineElement> {
        let size: f32 = context.font_size().into();
        let image = render_formula(node.as_text(), size, window.scale_factor(), context.text_style().color)?;
        let height = (image.height - ratex_gpui::render::PAD * 2.).max(1.);
        Some(InlineElement::new(formula_element(image)).with_baseline(px((height - size * 0.18).max(0.))))
    }
}

struct HtmlMathPlugin;
impl MarkdownPlugin for HtmlMathPlugin {
    fn name(&self) -> &str { "html-math" }
    fn is_block(&self) -> bool { true }
    fn parse(&self, node: &markdown_ast::Node, _context: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let markdown_ast::Node::Html(html) = node else { return None; };
        if !html.value.contains('$') && !html.value.contains(r"\(") && !html.value.contains(r"\[") { return None; }
        let content = practice::prompts::statement_markdown(&html.value);
        if content == html.value { return None; }
        Some(MarkdownNode::new(self.name(), content.clone()).text(content).markdown(html.value.clone()))
    }
    fn render(&self, node: &MarkdownNode, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        markdown(SharedString::from(format!("html-math-{}", node.source_range().map_or(0, |range| range.start))), node.as_text())
    }
}

pub fn markdown(id: impl Into<ElementId>, source: impl AsRef<str>) -> TextView {
    TextView::markdown(id, practice::rich_text::normalize_math(source.as_ref())).selectable(true)
        .plugin(MathPlugin { block: false }).plugin(MathPlugin { block: true }).plugin(HtmlMathPlugin)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn native_math_renders_problem_and_chat_constructs_and_caches_images() {
        for latex in [r"c_i \mathrel{+}= \operatorname{sgn}(a_i-b_i)", r"\frac{-b\pm\sqrt{b^2-4ac}}{2a}",
            r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}", r"\begin{pmatrix}1&2\\3&4\end{pmatrix}",
            r"\begin{aligned}a&=b+c\\d&=e\end{aligned}", r"\left\{x\in\mathbb{R}:x\ge0\right\}"] {
            let first = render_formula(latex, 18., 1., rgb(0xffffff).into()).unwrap_or_else(|| panic!("Cannot render {latex}"));
            let second = render_formula(latex, 18., 1., rgb(0xffffff).into()).unwrap();
            assert!(first.width > 16. && first.height > 16.);
            assert!(Arc::ptr_eq(&first.image, &second.image));
        }
        assert!(render_formula(&"x".repeat(4097), 18., 1., rgb(0xffffff).into()).is_none());
        assert!(render_formula(r"\frac{", 18., 1., rgb(0xffffff).into()).is_none());
        let normal = render_formula("x_i", 18., 1., rgb(0xffffff).into()).unwrap();
        let scaled = render_formula("x_i", 36., 2., rgb(0xffffff).into()).unwrap();
        assert!(scaled.width > normal.width);
        assert!(!Arc::ptr_eq(&normal.image, &scaled.image));
    }
}
