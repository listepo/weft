//! Partial parsing (SPEC §6.3): markup that stops anywhere reads as far as it goes.

#![allow(clippy::unwrap_used, clippy::panic)]

use weft_core::{Code, ParseOptions, ParseResult, parse_partial, stringify};

fn read(markup: &str) -> ParseResult {
    parse_partial(markup, &ParseOptions::default())
}

fn pending(markup: &str) -> Vec<&'static str> {
    read(markup)
        .pending
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

fn json(markup: &str) -> String {
    stringify(&read(markup).document.unwrap())
}

const ROOT: &str = "<screen id=\"s\" weft=\"0.1\">";

#[test]
fn a_finished_document_has_nothing_pending() {
    let r = read(&format!("{ROOT}<text id=\"t\">Hi</text></screen>"));
    assert!(r.document.is_some());
    assert!(r.diagnostics.is_empty() && r.pending.is_empty());
}

#[test]
fn every_open_element_is_pending_and_kept() {
    let markup = format!("{ROOT}<stack id=\"a\"><text id=\"t\">Hel");
    let r = read(&markup);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    assert_eq!(pending(&markup), ["W110", "W110", "W110"]);
    let out = json(&markup);
    assert!(out.contains("\"Hel\""), "{out}");
    assert!(out.contains("\"stack\""), "{out}");
}

#[test]
fn an_unfinished_start_tag_waits_for_the_next_chunk() {
    for tail in [
        "<",
        "<te",
        "<text",
        "<text id",
        "<text id=",
        "<text id=\"t",
        "<text id=\"t\"",
        "<text id=\"t\" text=\"Hel",
        "<text id=\"t\"/",
    ] {
        let markup = format!("{ROOT}{tail}");
        let r = read(&markup);
        assert!(r.diagnostics.is_empty(), "{tail}: {:?}", r.diagnostics);
        assert!(!json(&markup).contains("\"text\""), "{tail}");
    }
    assert_eq!(pending(&format!("{ROOT}<text id=\"t\"")), ["W110", "W110"]);
}

#[test]
fn an_unfinished_end_tag_or_reference_is_not_an_error() {
    for tail in ["</", "</te", "</text", "</text "] {
        let markup = format!("{ROOT}<text id=\"t\">Hi{tail}");
        let r = read(&markup);
        assert!(r.diagnostics.is_empty(), "{tail}: {:?}", r.diagnostics);
        assert!(json(&markup).contains("\"Hi\""), "{tail}");
    }
    for tail in ["&", "&am", "&#", "&#x2", "&#16"] {
        let markup = format!("{ROOT}<text id=\"t\">Hi {tail}");
        let r = read(&markup);
        assert!(r.diagnostics.is_empty(), "{tail}: {:?}", r.diagnostics);
        assert!(json(&markup).contains("\"Hi\""), "{tail}");
    }
}

#[test]
fn an_unfinished_comment_is_pending() {
    for tail in ["<!", "<!-", "<!--", "<!-- note"] {
        let markup = format!("{ROOT}{tail}");
        assert!(read(&markup).diagnostics.is_empty(), "{tail}");
    }
    assert_eq!(pending(&format!("{ROOT}<!-- note")), ["W110", "W115"]);
}

#[test]
fn nothing_readable_yet_gives_no_document() {
    for markup in [
        "",
        "  ",
        "<",
        "<scr",
        "<screen id=\"s\"",
        "<screen id=\"s\" weft=\"0.1\"",
    ] {
        let r = read(markup);
        assert!(r.document.is_none(), "{markup:?}");
        assert!(r.diagnostics.is_empty(), "{markup:?}: {:?}", r.diagnostics);
        assert!(!r.pending.is_empty(), "{markup:?}");
    }
}

#[test]
fn a_mistake_in_the_finished_part_is_still_an_error() {
    let r = read(&format!("{ROOT}<a id=\"a\"></b><text id=\"t\">Hi"));
    assert!(r.document.is_none());
    assert_eq!(r.diagnostics[0].code, Code::W109);
    let r = read(&format!("{ROOT}<text id=\"t\" text=x/>"));
    assert_eq!(r.diagnostics[0].code, Code::W106);
}

#[test]
fn complete_parsing_is_unchanged() {
    let r = weft_core::parse(
        &format!("{ROOT}<text id=\"t\">Hi"),
        &ParseOptions::default(),
    );
    assert!(r.document.is_none());
    assert!(r.pending.is_empty());
    assert_eq!(r.diagnostics.len(), 2);
}

#[test]
fn any_cut_of_any_markup_is_handled() {
    let markup = format!(
        "{ROOT}<!-- c --><stack id=\"a\" gap=\"x &amp; y\"><slot name=\"s\"><text id=\"t\">É&#233;😀</text></slot></stack></screen>"
    );
    for (cut, _) in markup.char_indices() {
        let r = read(&markup[..cut]);
        assert!(r.diagnostics.is_empty() || r.document.is_none(), "{cut}");
    }
}
