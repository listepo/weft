//! Properties that hold for every input: nothing panics, the writers are stable, and the two
//! spellings of a document (markup and JSON) mean the same thing. The configuration is fixed so
//! a failure reproduces on every machine.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeSet;

use common::{catalog, codes, on_big_stack, parse_lenient, tokens};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, RngSeed};
use serde_json::{Map, Value as Json, json};
use weft_core::{
    ApplyOptions, Document, Mode, ParseOptions, ValidateOptions, apply_patches, canonicalize,
    parse, serialize, stringify, validate, validate_document,
};

fn config() -> Config {
    Config {
        cases: 96,
        max_shrink_iters: 512,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0x57EF7),
        ..Config::default()
    }
}

// ---- generators --------------------------------------------------------------------------

/// Characters that make markup interesting: delimiters, entities, whitespace, comments, braces
/// of references, and a few astral and combining characters for the UTF-16 columns.
fn markup_soup() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        Just("<".to_owned()),
        Just(">".to_owned()),
        Just("/".to_owned()),
        Just("=".to_owned()),
        Just("\"".to_owned()),
        Just("'".to_owned()),
        Just("&".to_owned()),
        Just("&amp;".to_owned()),
        Just("&#x41;".to_owned()),
        Just(";".to_owned()),
        Just(" ".to_owned()),
        Just("\n".to_owned()),
        Just("\r\n".to_owned()),
        Just("\t".to_owned()),
        Just("<!--".to_owned()),
        Just("-->".to_owned()),
        Just("--".to_owned()),
        Just("<![CDATA[".to_owned()),
        Just("<?".to_owned()),
        Just("{".to_owned()),
        Just("}".to_owned()),
        Just("{$.a}".to_owned()),
        Just("{token.space.md}".to_owned()),
        Just("<screen id=\"s\" weft=\"0.1\">".to_owned()),
        Just("</screen>".to_owned()),
        Just("<stack id=\"a\">".to_owned()),
        Just("</stack>".to_owned()),
        Just("<text id=\"t\">".to_owned()),
        Just("</text>".to_owned()),
        Just("<slot name=\"x\">".to_owned()),
        Just("</slot>".to_owned()),
        Just("\u{1F600}".to_owned()),
        Just("\u{FEFF}".to_owned()),
        Just("e\u{301}".to_owned()),
        "[a-z-]{1,6}",
        "\\PC{1,4}",
    ];
    prop::collection::vec(piece, 0..40).prop_map(|v| v.concat())
}

fn any_text() -> impl Strategy<Value = String> {
    prop_oneof!["\\PC{0,24}", markup_soup()]
}

fn json_scalar() -> impl Strategy<Value = Json> {
    prop_oneof![
        Just(Json::Null),
        any::<bool>().prop_map(Json::Bool),
        any::<i32>().prop_map(|n| json!(n)),
        (-1.0e6..1.0e6f64).prop_map(|n| json!(n)),
        any_text().prop_map(Json::String),
        Just(json!("screen")),
        Just(json!("stack")),
        Just(json!("0.1")),
    ]
}

/// Arbitrary JSON, weighted toward the member names the validator looks at.
fn any_json() -> impl Strategy<Value = Json> {
    let key = prop_oneof![
        Just("kind".to_owned()),
        Just("id".to_owned()),
        Just("props".to_owned()),
        Just("children".to_owned()),
        Just("slots".to_owned()),
        Just("on".to_owned()),
        Just("weft".to_owned()),
        Just("root".to_owned()),
        Just("bind".to_owned()),
        Just("token".to_owned()),
        Just("not".to_owned()),
        "[a-z0-9-]{0,6}",
    ];
    json_scalar().prop_recursive(6, 64, 6, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Json::Array),
            prop::collection::vec((key.clone(), inner), 0..5)
                .prop_map(|m| Json::Object(m.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

/// A document-shaped JSON value: the right skeleton with arbitrary pieces inside it.
fn document_like() -> impl Strategy<Value = Json> {
    (any_json(), any_json(), any_json(), any::<bool>()).prop_map(|(props, children, slots, ok)| {
        json!({
            "weft": if ok { "0.1" } else { "9.9" },
            "root": {"kind": "screen", "id": "s", "props": props, "children": children, "slots": slots}
        })
    })
}

#[derive(Clone, Debug)]
enum El {
    Stack {
        direction: Option<&'static str>,
        wrap: Option<bool>,
        gap: Option<&'static str>,
        kids: Vec<El>,
    },
    Text(Option<&'static str>, String),
    Heading(u8, String),
    Button(Option<&'static str>, Option<bool>, String),
    Field(String, Option<String>),
}

fn element(valid: bool) -> impl Strategy<Value = El> {
    let direction = if valid {
        prop_oneof![Just(None), Just(Some("row")), Just(Some("column"))].boxed()
    } else {
        prop_oneof![Just(None), Just(Some("row")), Just(Some("diagonal"))].boxed()
    };
    let gap = if valid {
        prop_oneof![
            Just(None),
            Just(Some("{token.space.sm}")),
            Just(Some("{token.space.md}")),
            Just(Some("{$.gap}")),
        ]
        .boxed()
    } else {
        prop_oneof![
            Just(None),
            Just(Some("{token.space.md}")),
            Just(Some("{token.color.accent}")),
            Just(Some("12")),
        ]
        .boxed()
    };
    let tone = prop_oneof![Just(None), Just(Some("muted")), Just(Some("danger"))];
    let level = if valid { 1u8..=6 } else { 0u8..=9 };
    let leaf = prop_oneof![
        (tone, any_text()).prop_map(|(t, s)| El::Text(t, s)),
        (level, any_text()).prop_map(|(l, s)| El::Heading(l, s)),
        (
            prop_oneof![Just(None), Just(Some("primary")), Just(Some("danger"))],
            prop::option::of(any::<bool>()),
            any_text()
        )
            .prop_map(|(v, d, s)| El::Button(v, d, s)),
        (any_text(), prop::option::of(any_text())).prop_map(|(l, v)| El::Field(l, v)),
    ];
    leaf.prop_recursive(4, 24, 4, move |inner| {
        (
            direction.clone(),
            prop::option::of(any::<bool>()),
            gap.clone(),
            prop::collection::vec(inner, 0..4),
        )
            .prop_map(|(direction, wrap, gap, kids)| El::Stack {
                direction,
                wrap,
                gap,
                kids,
            })
    })
}

/// A literal attribute value: a `{` makes a reference, or a reference mixed with text, so none
/// are generated.
fn literal(s: &str) -> String {
    s.replace('{', "(")
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render(el: &El, next: &mut u32, out: &mut String) {
    let id = *next;
    *next += 1;
    let attr = |name: &str, value: Option<String>| {
        value.map_or(String::new(), |v| format!(" {name}=\"{}\"", escape(&v)))
    };
    match el {
        El::Stack {
            direction,
            wrap,
            gap,
            kids,
        } => {
            out.push_str(&format!(
                "<stack id=\"e{id}\"{}{}{}>",
                attr("direction", direction.map(str::to_owned)),
                attr("wrap", wrap.map(|w| w.to_string())),
                attr("gap", gap.map(str::to_owned)),
            ));
            for kid in kids {
                render(kid, next, out);
            }
            out.push_str("</stack>");
        }
        El::Text(tone, s) => out.push_str(&format!(
            "<text id=\"e{id}\"{}>{}</text>",
            attr("tone", tone.map(str::to_owned)),
            escape(s)
        )),
        El::Heading(level, s) => out.push_str(&format!(
            "<heading id=\"e{id}\" level=\"{level}\">{}</heading>",
            escape(s)
        )),
        El::Button(variant, disabled, s) => out.push_str(&format!(
            "<button id=\"e{id}\"{}{}>{}</button>",
            attr("variant", variant.map(str::to_owned)),
            attr("disabled", disabled.map(|d| d.to_string())),
            escape(s)
        )),
        El::Field(label, value) => out.push_str(&format!(
            "<field id=\"e{id}\" label=\"{}\"{}/>",
            escape(&literal(label)),
            attr("value", value.as_deref().map(literal))
        )),
    }
}

fn markup_of(kids: &[El]) -> String {
    let mut out = String::from("<screen id=\"root\" weft=\"0.1\">");
    let mut next = 0;
    for kid in kids {
        render(kid, &mut next, &mut out);
    }
    out.push_str("</screen>");
    out
}

fn valid_markup() -> impl Strategy<Value = String> {
    prop::collection::vec(element(true), 0..4).prop_map(|kids| markup_of(&kids))
}

fn loose_markup() -> impl Strategy<Value = String> {
    prop::collection::vec(element(false), 0..4).prop_map(|kids| markup_of(&kids))
}

// ---- helpers -----------------------------------------------------------------------------

fn lenient_options(catalog: &weft_core::Catalog) -> ParseOptions<'_> {
    ParseOptions {
        catalog: Some(catalog),
        ..ParseOptions::default()
    }
}

fn validated_codes(input: &Json) -> BTreeSet<&'static str> {
    let catalog = catalog();
    let options = ValidateOptions {
        catalog: Some(&catalog),
        ..ValidateOptions::default()
    };
    codes(&validate(input, &options)).into_iter().collect()
}

fn document_codes(document: &Document) -> BTreeSet<&'static str> {
    let catalog = catalog();
    let options = ValidateOptions {
        catalog: Some(&catalog),
        ..ValidateOptions::default()
    };
    codes(&validate_document(document, &options))
        .into_iter()
        .collect()
}

fn parse_with_everything(markup: &str, mode: Mode) {
    let catalog = catalog();
    let tokens = tokens();
    let actions = vec!["known".to_owned()];
    let options = ParseOptions {
        catalog: Some(&catalog),
        mode,
        tokens: Some(&tokens),
        actions: Some(&actions),
    };
    let _ = parse(markup, &options);
    let _ = parse(markup, &ParseOptions::default());
}

// ---- no input panics ---------------------------------------------------------------------

proptest! {
    #![proptest_config(config())]

    #[test]
    fn parsing_never_panics_on_any_string(s in any::<String>()) {
        parse_with_everything(&s, Mode::Lenient);
    }

    #[test]
    fn parsing_never_panics_on_markup_like_strings(s in markup_soup()) {
        parse_with_everything(&s, Mode::Lenient);
        parse_with_everything(&s, Mode::Strict);
    }

    #[test]
    fn reading_json_never_panics_on_any_string(s in any::<String>(), soup in markup_soup()) {
        let _ = weft_core::parse_json(&s);
        let _ = weft_core::parse_json(&soup);
        let _ = weft_core::parse_json(&format!("[{soup}]"));
    }

    #[test]
    fn validating_never_panics_on_arbitrary_json(input in any_json()) {
        let _ = validated_codes(&input);
    }

    #[test]
    fn validating_never_panics_on_document_shaped_json(input in document_like()) {
        let _ = validated_codes(&input);
    }

    #[test]
    fn applying_patches_never_panics_on_arbitrary_json(patches in any_json()) {
        let catalog = catalog();
        let base = common::document(&common::screen("<stack id=\"a\"><text id=\"b\">x</text></stack>"));
        let _ = apply_patches(&base, &patches, &ApplyOptions::new(&catalog));
    }

    #[test]
    fn applying_patches_never_panics_on_patch_shaped_json(
        ops in prop::collection::vec(
            (
                prop_oneof![Just("set".to_owned()), Just("insert".to_owned()), Just("remove".to_owned()), Just("move".to_owned()), "[a-z]{0,6}"],
                prop_oneof![Just("a".to_owned()), Just("b".to_owned()), Just("root".to_owned()), "[a-z]{0,4}"],
                prop_oneof![Just("a".to_owned()), Just("b".to_owned()), Just("root".to_owned()), "[a-z]{0,4}"],
                prop::option::of(any_json()),
                prop::option::of(any_json()),
                prop::option::of(any_text()),
            ),
            0..5,
        )
    ) {
        let catalog = catalog();
        let base = common::document(&common::screen("<stack id=\"a\"><text id=\"b\">x</text></stack>"));
        let patches: Vec<Json> = ops.into_iter().map(|(op, id, parent, value, index, markup)| {
            let mut m = Map::new();
            m.insert("op".into(), json!(op));
            m.insert("id".into(), json!(id));
            m.insert("parent".into(), json!(parent));
            m.insert("prop".into(), json!("tone"));
            if let Some(v) = value { m.insert("value".into(), v); }
            if let Some(i) = index { m.insert("index".into(), i); }
            if let Some(s) = markup { m.insert("markup".into(), json!(s)); }
            Json::Object(m)
        }).collect();
        let _ = apply_patches(&base, &Json::Array(patches), &ApplyOptions::new(&catalog));
    }
}

/// `json!` copies what it embeds, which would make building a deep value quadratic.
fn wrap(key: &str, inner: Json) -> Json {
    let mut m = Map::new();
    m.insert(key.to_owned(), inner);
    Json::Object(m)
}

fn nested_array(depth: usize) -> Json {
    (0..depth).fold(json!("x"), |inner, _| Json::Array(vec![inner]))
}

#[test]
fn a_deeply_nested_json_value_is_refused_not_a_crash() {
    on_big_stack(|| {
        for depth in [10, 300, 800, 5_000] {
            let mut node = json!({"kind": "text", "id": "leaf"});
            for i in 0..depth {
                let mut m = Map::new();
                m.insert("kind".into(), json!("stack"));
                m.insert("id".into(), json!(format!("n{i}")));
                m.insert("children".into(), Json::Array(vec![node]));
                node = Json::Object(m);
            }
            let mut root = Map::new();
            root.insert("kind".into(), json!("screen"));
            root.insert("id".into(), json!("s"));
            root.insert("children".into(), Json::Array(vec![node]));
            let input = wrap("root", Json::Object(root));
            let input = match input {
                Json::Object(mut m) => {
                    m.insert("weft".into(), json!("0.1"));
                    Json::Object(m)
                }
                other => other,
            };
            let found = validated_codes(&input);
            if depth > 400 {
                assert!(found.contains("W200"), "depth {depth}: {found:?}");
            }
            // The same value as nested arrays and objects of no meaning at all.
            let junk = (0..depth).fold(json!(1), |inner, _| {
                Json::Array(vec![wrap("children", inner)])
            });
            let _ = validated_codes(&junk);
            let _ = validated_codes(&wrap("root", junk));
        }
    });
}

#[test]
fn deeply_nested_json_text_is_refused_not_a_crash() {
    on_big_stack(|| {
        for depth in [100, 2_000, 100_000] {
            let text = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
            let r = weft_core::parse_json(&text);
            assert_eq!(r.is_err(), depth > 772, "depth {depth}");
            let object = format!("{}1{}", "{\"a\":".repeat(depth), "}".repeat(depth));
            let _ = weft_core::parse_json(&object);
        }
    });
}

#[test]
fn deeply_nested_markup_is_refused_not_a_crash() {
    on_big_stack(|| {
        for depth in [10, 300, 2_000, 20_000] {
            let markup = format!("{}{}", "<stack>".repeat(depth), "</stack>".repeat(depth));
            parse_with_everything(&markup, Mode::Lenient);
            let unclosed = "<stack>".repeat(depth);
            parse_with_everything(&unclosed, Mode::Lenient);
        }
    });
}

#[test]
fn patches_given_as_a_huge_or_deep_json_value_are_refused_not_a_crash() {
    on_big_stack(|| {
        let catalog = catalog();
        let base = common::document(&common::screen("<stack id=\"a\"/>"));
        let deep = nested_array(5_000);
        let _ = apply_patches(&base, &deep, &ApplyOptions::new(&catalog));
        let patches = Json::Array(vec![{
            let mut m = Map::new();
            m.insert("op".into(), json!("set"));
            m.insert("id".into(), json!("a"));
            m.insert("prop".into(), json!("wrap"));
            m.insert("value".into(), deep);
            Json::Object(m)
        }]);
        let _ = apply_patches(&base, &patches, &ApplyOptions::new(&catalog));
    });
}

// ---- the writers -------------------------------------------------------------------------

/// Every document the reader produces survives a trip through the markup writer unchanged.
fn assert_markup_round_trip(markup: &str) -> Result<(), TestCaseError> {
    let catalog = catalog();
    let Some(first) = parse(markup, &lenient_options(&catalog)).document else {
        return Ok(());
    };
    let written = serialize(&first);
    let again = parse(&written, &lenient_options(&catalog));
    prop_assert!(
        again.document.is_some(),
        "{written:?}: {:?}",
        again.diagnostics
    );
    let second = again.document.unwrap();
    prop_assert_eq!(&first, &second, "{:?}", written);
    prop_assert_eq!(serialize(&second), written);
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn parse_then_serialize_then_parse_gives_the_same_document(markup in loose_markup()) {
        assert_markup_round_trip(&markup)?;
    }

    #[test]
    fn the_same_holds_for_whatever_the_reader_accepts_of_arbitrary_text(markup in markup_soup()) {
        assert_markup_round_trip(&markup)?;
        assert_markup_round_trip(&common::screen(&markup))?;
    }

    #[test]
    fn serializing_is_idempotent_through_the_reader(markup in valid_markup()) {
        let first = common::document(&markup);
        let once = serialize(&first);
        let twice = serialize(&common::document(&once));
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn canonical_json_is_stable(markup in loose_markup()) {
        let Some(doc) = parse_lenient(&markup).document else { return Ok(()); };
        let canonical = canonicalize(&doc);
        prop_assert_eq!(&canonicalize(&canonical), &canonical);
        let text = stringify(&canonical);
        prop_assert_eq!(&stringify(&doc), &text);
        prop_assert_eq!(stringify(&canonicalize(&canonical)), text.clone());
        // The text is JSON, ends with a newline and writes the same keys back in the same order.
        prop_assert!(text.ends_with('\n'));
        let value = weft_core::parse_json(&text).unwrap();
        prop_assert_eq!(serde_json::to_string_pretty(&value).unwrap() + "\n", text);
    }

    #[test]
    fn json_keys_are_ordered_the_same_whatever_their_input_order(input in any_json()) {
        let once = weft_core::order_keys(input);
        prop_assert_eq!(weft_core::order_keys(once.clone()), once.clone());
        let compact = weft_core::to_compact(&once);
        prop_assert_eq!(weft_core::to_compact(&weft_core::parse_json(&compact).unwrap()), compact);
    }
}

// ---- two spellings, one meaning ----------------------------------------------------------

proptest! {
    #![proptest_config(config())]

    #[test]
    fn valid_markup_has_no_diagnostics(markup in valid_markup()) {
        let r = parse_lenient(&markup);
        prop_assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        prop_assert!(r.document.is_some());
    }

    #[test]
    fn a_document_validates_the_same_after_a_json_round_trip(markup in loose_markup()) {
        let Some(doc) = parse_lenient(&markup).document else { return Ok(()); };
        let direct = document_codes(&doc);
        let via_json = serde_json::to_value(&doc).unwrap();
        prop_assert_eq!(validated_codes(&via_json), direct.clone());
        // Through the canonical JSON text as well, which is what a file on disk holds.
        let text = stringify(&doc);
        let reread = weft_core::parse_json(&text).unwrap();
        prop_assert_eq!(validated_codes(&reread), direct);
    }

    #[test]
    fn valid_markup_stays_valid_in_json_and_in_strict_mode(markup in valid_markup()) {
        let doc = common::document(&markup);
        let json = serde_json::to_value(&doc).unwrap();
        prop_assert!(validated_codes(&json).is_empty());
        let catalog = catalog();
        let options = ValidateOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            ..ValidateOptions::default()
        };
        prop_assert!(validate(&json, &options).is_empty());
    }

    #[test]
    fn an_empty_patch_list_changes_nothing_for_valid_documents(markup in valid_markup()) {
        let doc = common::document(&markup);
        let catalog = catalog();
        let r = apply_patches(&doc, &json!([]), &ApplyOptions::new(&catalog));
        prop_assert!(r.diagnostics.is_empty());
        prop_assert_eq!(r.document.unwrap(), doc);
    }

    #[test]
    fn removing_an_element_then_inserting_it_back_restores_the_document(markup in valid_markup()) {
        // The first generated element, if any, is e0; take it out of the screen and put it back.
        let doc = common::document(&markup);
        let Some(weft_core::Child::Node(first)) = doc.root.children.first() else { return Ok(()); };
        let fragment = {
            let single = Document { weft: "0.1".into(), context: Vec::new(), fragments: Default::default(), root: {
                let mut r = doc.root.clone();
                r.children = vec![weft_core::Child::Node(first.clone())];
                r
            }};
            serialize(&single)
        };
        let inner = fragment
            .split_once('\n').map_or("", |(_, rest)| rest)
            .rsplit_once("</screen>").map_or("", |(body, _)| body)
            .to_owned();
        let catalog = catalog();
        let id = first.id.clone().unwrap();
        let patches = json!([
            {"op": "remove", "id": id},
            {"op": "insert", "parent": "root", "index": 0, "markup": inner},
        ]);
        let r = apply_patches(&doc, &patches, &ApplyOptions::new(&catalog));
        prop_assert!(r.document.is_some(), "{:?}", r.diagnostics);
        prop_assert_eq!(stringify(&r.document.unwrap()), stringify(&doc));
    }
}
