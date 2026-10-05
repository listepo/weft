//! The canonical Weft source a generator can leave in a leading comment of its output, so that the
//! importer gives back exactly that document. The comment is only a claim: the importer believes it
//! only when regenerating from it yields the same code (compared structurally, so a formatter run
//! over the output does not matter), and otherwise reads the code as hand-written.

/// The first word of the comment.
pub const MARKER: &str = "weft:source";

/// The comment text after the marker line: `\` escapes the next character, so the payload never
/// contains a sequence that would end or nest an HTML or JavaScript comment (`-->`, `--!>`, `<!--`,
/// `*/`).
pub fn escape(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len() + 8);
    let mut chars = markup.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '-' if chars.peek() == Some(&'-') => out.push_str("-\\"),
            '*' if chars.peek() == Some(&'/') => out.push_str("*\\"),
            '<' if chars.peek() == Some(&'!') => out.push_str("<\\"),
            _ => out.push(c),
        }
    }
    out
}

pub fn unescape(payload: &str) -> String {
    let mut out = String::with_capacity(payload.len());
    let mut chars = payload.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The comment a generator writes: `open`, the marker and its `options` on the first line, the
/// escaped markup, then `close` on a line of its own.
pub fn comment(open: &str, options: &str, markup: &str, close: &str) -> String {
    let head = if options.is_empty() {
        MARKER.to_owned()
    } else {
        format!("{MARKER} {options}")
    };
    format!("{open} {head}\n{}\n{close}", escape(markup.trim_end()))
}

/// The first source comment in `text`: its options line and its unescaped markup. Only the first
/// `open … close` comment that starts with the marker counts.
pub fn find<'t>(text: &'t str, open: &str, close: &str) -> Option<(&'t str, String)> {
    let mut rest = text;
    while let Some(at) = rest.find(open) {
        let body = &rest[at + open.len()..];
        let end = body.find(close)?;
        let inner = &body[..end];
        if let Some(after) = inner.trim_start().strip_prefix(MARKER) {
            let (options, payload) = after.split_once('\n').unwrap_or((after, ""));
            // The generator ends the payload with a line break before `close`.
            let payload = payload.strip_suffix('\n').unwrap_or(payload);
            return Some((options.trim(), unescape(payload)));
        }
        rest = &body[end + close.len()..];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_cannot_close_or_nest_comments() {
        let markup = r#"<text id="t">a --> b --!> c <!-- d */ e \ f -- g</text>"#;
        let escaped = escape(markup);
        for bad in ["-->", "--!>", "<!--", "*/", "--"] {
            assert!(!escaped.contains(bad), "{bad} in {escaped}");
        }
        assert_eq!(unescape(&escaped), markup);
    }

    #[test]
    fn the_first_marked_comment_is_found() {
        let markup = "<screen id=\"s\" weft=\"0.1\">\n  <text id=\"t\">x --> y</text>\n</screen>";
        let html = format!(
            "<!-- other -->\n{}\n<p>",
            comment("<!--", "", markup, "-->")
        );
        let (options, found) = find(&html, "<!--", "-->").unwrap();
        assert_eq!((options, found.as_str()), ("", markup));
        let js = comment("/*", "name=Login typescript", markup, "*/");
        let (options, found) = find(&js, "/*", "*/").unwrap();
        assert_eq!((options, found.as_str()), ("name=Login typescript", markup));
        assert!(find("<!-- weft:sourc -->", "<!--", "-->").is_none());
        assert!(find("/* weft:source never closed", "/*", "*/").is_none());
    }
}
