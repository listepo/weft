//! Claims about fragment files (SPEC §10.7): a `<fragment>` is checked on its own, with its
//! parameters in scope and read by their types.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{document, markup_codes, screen};
use serde_json::json;

fn fragment(body: &str) -> Vec<&'static str> {
    markup_codes(&format!("<fragment weft=\"0.2\">{body}</fragment>"))
}

const SPEC_FRAGMENT: &str = r#"<fragment label="Page header" weft="0.2">
  <param name="title" required="true" type="string"/>
  <param default="muted" name="tone" type="enum" values="default muted"/>
  <param name="back" type="action"/>
  <param allowed-children="button link" name="actions" type="slot"/>
  <stack id="bar" direction="row">
    <button id="back" on-press="{$back}">Back</button>
    <heading id="title" level="1" text="{$title}"/>
    <text id="note" tone="{$tone}">Signed in</text>
    <outlet name="actions"/>
  </stack>
</fragment>"#;

#[test]
fn the_spec_fragment_checks_cleanly_and_reads_as_canonical_json() {
    assert_eq!(markup_codes(SPEC_FRAGMENT), Vec::<&str>::new());
    let root = serde_json::to_value(document(SPEC_FRAGMENT).root).unwrap();
    assert_eq!(
        root["children"][0],
        json!({"kind": "param", "props": {"name": "title", "required": true, "type": "string"}})
    );
    assert_eq!(root["children"][1]["props"]["default"], json!("muted"));
    let bar = &root["children"][4];
    assert_eq!(bar["children"][0]["on"], json!({"press": "{$back}"}));
    assert_eq!(
        bar["children"][1]["props"]["text"],
        json!({"bind": "$title"})
    );
    assert_eq!(
        bar["children"][3],
        json!({"kind": "outlet", "props": {"name": "actions"}})
    );
}

#[test]
fn a_fragment_body_reads_its_parameters_by_type() {
    let p = r#"<param name="n" type="number"/><param name="s" type="string"/><param name="e" type="enum" values="muted loud"/><param name="t" type="token" token-type="color"/><param name="b" type="boolean"/><param name="go" type="action"/><param name="more" type="slot"/>"#;
    let cases: &[(&str, &[&str])] = &[
        (r#"<text id="a" text="{$s}"/>"#, &[]),
        (r#"<text id="a" text="{$e}"/>"#, &[]),
        (r#"<text id="a" text="{$n}"/>"#, &["W204"]),
        (r#"<text id="a" tone="{$e}"/>"#, &["W203"]),
        (r#"<stack id="a" gap="{$t}"/>"#, &["W307"]),
        (r#"<button id="a" disabled="{!$b}">B</button>"#, &[]),
        (r#"<text id="a" text="{!$s}"/>"#, &["W218"]),
        (r#"<text id="a" text="{$s.x}"/>"#, &["W807"]),
        (r#"<text id="a" text="{$go}"/>"#, &["W807"]),
        (r#"<text id="a" text="{$more}"/>"#, &["W807"]),
        (r#"<text id="a" text="{$nope}"/>"#, &["W305"]),
        (r#"<button id="a" on-press="{$go}">B</button>"#, &[]),
        (r#"<button id="a" on-press="{$s}">B</button>"#, &["W807"]),
        (r#"<button id="a" on-press="{$gone}">B</button>"#, &["W305"]),
        (r#"<stack id="a"><outlet name="more"/></stack>"#, &[]),
        (
            r#"<stack id="a"><outlet name="more"/><outlet name="more"/></stack>"#,
            &["W804"],
        ),
        (r#"<stack id="a"><outlet name="go"/></stack>"#, &["W804"]),
        (
            r#"<stack id="a"><param name="x" type="string"/></stack>"#,
            &["W803"],
        ),
        (
            r#"<button id="a" submit="true" grow="true">B</button>"#,
            &[],
        ),
    ];
    for (body, expected) in cases {
        assert_eq!(fragment(&format!("{p}{body}")), *expected, "{body}");
    }
}

#[test]
fn declarations_are_checked() {
    let cases: &[(&str, &[&str])] = &[
        (
            r#"<param name="a" type="string"/><text id="t">T</text>"#,
            &[],
        ),
        (
            r#"<text id="t">T</text><param name="a" type="string"/>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="string"/><param name="a" type="number"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="id" type="string"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="string" values="x y"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="number" default="x"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="number" min="1" default="0"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="enum"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="string" required="true" default="x"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="slot" content="text"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (r#"<param name="a" type="string"/>"#, &["W803"]),
        (r#"<text id="t">T</text><text id="t">U</text>"#, &["W301"]),
    ];
    for (body, expected) in cases {
        assert_eq!(fragment(body), *expected, "{body}");
    }
    assert_eq!(
        markup_codes(&screen(r#"<stack id="a"><outlet name="x"/></stack>"#)),
        ["W804"]
    );
}
