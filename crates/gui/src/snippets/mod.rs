pub mod expansion;
mod completion;
mod menu;
mod editor;
pub use editor::SnippetEditor;

mod ai;
pub use editor::bind_keys as editor_bind_keys;

#[cfg(feature="gui-test")]
pub(crate) use editor::fixture;
