use gpui_kit::*;

pub(super) fn line(window: &mut Window, origin: Point<Pixels>, from: (f32, f32), to: (f32, f32), color: Hsla) {
    let mut path = PathBuilder::stroke(px(1.4));
    path.move_to(point(origin.x + px(from.0), origin.y + px(from.1)));
    path.line_to(point(origin.x + px(to.0), origin.y + px(to.1)));
    if let Ok(path) = path.build() { window.paint_path(path, color); }
}

pub(super) fn arrow_head(window: &mut Window, origin: Point<Pixels>, from: (f32, f32), to: (f32, f32), color: Hsla) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len);
    let base = (to.0 - ux * 7., to.1 - uy * 7.);
    let (px_, py_) = (-uy * 3.5, ux * 3.5);
    let mut path = PathBuilder::fill();
    path.move_to(point(origin.x + px(to.0), origin.y + px(to.1)));
    path.line_to(point(origin.x + px(base.0 + px_), origin.y + px(base.1 + py_)));
    path.line_to(point(origin.x + px(base.0 - px_), origin.y + px(base.1 - py_)));
    path.close();
    if let Ok(path) = path.build() { window.paint_path(path, color); }
}
