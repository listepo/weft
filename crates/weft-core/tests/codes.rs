//! The diagnostic registry is an API (SPEC §6.2): every code has a meaning, a severity and at
//! least one input that produces it. A new code without a case here fails this file.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeSet;

use common::{catalog, codes, parse_lenient};
use serde_json::{Value as Json, json};
use weft_core::{
    ApplyOptions, Code, MAX_DEPTH, Mode, ParseOptions, Severity, ValidateOptions, apply_patches,
    parse, parse_json, validate,
};

const SPEC: &str = include_str!("../../../SPEC.md");
const FIXTURE: &str = include_str!("fixtures/differential.json");

/// Codes of the import layer, produced by the importer package and never by the core.
const IMPORT: [&str; 2] = ["W601", "W602"];

enum Case {
    /// Markup parsed without a catalog: the syntax layer only.
    Syntax(String),
    /// Markup parsed against the fixture catalog, lenient.
    Markup(String),
    /// Markup parsed with the fixture tokens and the action list `["known"]`.
    Checked(String),
    /// JSON validated against the fixture catalog, lenient.
    Json(Json),
    /// Patches applied to the fixture's patch base.
    Patch(Json),
}

fn in_screen(inner: &str) -> Case {
    Case::Markup(common::screen(inner))
}

fn root(children: Json) -> Json {
    json!({"weft": "0.1", "root": {"kind": "screen", "id": "s", "children": children}})
}

fn cases() -> Vec<(&'static str, Case)> {
    let syntax = |code, markup: &str| (code, Case::Syntax(markup.to_owned()));
    let patch = |code, p: Json| (code, Case::Patch(p));
    vec![
        syntax("W101", "<a><</a>"),
        syntax("W102", "<?xml version=\"1.0\"?><a/>"),
        syntax("W103", "<!DOCTYPE a><a/>"),
        syntax("W104", "<a><![CDATA[x]]></a>"),
        syntax("W105", "<A/>"),
        syntax("W106", "<a b=c/>"),
        syntax("W107", "<a b/>"),
        syntax("W108", "<a b=\"1\" b=\"2\"/>"),
        syntax("W109", "<a><b></c></a>"),
        syntax("W110", "<a><b></b>"),
        syntax("W111", "<a/></b>"),
        syntax("W112", "<a>&nope;</a>"),
        syntax("W113", "<a b=\"<\"/>"),
        syntax("W114", "<a/>text"),
        syntax("W115", "<a><!-- x -- y --></a>"),
        syntax("W116", "<a b=\"{oops}\"/>"),
        ("W117", Case::Syntax("<a>".repeat(MAX_DEPTH + 1))),
        syntax("W118", "<a><slot/></a>"),
        syntax("W119", "<a><slot name=\"s\"/><slot name=\"s\"/></a>"),
        ("W200", Case::Json(json!({"weft": "0.1"}))),
        (
            "W201",
            Case::Json(json!({"weft": "0.1", "root": {"kind": "stack", "id": "a"}})),
        ),
        (
            "W202",
            Case::Json(json!({"weft": "0.1", "root": {"kind": "screen"}})),
        ),
        (
            "W203",
            in_screen("<stack id=\"a\" direction=\"diagonal\"/>"),
        ),
        (
            "W204",
            in_screen("<heading id=\"a\" level=\"two\">T</heading>"),
        ),
        ("W205", in_screen("<heading id=\"a\">T</heading>")),
        (
            "W206",
            in_screen("<button id=\"a\" on-nope=\"go\">T</button>"),
        ),
        (
            "W207",
            in_screen("<stack id=\"a\"><slot name=\"x\"><stack id=\"b\"/></slot></stack>"),
        ),
        ("W208", in_screen("<dialog id=\"a\" label=\"L\"/>")),
        ("W209", in_screen("<stack id=\"a\" role=\"group\"/>")),
        ("W210", in_screen("<x-acme-box id=\"a\"/>")),
        ("W211", in_screen("<x-acme-box id=\"a\" role=\"nope\"/>")),
        ("W212", in_screen("<stack id=\"1a\"/>")),
        ("W213", in_screen("<text id=\"a\" text=\"Hi {$.name}\"/>")),
        ("W214", in_screen("<text id=\"a\" text=\"{$.a..b}\"/>")),
        ("W215", in_screen("<stack id=\"a\" gap=\"{token.a b}\"/>")),
        (
            "W216",
            in_screen("<button id=\"a\" on-press=\"Bad Action\">T</button>"),
        ),
        (
            "W217",
            in_screen("<button id=\"a\" submit=\"{$.x}\">T</button>"),
        ),
        ("W218", in_screen("<text id=\"a\" text=\"{!$.x}\"/>")),
        (
            "W219",
            Case::Json(json!({"weft": "one", "root": {"kind": "screen", "id": "s"}})),
        ),
        ("W220", in_screen("<x-foo id=\"a\" role=\"group\"/>")),
        (
            "W221",
            Case::Json(root(
                json!([{"kind": "text", "id": "t", "children": ["a\u{1}b"]}]),
            )),
        ),
        ("W222", in_screen("<each id=\"a\"><stack id=\"b\"/></each>")),
        (
            "W223",
            Case::Json(
                json!({"weft": "0.1", "root": {"kind": "screen", "id": "s", "props": {"id": "x"}}}),
            ),
        ),
        (
            "W224",
            in_screen("<heading id=\"a\" level=\"9\">T</heading>"),
        ),
        ("W301", in_screen("<stack id=\"a\"/><stack id=\"a\"/>")),
        (
            "W302",
            in_screen("<select id=\"a\" label=\"L\"><stack id=\"b\"/></select>"),
        ),
        ("W303", in_screen("<option id=\"a\" value=\"v\">T</option>")),
        (
            "W304",
            in_screen("<field id=\"a\" label=\"L\">text</field>"),
        ),
        ("W305", in_screen("<text id=\"a\" text=\"{$x.y}\"/>")),
        (
            "W306",
            Case::Checked(common::screen(
                "<stack id=\"a\" gap=\"{token.space.huge}\"/>",
            )),
        ),
        (
            "W307",
            Case::Checked(common::screen(
                "<stack id=\"a\" gap=\"{token.color.accent}\"/>",
            )),
        ),
        (
            "W308",
            Case::Checked(common::screen(
                "<button id=\"a\" on-press=\"go\">T</button>",
            )),
        ),
        (
            "W309",
            in_screen("<tabs id=\"a\" selected=\"nope\"><tab id=\"b\" label=\"B\"/></tabs>"),
        ),
        ("W310", in_screen("<text id=\"a\" text=\"x\">y</text>")),
        (
            "W311",
            in_screen(
                "<each id=\"a\" as=\"x\" in=\"{$.xs}\"><each id=\"b\" as=\"x\" in=\"{$x.ys}\"><stack id=\"c\"/></each></each>",
            ),
        ),
        ("W312", in_screen("<screen id=\"inner\" weft=\"0.1\"/>")),
        (
            "W313",
            in_screen("<button id=\"a\" submit=\"true\">T</button>"),
        ),
        ("W314", in_screen("<each id=\"a\" as=\"x\" in=\"{$.xs}\"/>")),
        ("W401", in_screen("<mystery id=\"a\"/>")),
        ("W402", in_screen("<stack id=\"a\" shape=\"round\"/>")),
        (
            "W403",
            Case::Json(json!({"weft": "0.9", "root": {"kind": "screen", "id": "s"}})),
        ),
        (
            "W404",
            Case::Json(json!({"weft": "1.0", "root": {"kind": "screen", "id": "s"}})),
        ),
        patch("W501", json!({"op": "remove"})),
        patch("W502", json!([{"op": "remove", "id": "nope"}])),
        patch(
            "W503",
            json!([{"op": "set", "id": "go", "prop": "id", "value": "x"}]),
        ),
        patch(
            "W504",
            json!([{"op": "insert", "parent": "go", "slot": "nope", "markup": "<stack id=\"n\"/>"}]),
        ),
        patch(
            "W505",
            json!([{"op": "insert", "parent": "main", "index": 99, "markup": "<stack id=\"n\"/>"}]),
        ),
        patch(
            "W506",
            json!([{"op": "move", "id": "main", "parent": "go"}]),
        ),
        patch("W507", json!([{"op": "remove", "id": "s"}])),
        patch(
            "W508",
            json!([{"op": "insert", "parent": "main", "markup": "text"}]),
        ),
        patch(
            "W509",
            json!([{"op": "insert", "parent": "main", "markup": "<stack id=\"go\"/>"}]),
        ),
    ]
}

fn run(case: &Case) -> Vec<&'static str> {
    let catalog = catalog();
    match case {
        Case::Syntax(markup) => codes(&parse(markup, &ParseOptions::default()).diagnostics),
        Case::Markup(markup) => codes(&parse_lenient(markup).diagnostics),
        Case::Checked(markup) => {
            let tokens = common::tokens();
            let actions = vec!["known".to_owned()];
            let options = ParseOptions {
                catalog: Some(&catalog),
                tokens: Some(&tokens),
                actions: Some(&actions),
                ..ParseOptions::default()
            };
            codes(&parse(markup, &options).diagnostics)
        }
        Case::Json(input) => {
            let options = ValidateOptions {
                catalog: Some(&catalog),
                ..ValidateOptions::default()
            };
            codes(&validate(input, &options))
        }
        Case::Patch(patches) => {
            let fixture = parse_json(FIXTURE).unwrap();
            let base = parse(
                fixture["patchBase"].as_str().unwrap(),
                &ParseOptions {
                    catalog: Some(&catalog),
                    ..ParseOptions::default()
                },
            )
            .document
            .unwrap();
            codes(&apply_patches(&base, patches, &ApplyOptions::new(&catalog)).diagnostics)
        }
    }
}

#[test]
fn the_registry_lists_exactly_the_codes_of_the_spec_table() {
    let spec: BTreeSet<String> = SPEC
        .lines()
        .filter_map(|l| l.strip_prefix("| W"))
        .filter_map(|l| l.split(' ').next())
        .map(|digits| format!("W{digits}"))
        .collect();
    let registry: BTreeSet<String> = Code::ALL.iter().map(|c| c.as_str().to_owned()).collect();
    assert_eq!(registry, spec);
}

#[test]
fn every_code_has_a_summary_that_is_one_sentence() {
    for code in Code::ALL {
        let summary = code.summary();
        assert!(summary.ends_with('.'), "{}: {summary}", code.as_str());
        assert!(summary.len() > 8, "{}", code.as_str());
    }
}

#[test]
fn every_core_code_is_produced_by_some_input() {
    let cases = cases();
    let covered: BTreeSet<&str> = cases.iter().map(|(code, _)| *code).collect();
    for code in Code::ALL {
        let name = code.as_str();
        assert!(
            covered.contains(name) || IMPORT.contains(&name),
            "no case produces {name}"
        );
    }
    for (code, case) in &cases {
        let got = run(case);
        assert!(got.contains(code), "{code} is not produced: {got:?}");
    }
}

#[test]
fn severities_are_errors_except_the_mode_codes_and_the_import_warning() {
    for code in Code::ALL {
        let (lenient, strict) = (code.severity(Mode::Lenient), code.severity(Mode::Strict));
        match code.as_str() {
            "W401" | "W402" | "W403" => {
                assert_eq!((lenient, strict), (Severity::Warning, Severity::Error));
            }
            "W602" => assert_eq!((lenient, strict), (Severity::Warning, Severity::Warning)),
            other => assert_eq!(
                (lenient, strict),
                (Severity::Error, Severity::Error),
                "{other}"
            ),
        }
    }
}
