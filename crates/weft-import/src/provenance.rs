//! The canonical Weft source a generator can leave in a comment of its output, so that the importer
//! gives back exactly that document. The comment is only a claim: the importer believes it only
//! when the code is what that document generates, and otherwise reads the code as hand-written.
//! The web and SwiftUI targets share this module, so every comment is escaped alike.

use weft_core::{
    Catalog, Diagnostic, Document, Mode, ParseOptions, canonicalize, has_errors, parse,
};

/// The first word of the comment.
pub const MARKER: &str = "weft:source";

/// Text a readable note keeps: the format's limit (SPEC §2.3), for input that was not validated.
const MAX_NOTE: usize = 500;

/// The comment text after the marker line: `\` escapes the next character, so the payload never
/// contains a sequence that would end or nest an HTML or JavaScript comment (`-->`, `--!>`, `<!--`,
/// `*/`). Every `@` is followed by a `\` too: context is untrusted text, and `@license` or
/// `@preserve` would make a minifier keep the comment, with the screen's notes, in a production
/// bundle.
pub fn escape(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len() + 8);
    let mut chars = markup.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '-' if chars.peek() == Some(&'-') => out.push_str("-\\"),
            '*' if chars.peek() == Some(&'/') => out.push_str("*\\"),
            '<' if chars.peek() == Some(&'!') => out.push_str("<\\"),
            '@' => out.push_str("@\\"),
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

fn head(options: &str) -> String {
    if options.is_empty() {
        MARKER.to_owned()
    } else {
        format!("{MARKER} {options}")
    }
}

/// The comment a generator writes: `open`, the marker and its `options` on the first line, the
/// escaped markup, then `close` on a line of its own.
pub fn comment(open: &str, options: &str, markup: &str, close: &str) -> String {
    format!(
        "{open} {}\n{}\n{close}",
        head(options),
        escape(markup.trim_end())
    )
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

/// The comment as line comments, for languages whose block comments nest (Swift): `prefix` and the
/// marker with its `options` on the first line, then `prefix` before each escaped markup line. The
/// markup is split at `\r` as well as `\n`, because either ends a Swift line comment: no character
/// of it can end a comment line early and leave the rest of that line as code.
pub fn line_comment(prefix: &str, options: &str, markup: &str) -> String {
    let mut out = format!("{prefix} {}\n", head(options));
    for line in escape(markup.trim_end()).split(['\n', '\r']) {
        if line.is_empty() {
            out.push_str(prefix);
        } else {
            out.push_str(&format!("{prefix} {line}"));
        }
        out.push('\n');
    }
    out
}

/// The first comment `line_comment` wrote in `text`: its options and its unescaped markup, read up
/// to the first line that is not a `prefix` comment. Indentation before `prefix` does not matter.
pub fn find_lines<'t>(text: &'t str, prefix: &str) -> Option<(&'t str, String)> {
    let mut lines = text.lines();
    let options = lines.by_ref().find_map(|line| {
        let rest = line
            .trim()
            .strip_prefix(prefix)?
            .trim_start()
            .strip_prefix(MARKER)?;
        (rest.is_empty() || rest.starts_with(' ')).then(|| rest.trim())
    })?;
    let payload: Vec<&str> = lines
        .map_while(|line| {
            let rest = line.trim_start().strip_prefix(prefix)?;
            Some(rest.strip_prefix(' ').unwrap_or(rest))
        })
        .collect();
    Some((options, unescape(&payload.join("\n"))))
}

/// The document a source comment claims, when its markup parses without errors.
pub fn claimed(markup: &str, catalog: &Catalog) -> Option<(Document, Vec<Diagnostic>)> {
    let parsed = parse(
        markup,
        &ParseOptions {
            catalog: Some(catalog),
            mode: Mode::Lenient,
            ..Default::default()
        },
    );
    let document = parsed
        .document
        .filter(|_| !has_errors(&parsed.diagnostics))?;
    Some((canonicalize(&document), parsed.diagnostics))
}

/// A readable note for developers about one context entry (SPEC §2.3):
/// `<kind>[ <status>] (<by> <name>): <text>`. It is derived from the context the source comment
/// carries, so importers ignore it. Entry text is untrusted: it is cut at the format's limit,
/// control and line-separator characters become spaces so the note stays on one line of a line
/// comment, and it is escaped like the source comment.
pub fn note(kind: &str, status: Option<&str>, by: &str, name: &str, text: &str) -> String {
    let status = status.map(|s| format!(" {s}")).unwrap_or_default();
    let text: String = text.chars().take(MAX_NOTE).collect();
    let line: String = format!("{kind}{status} ({by} {name}): {text}")
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                ' '
            } else {
                c
            }
        })
        .collect();
    escape(&line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_cannot_close_or_nest_comments() {
        let markup = r#"<text id="t">a --> b --!> c <!-- d */ e \ f -- g @license @\ @@ @</text>"#;
        let escaped = escape(markup);
        for bad in ["-->", "--!>", "<!--", "*/", "--", "@license"] {
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

    #[test]
    fn line_comments_read_back_and_keep_every_line_a_comment() {
        let markup =
            "<screen id=\"s\" weft=\"0.1\">\n  <text id=\"t\">a @x \\ */</text>\n</screen>";
        let swift = format!(
            "// other\n{}\nimport SwiftUI\n",
            line_comment("//", "swiftui", markup)
        );
        let (options, found) = find_lines(&swift, "//").unwrap();
        assert_eq!((options, found.as_str()), ("swiftui", markup));
        // A formatter's indentation does not matter.
        let indented: String = swift.lines().map(|l| format!("  {l}\n")).collect();
        assert_eq!(find_lines(&indented, "//").unwrap().1, markup);
        let broken = line_comment("//", "", "a\rstruct X {}\nb");
        assert!(broken.lines().all(|l| l.starts_with("//")), "{broken}");
        assert!(find_lines("// weft:sourced\n// x", "//").is_none());
    }

    #[test]
    fn notes_stay_on_one_line() {
        let line = note(
            "todo",
            Some("open"),
            "agent",
            "m",
            "a\nb\r\u{2028}c @license",
        );
        assert_eq!(line, "todo open (agent m): a b  c @\\license");
    }
}
