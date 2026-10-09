//! The context block of SPEC §2.3: lifted out of the tree, written first, kept in canonical JSON
//! in written order, and checked entry by entry.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, codes, document, json_codes, markup_codes, parse_strict};
use serde_json::{Value as Json, json};
use weft_core::{
    Code, ParseOptions, Severity, ValidateOptions, parse, parse_json, serialize, stringify,
    validate,
};

const LOGIN: &str = r#"<screen id="login" label="Sign in" weft="0.2">
  <context>
    <entry id="why" by="human" kind="intent" name="Ivan">Returning users sign in with email &amp; password. Social sign-in is out of scope.</entry>
    <entry id="submit-disabled" by="agent" for="go" kind="decision" name="claude-opus-5-5">Disabled until an email is typed, so {$.email} is never empty.</entry>
    <entry id="reset-where" by="agent" for="reset" kind="question" name="claude-opus-5-5" status="open">Should reset open a dialog or a screen of its own?</entry>
    <entry id="copy" by="human" kind="source" name="Иван Тугай">Wording from the brand voice guide, section "Sign-in".</entry>
  </context>
  <form id="f1" state="idle" on-submit="auth.submit">
    <field id="email" label="Email" required="true" type="email" value="{$.email}"/>
    <button id="go" disabled="{!$.email}" submit="true" variant="primary">Sign in</button>
    <slot name="footer">
      <link id="reset" on-press="nav.reset">Forgot password?</link>
    </slot>
  </form>
</screen>
"#;

fn with_context(entries: &str) -> String {
    format!(
        "<screen id=\"root\" weft=\"0.2\"><context>{entries}</context><text id=\"t\">x</text></screen>"
    )
}

fn entry(attrs: &str, text: &str) -> String {
    format!("<entry id=\"n\" by=\"agent\" name=\"m\" {attrs}>{text}</entry>")
}

#[test]
fn the_login_example_round_trips_byte_for_byte() {
    let doc = document(LOGIN);
    assert_eq!(serialize(&doc), LOGIN);
    let json = stringify(&doc);
    let input = parse_json(&json).unwrap();
    assert!(json_codes(&input).is_empty());
    let back = weft_core::to_document(&input);
    assert_eq!(serialize(&back), LOGIN);
    assert_eq!(stringify(&back), json);
}

#[test]
fn canonical_json_keeps_written_order_and_member_order() {
    let json: Json = serde_json::from_str(&stringify(&document(LOGIN))).unwrap();
    let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["weft", "context", "root"]);
    let ids: Vec<&str> = json["context"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["why", "submit-disabled", "reset-where", "copy"]);
    let question: Vec<&String> = json["context"][2].as_object().unwrap().keys().collect();
    assert_eq!(
        question,
        ["id", "kind", "by", "name", "for", "status", "text"]
    );
    assert_eq!(
        json["context"][0]["text"],
        "Returning users sign in with email & password. Social sign-in is out of scope."
    );
}

#[test]
fn the_block_is_lifted_wherever_it_stands_under_the_root() {
    let late = "<screen id=\"root\" weft=\"0.2\">\n  <text id=\"t\">x</text>\n  <context>\n    <entry id=\"n\" by=\"agent\" kind=\"intent\" name=\"m\">  a\n   b </entry>\n  </context>\n</screen>\n";
    let doc = document(late);
    assert_eq!(doc.root.children.len(), 1);
    assert_eq!(
        serialize(&doc),
        "<screen id=\"root\" weft=\"0.2\">\n  <context>\n    <entry id=\"n\" by=\"agent\" kind=\"intent\" name=\"m\">a b</entry>\n  </context>\n  <text id=\"t\">x</text>\n</screen>\n"
    );
    let empty = document("<screen id=\"root\" weft=\"0.2\"><context/></screen>");
    assert!(empty.context.is_empty());
    assert_eq!(serialize(&empty), "<screen id=\"root\" weft=\"0.2\"/>\n");
}

#[test]
fn a_root_with_only_context_is_not_self_closing() {
    let markup = "<screen id=\"root\" weft=\"0.2\">\n  <context>\n    <entry id=\"n\" by=\"agent\" kind=\"intent\" name=\"m\">x</entry>\n  </context>\n</screen>\n";
    assert_eq!(serialize(&document(markup)), markup);
}

#[test]
fn misplaced_or_malformed_blocks_are_w120() {
    let cases = [
        "<screen id=\"root\" weft=\"0.2\"><form id=\"f\"><context/></form></screen>",
        "<screen id=\"root\" weft=\"0.2\"><context/><context/></screen>",
        "<screen id=\"root\" weft=\"0.2\"><context id=\"c\"/></screen>",
        "<screen id=\"root\" weft=\"0.2\"><context>note</context></screen>",
        "<screen id=\"root\" weft=\"0.2\"><context><text id=\"t\">x</text></context></screen>",
        "<context/>",
    ];
    for markup in cases {
        assert_eq!(markup_codes(markup), ["W120"], "{markup}");
    }
}

#[test]
fn misplaced_or_malformed_entries_are_w121() {
    let cases = [
        "<screen id=\"root\" weft=\"0.2\"><entry id=\"n\" by=\"agent\" kind=\"intent\" name=\"m\">x</entry></screen>".to_owned(),
        with_context(&entry("kind=\"intent\" x-acme-y=\"1\"", "x")),
        with_context(&entry("kind=\"intent\"", "a <text id=\"t\">x</text>")),
        with_context("<entry id=\"n\" kind=\"intent\" name=\"m\">x</entry>"),
    ];
    for markup in &cases {
        assert_eq!(markup_codes(markup), ["W121"], "{markup}");
    }
}

#[test]
fn entry_values_are_checked() {
    let expect = [
        (entry("kind=\"note\"", "x"), vec!["W203"]),
        (
            "<entry id=\"n\" by=\"bot\" kind=\"intent\" name=\"m\">x</entry>".to_owned(),
            vec!["W203"],
        ),
        (entry("kind=\"todo\" status=\"done\"", "x"), vec!["W203"]),
        (entry("kind=\"todo\"", "x"), vec!["W227"]),
        (entry("kind=\"intent\" status=\"open\"", "x"), vec!["W227"]),
        (entry("kind=\"intent\"", " "), vec!["W229"]),
        (
            "<entry id=\"n\" by=\"agent\" kind=\"intent\" name=\"\">x</entry>".to_owned(),
            vec!["W229"],
        ),
        (entry("kind=\"intent\" for=\"root\"", "x"), vec!["W309"]),
        (entry("kind=\"intent\" for=\"nope\"", "x"), vec!["W309"]),
        (entry("kind=\"intent\" for=\"n\"", "x"), vec!["W309"]),
        (
            entry(
                "kind=\"intent\" for=\"t\"",
                "{$.x} & {token.y}".replace('&', "&amp;").as_str(),
            ),
            vec![],
        ),
        (
            "<entry by=\"agent\" kind=\"intent\" name=\"m\">x</entry>".to_owned(),
            vec!["W202"],
        ),
        (
            "<entry id=\"t\" by=\"agent\" kind=\"intent\" name=\"m\">x</entry>".to_owned(),
            vec!["W301"],
        ),
        (
            "<entry id=\"1\" by=\"agent\" kind=\"intent\" name=\"m\">x</entry>".to_owned(),
            vec!["W212"],
        ),
    ];
    for (entries, want) in expect {
        let markup = with_context(&entries);
        assert_eq!(markup_codes(&markup), want, "{markup}");
    }
}

#[test]
fn paths_name_the_block_and_the_entry() {
    let markup = with_context(&format!(
        "{}<entry by=\"agent\" kind=\"todo\" name=\"m\">x</entry>",
        entry("kind=\"question\"", "x")
    ));
    let result = common::parse_lenient(&markup);
    let paths: Vec<(&str, &str)> = result
        .diagnostics
        .iter()
        .map(|d| (d.code.as_str(), d.path.as_str()))
        .collect();
    assert_eq!(
        paths,
        [
            ("W227", "/screen#root/context/entry#n"),
            ("W202", "/screen#root/context/entry[1]"),
            ("W227", "/screen#root/context/entry[1]"),
        ]
    );
    assert!(result.diagnostics.iter().all(|d| d.line == Some(1)));
}

#[test]
fn limits_are_a_mode_code() {
    let long = with_context(&entry("kind=\"intent\"", &"x".repeat(501)));
    let lenient = common::parse_lenient(&long);
    assert_eq!(codes(&lenient.diagnostics), ["W228"]);
    assert_eq!(lenient.diagnostics[0].severity, Severity::Warning);
    assert!(lenient.document.is_some());
    let strict = parse_strict(&long);
    assert_eq!(strict.diagnostics[0].severity, Severity::Error);

    let many: String = (0..101)
        .map(|i| format!("<entry id=\"e{i}\" by=\"agent\" kind=\"intent\" name=\"m\">x</entry>"))
        .collect();
    assert_eq!(markup_codes(&with_context(&many)), ["W228"]);
    let full: String = (0..33)
        .map(|i| {
            format!(
                "<entry id=\"e{i}\" by=\"agent\" kind=\"intent\" name=\"m\">{}</entry>",
                "x".repeat(500)
            )
        })
        .collect();
    let d = &common::parse_lenient(&with_context(&full)).diagnostics;
    assert_eq!(codes(d), ["W228"]);
    assert_eq!(d[0].path, "/screen#root/context");
}

#[test]
fn json_entries_have_a_shape_and_the_same_checks() {
    let doc = |context: Json| json!({"weft": "0.2", "context": context, "root": {"kind": "screen", "id": "s"}});
    assert_eq!(json_codes(&doc(json!({}))), ["W200"]);
    assert_eq!(
        json_codes(&doc(
            json!([{"id": "a", "kind": "intent", "by": "agent", "text": "x"}])
        )),
        ["W200"]
    );
    assert_eq!(
        json_codes(&doc(
            json!([{"id": "a", "kind": "intent", "by": "agent", "name": "m", "text": "x", "extra": 1}])
        )),
        ["W200"]
    );
    assert_eq!(
        json_codes(&doc(
            json!([{"kind": "intent", "by": "agent", "name": "m", "text": "x"}])
        )),
        ["W202"]
    );
    assert_eq!(
        json_codes(&doc(
            json!([{"id": "a", "kind": "intent", "by": "agent", "name": "m", "text": "x\u{1}"}])
        )),
        ["W221"]
    );
    assert!(json_codes(&doc(json!([]))).is_empty());
    let reserved = json!({"weft": "0.2", "root": {"kind": "screen", "id": "s", "children": [{"kind": "entry", "id": "e"}]}});
    assert_eq!(json_codes(&reserved), ["W223"]);
}

#[test]
fn insert_markup_cannot_bring_context() {
    let base = document(LOGIN);
    let catalog = catalog();
    let options = weft_core::ApplyOptions::new(&catalog);
    let patches = json!([{"op": "insert", "parent": "f1", "markup": "<context/>"}]);
    let result = weft_core::apply_patches(&base, &patches, &options);
    assert!(result.document.is_none());
    assert!(codes(&result.diagnostics).contains(&"W120"));
}

#[test]
fn without_a_catalog_the_block_is_still_read() {
    let r = parse(LOGIN, &ParseOptions::default());
    assert!(r.diagnostics.is_empty());
    assert_eq!(r.document.unwrap().context.len(), 4);
    let options = ValidateOptions::default();
    assert!(validate(&parse_json(&stringify(&document(LOGIN))).unwrap(), &options).is_empty());
    assert_eq!(Code::W228.as_str(), "W228");
}
