//! Dependency-free runner for the same engine tests compiled into gpui-base.
#[path = "../vim.rs"]
mod vim;
fn main() {
    let mut engine = vim::VimEngine::new(std::rc::Rc::new(std::cell::RefCell::new(
        vim::VimShared::default(),
    )));
    let _ = engine.mode().label();
    let _ = engine.pending();
    let _ = engine.block_insertion();
    let _ = engine.block_geometry("", 4);
    let _ = engine.blur("", 0, 4);
}
