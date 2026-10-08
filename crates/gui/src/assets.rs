use gpui_kit::*;

gpui_kit::assets::icon_assets!(HomeIcons, [House, Map, X, Star, ExternalLink, Play, ArrowLeft, ChevronDown, Brackets, ArrowLeftRight, Layers, Search, PanelTop, Link, Network, GitBranch, ListOrdered, Undo2, CalendarRange, Zap, Share2, Workflow, ChartColumn, Grid2x2, Binary, Calculator, FileText, Lightbulb, Copy, Info, RefreshCw, Plus, Pencil, Tags, ArrowDownAZ, Download, PanelLeft, PanelBottom, PanelRight, LifeBuoy, Bug, FlaskConical, Gauge, Rocket, Sparkles, Footprints, Puzzle, BookOpen, WandSparkles, BugPlay, Timer, CircleStop, SkipBack, SkipForward, Pause, ChevronLeft, ChevronRight, Code, Bot, CircleCheck, CircleX, Brain, Globe, Terminal, ArrowUpRight, Send, Undo, Check, CornerDownRight, CornerUpLeft, TriangleAlert, ArrowRight, CircleDot, SquareFunction, MessageCircle, ArrowUp, Settings2]);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let language: Option<&'static [u8]> = match path {
            "languages/python.svg" => Some(include_bytes!("../assets/languages/python.svg")),
            "languages/cpp.svg" => Some(include_bytes!("../assets/languages/cpp.svg")),
            "languages/go.svg" => Some(include_bytes!("../assets/languages/go.svg")),
            "languages/java.svg" => Some(include_bytes!("../assets/languages/java.svg")),
            "languages/c.svg" => Some(include_bytes!("../assets/languages/c.svg")),
            _ => None,
        };
        if let Some(bytes) = language { return Ok(Some(std::borrow::Cow::Borrowed(bytes))); }
        let brand: Option<&'static [u8]> = match path {
            "leet.svg" => Some(include_bytes!("../assets/leet.svg")),
            "providers/neetcode.svg" => Some(include_bytes!("../assets/providers/neetcode.svg")),
            "providers/leetcode.svg" => Some(include_bytes!("../assets/providers/leetcode.svg")),
            "providers/codeforces.svg" => Some(include_bytes!("../assets/providers/codeforces.svg")),
            "providers/codechef.svg" => Some(include_bytes!("../assets/providers/codechef.svg")),
            "providers/chatgpt.svg" => Some(include_bytes!("../assets/providers/chatgpt.svg")),
            "providers/claude.svg" => Some(include_bytes!("../assets/providers/claude.svg")),
            "providers/gemini.svg" => Some(include_bytes!("../assets/providers/gemini.svg")),
            "providers/codex.svg" => Some(include_bytes!("../assets/providers/codex.svg")),
            "providers/claude-code.svg" => Some(include_bytes!("../assets/providers/claude-code.svg")),
            "providers/opencode.svg" => Some(include_bytes!("../assets/providers/opencode.svg")),
            "providers/antigravity.svg" => Some(include_bytes!("../assets/providers/antigravity.svg")),
            "providers/gemini-cli.svg" => Some(include_bytes!("../assets/providers/gemini-cli.svg")),
            "providers/cursor.svg" => Some(include_bytes!("../assets/providers/cursor.svg")),
            _ => None,
        };
        if let Some(bytes) = brand { return Ok(Some(std::borrow::Cow::Borrowed(bytes))); }
        match HomeIcons.load(path)? {
            Some(asset) => Ok(Some(asset)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(HomeIcons.list(path)?);
        paths.extend(["leet.svg", "providers/neetcode.svg", "providers/leetcode.svg", "providers/codeforces.svg", "providers/codechef.svg", "providers/chatgpt.svg", "providers/claude.svg", "providers/gemini.svg", "providers/codex.svg", "providers/claude-code.svg", "providers/opencode.svg", "providers/antigravity.svg", "providers/gemini-cli.svg", "providers/cursor.svg", "languages/python.svg", "languages/cpp.svg", "languages/go.svg", "languages/c.svg", "languages/java.svg"].into_iter().filter(|name| name.starts_with(path)).map(Into::into));
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::{Assets, HomeIcons};
    use gpui_kit::AssetSource;

    #[::core::prelude::v1::test]
    fn every_selectable_language_icon_is_bundled() {
        for language in practice::language::Language::ALL {
            let icon = Assets.load(&format!("languages/{}.svg", language.id())).unwrap().expect("Language icon must be bundled");
            assert!(!icon.is_empty());
        }
    }

    #[::core::prelude::v1::test]
    fn every_practice_provider_icon_is_bundled() {
        for provider in practice::language::Source::ALL {
            let name = provider.label().to_lowercase();
            assert!(!Assets.load(&format!("providers/{name}.svg")).unwrap().expect("Provider logo must be embedded").is_empty());
        }
    }

    #[::core::prelude::v1::test]
    fn update_icon_is_embedded_without_a_runtime_download() {
        for name in [gpui_kit::assets::IconName::Download, gpui_kit::assets::IconName::PanelLeft, gpui_kit::assets::IconName::PanelBottom, gpui_kit::assets::IconName::PanelRight] {
            let icon = HomeIcons.load(&name.path()).unwrap().expect("Status icons must be bundled");
            assert!(!icon.is_empty());
        }
    }
    #[::core::prelude::v1::test]
    fn chat_icons_are_embedded() {
        for name in [gpui_kit::assets::IconName::MessageCircle, gpui_kit::assets::IconName::ArrowUp, gpui_kit::assets::IconName::Settings2] {
            assert!(!HomeIcons.load(&name.path()).unwrap().expect("Chat icons must be bundled").is_empty());
        }
    }

    #[::core::prelude::v1::test]
    fn debugger_state_icons_are_embedded() {
        for name in [gpui_kit::assets::IconName::CornerDownRight, gpui_kit::assets::IconName::CornerUpLeft, gpui_kit::assets::IconName::TriangleAlert, gpui_kit::assets::IconName::ArrowRight, gpui_kit::assets::IconName::CircleDot, gpui_kit::assets::IconName::SquareFunction] {
            assert!(!HomeIcons.load(&name.path()).unwrap().expect("Debugger icons must be bundled").is_empty());
        }
    }

}
