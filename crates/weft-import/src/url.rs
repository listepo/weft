//! SPEC §9 Trust: URLs in `link.href` and `image.src` are used only when they are `http`,
//! `https`, `mailto` or relative; anything else is dropped. Tabs, newlines and surrounding
//! control characters are removed first, as a URL parser would. Generators share this so an
//! exporter cannot skip the allowlist.

/// A URL a renderer may use: `http`, `https`, `mailto`, or a relative reference with no scheme.
pub fn safe_url(value: &str) -> Option<String> {
    let url: String = value
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let url = url.trim_matches(|c: char| c <= ' ');
    if url.is_empty() {
        return None;
    }
    if !has_scheme(url) {
        return Some(url.to_owned());
    }
    let colon = url.find(':').unwrap_or_default();
    let scheme = &url[..colon];
    let valid = scheme
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
    let scheme = scheme.to_ascii_lowercase();
    (valid && matches!(scheme.as_str(), "http" | "https" | "mailto")).then(|| url.to_owned())
}

/// An `http`, `https` or `mailto` URL: A2UI `openUrl` needs an absolute URI of those schemes.
pub fn safe_absolute_url(value: &str) -> Option<String> {
    let url = safe_url(value)?;
    has_scheme(&url).then_some(url)
}

fn has_scheme(url: &str) -> bool {
    let colon = url.find(':');
    let delimiter = url.find(['/', '?', '#']);
    matches!((colon, delimiter), (Some(c), Some(d)) if c < d)
        || matches!((colon, delimiter), (Some(_), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_safe_urls_survive() {
        assert_eq!(safe_url("https://a.b/c").as_deref(), Some("https://a.b/c"));
        assert_eq!(safe_url("/x?y:z").as_deref(), Some("/x?y:z"));
        assert_eq!(safe_url("MAILTO:a@b").as_deref(), Some("MAILTO:a@b"));
        assert_eq!(safe_url("java\nscript:alert(1)"), None);
        assert_eq!(safe_url(" data:text/html,x"), None);
        assert_eq!(safe_url(""), None);
        assert_eq!(safe_url("javascript:alert(1)"), None);
        assert_eq!(safe_url("file:///etc/passwd"), None);
    }

    #[test]
    fn open_url_needs_an_allowed_absolute_uri() {
        assert_eq!(
            safe_absolute_url("https://a.b/c").as_deref(),
            Some("https://a.b/c")
        );
        assert_eq!(safe_absolute_url("/relative"), None);
        assert_eq!(safe_absolute_url("javascript:alert(1)"), None);
        assert_eq!(
            safe_absolute_url("mailto:a@b").as_deref(),
            Some("mailto:a@b")
        );
    }
}
