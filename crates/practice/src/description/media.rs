//! Public statement images: platform URL resolution and bounded, persistent downloads.
use std::{path::Path, time::Duration};
use anyhow::{ensure, Result};
use regex::{Captures, Regex};
use std::sync::LazyLock;
use url::Url;

static EMBED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<(?:iframe|video|audio|object)\b[^>]*>.*?</(?:iframe|video|audio|object)>|<embed\b[^>]*>").unwrap());
static MARKDOWN_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(!?\[[^\]]*\]\()([^\s)]+)(\))").unwrap());
static SVG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<svg\b[^>]*>.*?</svg>").unwrap());
static RESOURCE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<(?:img|a)\b[^>]*>").unwrap());

pub fn base_url(slug: &str) -> String {
    if slug.starts_with("cf:") { crate::codeforces::problem_url(slug).unwrap_or_else(|_| "https://codeforces.com/".into()) }
    else if slug.starts_with("cc:") { crate::codechef::problem_url(slug).unwrap_or_else(|_| "https://www.codechef.com/".into()) }
    else { format!("https://leetcode.com/problems/{slug}/") }
}

/// Keep the HTML intact while making lazy and relative image sources usable outside a browser.
pub fn normalize(html: &str, base: &str) -> String {
    let html = EMBED.replace_all(html, |capture: &Captures<'_>| {
        let fragment = scraper::Html::parse_fragment(&capture[0]);
        let selector = scraper::Selector::parse("iframe,video,audio,object,embed").expect("static embed selector");
        let Some(element) = fragment.select(&selector).next() else { return capture[0].to_owned(); };
        let attrs = element.value();
        let sources = scraper::Selector::parse("source[src]").expect("static media source selector");
        let source = attrs.attr("src").or_else(|| attrs.attr("data")).or_else(|| element.select(&sources).next().and_then(|source| source.value().attr("src")));
        let Some(source) = source.and_then(|source| Url::parse(base).ok()?.join(source).ok()).filter(|url| matches!(url.scheme(), "https" | "http")) else { return capture[0].to_owned(); };
        let label = attrs.attr("title").unwrap_or(match attrs.name() { "video" => "Video", "audio" => "Audio", _ => "Embedded content" });
        if attrs.attr("type").is_some_and(|kind| kind.starts_with("image/")) {
            return format!("<img src=\"{}\" alt=\"{}\">", html_escape::encode_double_quoted_attribute(source.as_str()), html_escape::encode_double_quoted_attribute(label));
        }
        let poster = attrs.attr("poster").map(|poster| format!("<img src=\"{}\" alt=\"{}\">", html_escape::encode_double_quoted_attribute(poster), html_escape::encode_double_quoted_attribute(label))).unwrap_or_default();
        format!("{poster}<p><a href=\"{}\">{}</a></p>", html_escape::encode_double_quoted_attribute(source.as_str()), html_escape::encode_text(label))
    });
    let html = SVG.replace_all(&html, |capture: &Captures<'_>| {
        let encoded: String = url::form_urlencoded::byte_serialize(capture[0].as_bytes()).collect();
        format!("<img src=\"data:image/svg+xml,{}\" alt=\"Diagram\">", encoded.replace('+', "%20"))
    });
    RESOURCE_TAG.replace_all(&html, |capture: &Captures<'_>| {
        let fragment = scraper::Html::parse_fragment(&capture[0]);
        let selector = scraper::Selector::parse("img,a").expect("static resource selector");
        let Some(image) = fragment.select(&selector).next() else { return capture[0].to_owned(); };
        let attrs = image.value();
        let from_set = |set: &str| set.split(',').next_back().and_then(|item| item.split_whitespace().next()).map(str::to_owned);
        let key = if attrs.name() == "a" { "href" } else { "src" };
        let source = if key == "href" { attrs.attr(key).map(str::to_owned) } else { ["data-src", "data-original", "data-lazy-src"].into_iter()
            .filter_map(|key| attrs.attr(key)).find(|value| !value.trim().is_empty()).map(str::to_owned)
            .or_else(|| attrs.attr("data-srcset").and_then(from_set))
            .or_else(|| attrs.attr("src").filter(|value| !value.trim().is_empty()).map(str::to_owned))
            .or_else(|| attrs.attr("srcset").and_then(from_set)) };
        let Some(source) = source else { return capture[0].to_owned(); };
        let source = Url::parse(base).ok().and_then(|base| base.join(source.trim()).ok())
            .filter(|url| matches!(url.scheme(), "https" | "http" | "data") || (key == "href" && url.scheme() == "mailto"))
            .map(|url| url.to_string()).unwrap_or_else(|| source.to_owned());
        let mut tag = format!("<{}", attrs.name());
        for (attribute, value) in attrs.attrs().filter(|(attribute, _)| *attribute != key) {
            tag.push_str(&format!(" {attribute}=\"{}\"", html_escape::encode_double_quoted_attribute(value)));
        }
        tag.push_str(&format!(" {key}=\"{}\">", html_escape::encode_double_quoted_attribute(&source)));
        tag
    }).into_owned()
}

pub fn normalize_markdown(markdown: &str, base: &str) -> String {
    MARKDOWN_LINK.replace_all(markdown, |capture: &Captures<'_>| {
        Url::parse(base).ok().and_then(|base| base.join(&capture[2]).ok())
            .map_or_else(|| capture[0].to_owned(), |url| format!("{}{}{}", &capture[1], url, &capture[3]))
    }).into_owned()
}

const LIMIT: u64 = 16 << 20;

pub fn cached_image(directory: &Path, uri: &str) -> Result<Vec<u8>> {
    let url = Url::parse(uri)?;
    ensure!(matches!(url.scheme(), "https" | "http") && url.username().is_empty() && url.password().is_none(), "Unsupported public image URL");
    let key = git2::Oid::hash_object(git2::ObjectType::Blob, uri.as_bytes())?;
    let path = directory.join(key.to_string());
    if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > 0 && metadata.len() <= LIMIT) { return Ok(std::fs::read(&path)?); }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(12))).max_redirects(5).build().into();
    let mut response = agent.get(uri).header("User-Agent", "leet public statement images").call()?;
    let content_type = response.headers().get("content-type").and_then(|value| value.to_str().ok()).unwrap_or("");
    ensure!(!content_type.starts_with("text/html"), "Statement image returned an HTML page");
    let bytes = response.body_mut().with_config().limit(LIMIT).read_to_vec()?;
    ensure!(!bytes.is_empty(), "Statement image is empty");
    std::fs::create_dir_all(directory)?;
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let temporary = directory.join(format!("{key}-{}-{unique}.tmp", std::process::id()));
    std::fs::write(&temporary, &bytes)?;
    if let Err(error) = std::fs::rename(&temporary, &path) { let _ = std::fs::remove_file(&temporary); return Err(error.into()); }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the public LeetCode image CDN"]
    fn live_rainwater_diagram() {
        let directory = tempfile::tempdir().unwrap();
        let image = cached_image(directory.path(), "https://assets.leetcode.com/uploads/2018/10/22/rainwatertrap.png").unwrap();
        assert!(image.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(cached_image(directory.path(), "https://assets.leetcode.com/uploads/2018/10/22/rainwatertrap.png").unwrap(), image);
    }
    #[test]
    fn relative_lazy_and_embedded_images_keep_attributes() {
        let html = normalize(r#"<p>Diagram</p><img src="blank.gif" data-src="/images/tree.svg" alt="a &amp; b" width="480"><img src="//cdn.example/a.png"><img src="data:image/png;base64,abc">"#, "https://codeforces.com/contest/1/problem/A");
        assert!(html.contains("https://codeforces.com/images/tree.svg"));
        assert!(html.contains("https://cdn.example/a.png"));
        assert!(html.contains("data:image/png;base64,abc"));
        assert!(html.contains("width=\"480\"") && html.contains("a &amp; b"));
        assert!(normalize("<img src='blank.gif' data-srcset='small.png 1x, large.png 2x'>", "https://www.codechef.com/").contains("src=\"https://www.codechef.com/large.png\""));
        assert!(normalize("<svg width='20'><rect width='20'/></svg>", "https://codeforces.com/").contains("data:image/svg+xml,"));
        assert!(normalize("<img srcset='small.png 1x, large.png 2x'>", "https://www.codechef.com/").contains("https://www.codechef.com/large.png"));
    }
    #[test]
    fn embedded_media_remains_available_as_links_and_posters() {
        let html = normalize("<video poster='/preview.png'><source src='/animation.mp4'></video><iframe src='/interactive' title='Interactive diagram'></iframe><audio src='/explanation.ogg'></audio>", "https://www.codechef.com/problems/EXAMPLE");
        for token in ["https://www.codechef.com/preview.png", "https://www.codechef.com/animation.mp4", "https://www.codechef.com/interactive", "Interactive diagram", "https://www.codechef.com/explanation.ogg"] { assert!(html.contains(token), "Dropped {token}: {html}"); }
        assert!(!html.contains("<video") && !html.contains("<iframe"));
    }
    #[test]
    fn download_cache_reopens_offline_and_rejects_failures() {
        use std::io::{Read, Write};
        let directory = tempfile::tempdir().unwrap();
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let uri = format!("http://{}/diagram.svg", server.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            let mut request = [0; 4096]; socket.read(&mut request).unwrap();
            let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='120' height='60'><rect width='120' height='60' fill='cyan'/></svg>";
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{svg}", svg.len()).unwrap();
        });
        let bytes = cached_image(directory.path(), &uri).unwrap(); thread.join().unwrap();
        assert_eq!(cached_image(directory.path(), &uri).unwrap(), bytes);
        assert!(cached_image(directory.path(), "file:///tmp/private").is_err());
        assert!(cached_image(directory.path(), "https://user:secret@example.com/a.png").is_err());
    }
}
