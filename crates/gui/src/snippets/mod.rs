pub mod expansion;
mod completion;
mod symbols;
mod menu;
pub(crate) mod signature_help;
mod editor;
pub use editor::SnippetEditor;

mod ai;
pub use editor::bind_keys as editor_bind_keys;

#[cfg(feature="gui-test")]
pub(crate) use editor::fixture;
