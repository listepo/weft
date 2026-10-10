//! Claims about the diagnostic type and its helpers: what a repairing model reads is stable.

#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::json;
use weft_core::{Code, Diagnostic, Mode, Position, Severity, did_you_mean, has_errors};

fn error() -> Diagnostic {
    Diagnostic::new(Code::W203, "/screen#s", "Bad.", "one of: \"a\"")
}

#[test]
fn a_new_diagnostic_is_resolved_for_lenient_mode_and_has_an_expectation() {
    let d = error();
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.expected.as_deref(), Some("one of: \"a\""));
    assert_eq!(
        (d.line, d.column, d.got.as_deref(), d.hint.as_deref()),
        (None, None, None, None)
    );
    let w = Diagnostic::new(Code::W401, "/", "m", "e");
    assert_eq!(w.severity, Severity::Warning);
}

#[test]
fn mode_re_resolves_only_the_mode_dependent_codes() {
    let w = Diagnostic::new(Code::W402, "/", "m", "e").mode(Mode::Strict);
    assert_eq!(w.severity, Severity::Error);
    let back = w.mode(Mode::Lenient);
    assert_eq!(back.severity, Severity::Warning);
    assert_eq!(error().mode(Mode::Lenient).severity, Severity::Error);
    assert_eq!(
        Diagnostic::new(Code::W602, "/", "m", "e")
            .mode(Mode::Strict)
            .severity,
        Severity::Warning
    );
}

#[test]
fn builders_set_position_got_and_hint_and_a_missing_position_changes_nothing() {
    let d = error()
        .pos(Some(Position { line: 3, column: 7 }))
        .got("x")
        .hint("fix it");
    assert_eq!((d.line, d.column), (Some(3), Some(7)));
    assert_eq!(d.got.as_deref(), Some("x"));
    assert_eq!(d.hint.as_deref(), Some("fix it"));
    let untouched = error().pos(None);
    assert_eq!((untouched.line, untouched.column), (None, None));
    assert_eq!(error().got_opt(None).got, None);
    assert_eq!(
        error().hint_opt(Some("h".into())).hint.as_deref(),
        Some("h")
    );
}

#[test]
fn a_diagnostic_serializes_to_the_spec_shape_and_skips_what_is_absent() {
    assert_eq!(
        serde_json::to_value(error()).unwrap(),
        json!({"code": "W203", "severity": "error", "message": "Bad.", "path": "/screen#s", "expected": "one of: \"a\""})
    );
    let full = error()
        .pos(Some(Position { line: 1, column: 2 }))
        .got("g")
        .hint("h");
    assert_eq!(
        serde_json::to_value(full).unwrap(),
        json!({"code": "W203", "severity": "error", "message": "Bad.", "path": "/screen#s",
               "line": 1, "column": 2, "expected": "one of: \"a\"", "got": "g", "hint": "h"})
    );
}

#[test]
fn a_warning_serializes_with_the_lowercase_severity() {
    let w = Diagnostic::new(Code::W401, "/", "m", "e");
    assert_eq!(
        serde_json::to_value(w).unwrap()["severity"],
        json!("warning")
    );
}

#[test]
fn a_position_defaults_to_zero_and_a_mode_defaults_to_lenient() {
    assert_eq!(Position::default(), Position { line: 0, column: 0 });
    assert_eq!(Mode::default(), Mode::Lenient);
}

#[test]
fn has_errors_ignores_warnings_and_an_empty_list() {
    let warning = Diagnostic::new(Code::W401, "/", "m", "e");
    assert!(!has_errors(&[]));
    assert!(!has_errors(std::slice::from_ref(&warning)));
    assert!(has_errors(&[warning.clone(), error()]));
    assert!(has_errors(&[warning.mode(Mode::Strict)]));
}

#[test]
fn codes_are_their_own_name_in_text_and_json() {
    assert_eq!(Code::W101.as_str(), "W101");
    assert_eq!(serde_json::to_value(Code::W509).unwrap(), json!("W509"));
    assert_eq!(Code::ALL.first(), Some(&Code::W101));
    assert_eq!(Code::ALL.last(), Some(&Code::W808));
}

#[test]
fn did_you_mean_suggests_a_close_candidate_in_a_quoted_sentence() {
    let states = ["idle", "submitting", "invalid"];
    assert_eq!(
        did_you_mean("submiting", states).as_deref(),
        Some("did you mean \"submitting\"?")
    );
    assert_eq!(
        did_you_mean("idl", states).as_deref(),
        Some("did you mean \"idle\"?")
    );
}

#[test]
fn did_you_mean_ignores_case_differences_and_prefers_them() {
    assert_eq!(
        did_you_mean("IDLE", ["idle", "idol"]).as_deref(),
        Some("did you mean \"idle\"?")
    );
    assert_eq!(
        did_you_mean("Idol", ["idle", "idol"]).as_deref(),
        Some("did you mean \"idol\"?")
    );
}

#[test]
fn did_you_mean_is_silent_for_a_word_that_is_already_a_candidate_or_too_far_away() {
    assert_eq!(did_you_mean("idle", ["idle", "busy"]), None);
    assert_eq!(did_you_mean("zzzzzz", ["idle", "busy"]), None);
    assert_eq!(did_you_mean("x", Vec::<String>::new()), None);
    assert_eq!(did_you_mean("", ["idle"]), None);
}

#[test]
fn the_typo_allowance_grows_with_the_word_length() {
    // One edit for words up to three units, a third of the length (at least two) above that.
    assert_eq!(
        did_you_mean("abc", ["abd"]).as_deref(),
        Some("did you mean \"abd\"?")
    );
    assert_eq!(did_you_mean("abc", ["axd"]), None);
    assert!(did_you_mean("abcdef", ["abxxef"]).is_some());
    assert!(did_you_mean("abcdef", ["axxxef"]).is_none());
    assert!(did_you_mean("abcdefghi", ["abcdefxxx"]).is_some());
}

#[test]
fn did_you_mean_measures_utf16_units_so_an_astral_character_costs_two_edits() {
    // U+1F600 is two UTF-16 units: replacing it by "a" takes two edits, over the limit of one
    // for a word of three units. The TypeScript core measures the same way.
    assert_eq!(did_you_mean("\u{1F600}b", ["ab"]), None);
    assert_eq!(
        did_you_mean("\u{e9}b", ["eb"]).as_deref(),
        Some("did you mean \"eb\"?")
    );
}

#[test]
fn did_you_mean_accepts_any_string_like_candidates() {
    let owned: Vec<String> = vec!["alpha".into(), "beta".into()];
    assert_eq!(
        did_you_mean("alpa", &owned).as_deref(),
        Some("did you mean \"alpha\"?")
    );
    assert_eq!(
        did_you_mean("alpa", owned.iter().map(String::as_str)).as_deref(),
        Some("did you mean \"alpha\"?")
    );
}

#[test]
fn quotes_in_a_suggestion_are_escaped_as_json_would() {
    assert_eq!(
        did_you_mean("a\"c", ["a\"b"]).as_deref(),
        Some("did you mean \"a\\\"b\"?")
    );
}
