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
struct FormulaImage { image: Arc<RenderImage>, width: f32, height: f32, crop: Bounds<Pixels>, baseline: f32 }
#[derive(Default)]
struct Cache { entries: VecDeque<(Key, Option<FormulaImage>)>, bytes: usize }
impl Cache {
    fn insert(&mut self, key: Key, image: Option<FormulaImage>, byte_limit: usize) {
        let bytes = image.as_ref().map_or(0, FormulaImage::bytes);
        if bytes > byte_limit { return; }
        while self.entries.len() >= 256 || self.bytes + bytes > byte_limit {
            let Some((_, previous)) = self.entries.pop_front() else { break; };
            self.bytes -= previous.as_ref().map_or(0, FormulaImage::bytes);
        }
        self.bytes += bytes;
        self.entries.push_back((key, image));
    }
}
impl FormulaImage {
    fn bytes(&self) -> usize { self.image.as_bytes(0).map_or(0, <[u8]>::len) }
}
static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(Cache::default()));
const CACHE_BYTE_LIMIT: usize = 32 * 1024 * 1024;

// The math face has a smaller visible x-height than the surrounding UI fonts.
const INLINE_MATH_SCALE: f32 = 1.15;

fn render_formula(latex: &str, size: f32, scale: f32, color: Hsla) -> Option<FormulaImage> {
    if latex.len() > 4096 || !size.is_finite() || size <= 0. || !scale.is_finite() || !(0.01..=16.).contains(&scale) { return None; }
    let key = Key { latex: latex.into(), size: size.to_bits(), scale: scale.to_bits(), color: [color.h.to_bits(), color.s.to_bits(), color.l.to_bits(), color.a.to_bits()] };
    {
        let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(index) = cache.entries.iter().position(|(previous, _)| previous == &key) {
            let entry = cache.entries.remove(index).unwrap(); let result = entry.1.clone(); cache.entries.push_back(entry); return result;
        }
    }
    let rendered = (|| {
        // Match ratex-gpui's layout options. Calculate metrics only on cache misses;
        // its image-only API does not expose the ascent used by its rasterizer.
        let nodes = ratex_parser::parse(latex).ok()?;
        let layout = ratex_layout::layout(&nodes, &ratex_layout::LayoutOptions::default());
        let pad = ratex_gpui::render::PAD;
        let width = (layout.width as f32 * size + pad * 2.) * scale;
        let height = (layout.total_height() as f32 * size + pad * 2.) * scale;
        if !width.is_finite() || !height.is_finite() || width / scale > 4096. || height / scale > 2048.
            || width.ceil() * height.ceil() * 4. > CACHE_BYTE_LIMIT as f32 { return None; }
        let rendered = ratex_gpui::render::render_latex(latex, size, scale, color)?;
        let crop = formula_crop(&rendered, scale);
        let baseline = pad + layout.height as f32 * size - f32::from(crop.top());
        Some(FormulaImage { image: rendered.image, width: rendered.width, height: rendered.height, crop, baseline })
    })();
    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    cache.insert(key, rendered.clone(), CACHE_BYTE_LIMIT);
    rendered
}

fn formula_crop(rendered: &ratex_gpui::render::Rendered, scale: f32) -> Bounds<Pixels> {
    let pad = ratex_gpui::render::PAD;
    let guard = 1. / scale;
    // Preserve TeX spacing, while including italic overhang and antialiased ink
    // beyond the nominal layout box. Keep one physical pixel around the result.
    let (mut left, mut top) = (pad, pad);
    let (mut right, mut bottom) = ((rendered.width - pad).max(pad), (rendered.height - pad).max(pad));
    let width = (rendered.width * scale).round().max(1.) as usize;
    if let Some(bytes) = rendered.image.as_bytes(0) {
        for (index, _) in bytes.chunks_exact(4).enumerate().filter(|(_, pixel)| pixel[3] > 0) {
            let (x, y) = ((index % width) as f32 / scale, (index / width) as f32 / scale);
            left = left.min(x); top = top.min(y);
            right = right.max(x + guard); bottom = bottom.max(y + guard);
        }
    }
    left = (left - guard).max(0.); top = (top - guard).max(0.);
    right = (right + guard).min(rendered.width); bottom = (bottom + guard).min(rendered.height);
    Bounds { origin: point(px(left), px(top)), size: gpui_kit::size(px((right - left).max(guard)), px((bottom - top).max(guard))) }
}

fn formula_element(image: FormulaImage) -> AnyElement {
    div().w(image.crop.size.width).h(image.crop.size.height).overflow_hidden()
        .child(img(image.image).w(px(image.width)).h(px(image.height)).ml(-image.crop.left()).mt(-image.crop.top())).into_any_element()
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
        let size = f32::from(context.font_size()) * INLINE_MATH_SCALE;
        let latex = format!(r"\textstyle {}", node.as_text());
        let image = render_formula(&latex, size, window.scale_factor(), context.text_style().color)?;
        let baseline = px(image.baseline);
        Some(InlineElement::new(formula_element(image)).with_baseline(baseline))
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
    fn inline_baseline_preserves_base_alignment_and_all_raster_ink() {
        for scale in [1., 2.] {
            let bare = render_formula(r"\textstyle x", 16.1, scale, rgb(0xffffff).into()).unwrap();
            let subscript = render_formula(r"\textstyle x_{j-1}", 16.1, scale, rgb(0xffffff).into()).unwrap();
            let raster_baseline = |image: &FormulaImage| image.baseline + f32::from(image.crop.top());
            assert!((raster_baseline(&bare) - raster_baseline(&subscript)).abs() < 0.001, "Subscripts moved the base glyph's baseline");
            assert!(subscript.height > bare.height, "The subscript should extend below the base");
            for latex in [r"\textstyle f", r"\textstyle j", r"\textstyle x_{j-1}", r"\textstyle 10^9", r"\textstyle \sqrt{x_i}", r"\textstyle \frac{x_i}{y_j}"] {
                let image = render_formula(latex, 16.1, scale, rgb(0xffffff).into()).unwrap();
                assert!(image.baseline >= 0. && image.baseline <= f32::from(image.crop.size.height), "Invalid baseline for {latex}");
                let width = (image.width * scale).round() as usize;
                for (index, _) in image.image.as_bytes(0).unwrap().chunks_exact(4).enumerate().filter(|(_, pixel)| pixel[3] > 0) {
                    let (x, y) = ((index % width) as f32 / scale, (index / width) as f32 / scale);
                    assert!(x >= f32::from(image.crop.left()) && x < f32::from(image.crop.right())
                        && y >= f32::from(image.crop.top()) && y < f32::from(image.crop.bottom()), "Clipped ink in {latex} at {x},{y}");
                }
            }
        }
        assert!(render_formula("x", 1_000_000., 2., rgb(0xffffff).into()).is_none());
    }
    #[::core::prelude::v1::test]
    fn small_inline_math_preserves_color_scale_and_cache_budget() {
        for size in [12., 14., 19.6] {
            for scale in [1., 2.] {
                for color in [rgb(0xffffff).into(), rgb(0x171717).into()] {
                    for latex in [r"\textstyle i", r"\textstyle a_{j-1}", r"\textstyle 10^9", r"\textstyle \sqrt{x_i}", r"\textstyle \frac{x_i}{y_j}"] {
                        let image = render_formula(latex, size * INLINE_MATH_SCALE, scale, color).unwrap();
                        assert!(image.image.as_bytes(0).unwrap().chunks_exact(4).any(|pixel| pixel[3] > 128), "No readable ink for {latex}");
                        let rgba: Rgba = color.into();
                        let expected = [rgba.b, rgba.g, rgba.r].map(|channel| (channel * 255.).round() as u8);
                        assert!(image.image.as_bytes(0).unwrap().chunks_exact(4).filter(|pixel| pixel[3] > 0).all(|pixel| pixel[..3] == expected), "Math foreground changed in {latex}");
                        let repeated = render_formula(latex, size * INLINE_MATH_SCALE, scale, color).unwrap();
                        assert!(Arc::ptr_eq(&image.image, &repeated.image));
                    }
                }
            }
        }
        let image = render_formula("x_i", 16.1, 2., rgb(0xffffff).into()).unwrap();
        let mut cache = Cache::default();
        for size in 0..4 {
            cache.insert(Key { latex: "x_i".into(), size, scale: 0, color: [0; 4] }, Some(image.clone()), image.bytes() * 2);
        }
        assert_eq!(cache.entries.len(), 2);
        assert_eq!(cache.bytes, image.bytes() * 2);
        assert_eq!(cache.entries.front().unwrap().0.size, 2);
        cache.insert(Key { latex: "oversized".into(), size: 0, scale: 0, color: [0; 4] }, Some(image.clone()), image.bytes() - 1);
        assert_eq!(cache.entries.len(), 2);
    }
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
