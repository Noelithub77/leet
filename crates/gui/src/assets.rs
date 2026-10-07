use gpui_kit::*;

gpui_kit::assets::icon_assets!(HomeIcons, [House, Map, X, Star, ExternalLink, Play, ArrowLeft, ChevronDown, Brackets, ArrowLeftRight, Layers, Search, PanelTop, Link, Network, GitBranch, ListOrdered, Undo2, CalendarRange, Zap, Share2, Workflow, ChartColumn, Grid2x2, Binary, Calculator, FileText, Lightbulb, Copy, Info, RefreshCw, Plus, Pencil, Tags, ArrowDownAZ, Download, PanelLeft, PanelBottom, PanelRight]);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let language: Option<&'static [u8]> = match path {
            "languages/python.svg" => Some(include_bytes!("../assets/languages/python.svg")),
            "languages/cpp.svg" => Some(include_bytes!("../assets/languages/cpp.svg")),
            "languages/go.svg" => Some(include_bytes!("../assets/languages/go.svg")),
            "languages/c.svg" => Some(include_bytes!("../assets/languages/c.svg")),
            _ => None,
        };
        if let Some(bytes) = language { return Ok(Some(std::borrow::Cow::Borrowed(bytes))); }
        let brand: Option<&'static [u8]> = match path {
            "leet.svg" => Some(include_bytes!("../assets/leet.svg")),
            "providers/neetcode.svg" => Some(include_bytes!("../assets/providers/neetcode.svg")),
            "providers/leetcode.svg" => Some(include_bytes!("../assets/providers/leetcode.svg")),
            "providers/codeforces.svg" => Some(include_bytes!("../assets/providers/codeforces.svg")),
            "providers/chatgpt.svg" => Some(include_bytes!("../assets/providers/chatgpt.svg")),
            "providers/claude.svg" => Some(include_bytes!("../assets/providers/claude.svg")),
            "providers/gemini.svg" => Some(include_bytes!("../assets/providers/gemini.svg")),
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
        paths.extend(["leet.svg", "providers/neetcode.svg", "providers/leetcode.svg", "providers/codeforces.svg", "providers/chatgpt.svg", "providers/claude.svg", "providers/gemini.svg", "languages/python.svg", "languages/cpp.svg", "languages/go.svg", "languages/c.svg"].into_iter().filter(|name| name.starts_with(path)).map(Into::into));
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::HomeIcons;
    use gpui_kit::AssetSource;

    #[::core::prelude::v1::test]
    fn update_icon_is_embedded_without_a_runtime_download() {
        for name in [gpui_kit::assets::IconName::Download, gpui_kit::assets::IconName::PanelLeft, gpui_kit::assets::IconName::PanelBottom, gpui_kit::assets::IconName::PanelRight] {
            let icon = HomeIcons.load(&name.path()).unwrap().expect("Status icons must be bundled");
            assert!(!icon.is_empty());
        }
    }
}
