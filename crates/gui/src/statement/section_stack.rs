//! Reserve every header and give short cards their natural height before sharing overflow.
use gpui_kit::*;

pub(super) struct SectionStack {
    cards: Vec<AnyElement>,
    expanded: bool,
}
impl SectionStack {
    pub(super) fn new(cards: Vec<AnyElement>, expanded: bool) -> Self { Self { cards, expanded } }
}
impl IntoElement for SectionStack {
    type Element = Self;
    fn into_element(self) -> Self { self }
}
impl Element for SectionStack {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> { None }
    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let header = rems(2.25).to_pixels(window.rem_size()) + px(2.);
        let gap = rems(0.5).to_pixels(window.rem_size());
        let height = if self.expanded { relative(1.).into() } else { (header * self.cards.len() as f32 + gap * self.cards.len().saturating_sub(1) as f32).into() };
        let style = Style { size: size(relative(1.).into(), height), min_size: size(px(0.).into(), px(0.).into()), flex_grow: if self.expanded { 1. } else { 0. }, flex_shrink: 1., ..Default::default() };
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        let gap = rems(0.5).to_pixels(window.rem_size());
        // The fixed header includes the card's two one-pixel borders.
        let header = rems(2.25).to_pixels(window.rem_size()) + px(2.);
        let natural: Vec<_> = self.cards.iter_mut().map(|card| {
            card.layout_as_root(size(AvailableSpace::Definite(bounds.size.width), AvailableSpace::MaxContent), window, cx).height
        }).collect();
        let available = (bounds.size.height - gap * self.cards.len().saturating_sub(1) as f32).max(px(0.));
        let heights = allocate_heights(&natural, header, available);
        let mut origin = bounds.origin;
        for (card, height) in self.cards.iter_mut().zip(heights) {
            card.layout_as_root(size(AvailableSpace::Definite(bounds.size.width), AvailableSpace::Definite(height)), window, cx);
            card.prepaint_at(origin, window, cx);
            origin.y += height + gap;
        }
    }
    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        for card in &mut self.cards { card.paint(window, cx); }
    }
}

fn allocate_heights(natural: &[Pixels], header: Pixels, available: Pixels) -> Vec<Pixels> {
    if natural.is_empty() { return Vec::new(); }
    let minimum = header.min(available / natural.len() as f32);
    let mut heights = vec![minimum; natural.len()];
    let mut remaining = (available - minimum * natural.len() as f32).max(px(0.));
    let mut pending: Vec<_> = (0..natural.len()).collect();
    while !pending.is_empty() {
        let share = remaining / pending.len() as f32;
        let short: Vec<_> = pending.iter().copied().filter(|index| natural[*index] - minimum <= share).collect();
        if short.is_empty() {
            for index in pending { heights[index] += share; }
            break;
        }
        for index in &short {
            let body = (natural[*index] - minimum).max(px(0.));
            heights[*index] += body;
            remaining = (remaining - body).max(px(0.));
        }
        pending.retain(|index| !short.contains(index));
    }
    heights
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn short_cards_fit_and_long_cards_share_the_remaining_space() {
        assert_eq!(allocate_heights(&[px(1000.), px(1000.), px(100.)], px(40.), px(600.)), vec![px(250.), px(250.), px(100.)]);
        assert_eq!(allocate_heights(&[px(80.), px(40.), px(40.)], px(40.), px(600.)), vec![px(80.), px(40.), px(40.)]);
        assert_eq!(allocate_heights(&[px(1000.), px(1000.), px(40.)], px(40.), px(300.)), vec![px(130.), px(130.), px(40.)]);
    }
}
