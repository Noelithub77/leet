//! Bundled themes (Vesper default) and live switching.

use gpui_kit::component::{Selectable as _, Theme, ThemeRegistry};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::*;

macro_rules! themes {
    ($($file:literal),* $(,)?) => { &[$(include_str!(concat!("../themes/", $file))),*] };
}

const THEMES: &[&str] = themes!(
    "vesper.json", "adventure.json", "alduin.json", "asciinema.json", "aurora.json", "ayu.json",
    "catppuccin.json", "everforest.json", "fahrenheit.json", "flexoki.json", "gruvbox.json",
    "harper.json", "hybrid.json", "jellybeans.json", "kibble.json", "macos-classic.json",
    "mellifluous.json", "molokai.json", "solarized.json", "spaceduck.json", "tokyonight.json",
    "twilight.json",
);

pub const DEFAULT: &str = "Vesper";

pub const ZOOM_MIN: f32 = 0.7;
pub const ZOOM_MAX: f32 = 1.8;

/// Font sizes before zoom, so switching themes keeps the zoom level.
struct Zoom {
    level: f32,
    ui: Pixels,
    mono: Pixels,
}

impl Global for Zoom {}

struct UiFont(SharedString);
impl Global for UiFont {}

pub fn init(name: &str, zoom: f32, font: &str, cx: &mut App) {
    cx.set_global(UiFont(font.to_owned().into()));
    let registry = ThemeRegistry::global_mut(cx);
    for source in THEMES {
        if let Err(err) = registry.load_themes_from_str(source) {
            eprintln!("leet: bundled theme failed to load: {err}");
        }
    }
    let theme = Theme::global(cx);
    cx.set_global(Zoom { level: 1.0, ui: theme.font_size, mono: theme.mono_font_size });
    if !apply(name, cx) {
        apply(DEFAULT, cx);
    }
    set_zoom(zoom, cx);
}

/// Scales every font (and with them all rem-based sizes). Returns the clamped level.
pub fn set_zoom(level: f32, cx: &mut App) -> f32 {
    let level = (level * 10.).round() / 10.;
    let level = level.clamp(ZOOM_MIN, ZOOM_MAX);
    let zoom = cx.global_mut::<Zoom>();
    zoom.level = level;
    let (ui, mono) = (zoom.ui * level, zoom.mono * level);
    Theme::update(cx, |theme| {
        theme.font_size = ui;
        theme.mono_font_size = mono;
    });
    cx.refresh_windows();
    level
}

/// Applies a registered theme by name; false when it doesn't exist.
pub fn apply(name: &str, cx: &mut App) -> bool {
    let Some(config) = ThemeRegistry::global(cx).themes().get(name).cloned() else {
        return false;
    };
    Theme::update(cx, |theme| {
        theme.apply_config(&config);
        theme.mode = config.mode;
        let cyan = rgb(0x8be9fd);
        theme.tab_active = cyan.opacity(0.14).into();
        theme.tab_active_foreground = cyan.into();
        theme.tokens.tab_active = theme.tab_active.into();
        theme.radius = px(10.);
        theme.radius_lg = px(16.);
    });
    if let Some(font) = cx.try_global::<UiFont>().map(|font| font.0.clone()) {
        set_font(font, cx);
    }
    if let Some(level) = cx.try_global::<Zoom>().map(|z| z.level) {
        set_zoom(level, cx);
    }
    cx.refresh_windows();
    true
}

pub fn set_font(name: impl Into<SharedString>, cx: &mut App) {
    let name = name.into();
    cx.set_global(UiFont(name.clone()));
    Theme::update(cx, |theme| theme.font_family = name);
    cx.refresh_windows();
}

pub fn fonts(cx: &App) -> Vec<String> {
    cx.text_system().all_font_names()
}

/// Theme names, dark first, each group sorted.
pub fn names(cx: &App) -> Vec<SharedString> {
    let mut themes: Vec<_> = ThemeRegistry::global(cx)
        .themes()
        .values()
        .map(|t| (!t.mode.is_dark(), t.name.clone()))
        .collect();
    themes.sort();
    themes.dedup();
    themes.into_iter().map(|(_, name)| name).collect()
}

/// Give choice cards a persistent accent instead of the toolkit's gray pressed state.
pub fn selected_choice(button: Button, selected: bool, cx: &App) -> Button {
    let button = button.selected(selected);
    if !selected { return button; }
    let accent: Hsla = rgb(0x99ffe4).into();
    button.custom(ButtonCustomVariant::new(cx)
        .foreground(accent)
        .color(accent.opacity(0.16))
        .hover(accent.opacity(0.22))
        .active(accent.opacity(0.16)))
        .border_1().border_color(accent)
}
