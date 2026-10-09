//! The context operations of SPEC §7 and the host options that guard them.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, codes, document};
use serde_json::{Value as Json, json};
use weft_core::{ApplyOptions, Author, Diagnostic, PatchResult, apply_patches, serialize};

const BASE: &str = r#"<screen id="login" label="Sign in" weft="0.2">
  <context>
    <entry id="why" by="human" kind="intent" name="Ivan">Returning users sign in.</entry>
    <entry id="submit-disabled" by="agent" for="go" kind="decision" name="m">Disabled until an email is typed.</entry>
    <entry id="reset-where" by="agent" for="reset" kind="question" name="m" status="open">Dialog or screen?</entry>
    <entry id="show-password" by="agent" kind="todo" name="m" status="open">Add a show-password toggle.</entry>
  </context>
  <form id="f1" on-submit="auth.submit">
    <field id="email" label="Email" type="email" value="{$.email}"/>
    <button id="go" disabled="{!$.email}" submit="true" variant="primary">Sign in</button>
    <slot name="footer">
      <link id="reset" on-press="nav.reset">Forgot password?</link>
    </slot>
  </form>
</screen>
"#;

fn apply_with<'a>(patches: Json, configure: impl FnOnce(&mut ApplyOptions<'a>)) -> PatchResult {
    // Leaked so the catalog outlives an author borrowed by the caller; a test process is short.
    let catalog: &'a _ = Box::leak(Box::new(catalog()));
    let mut options = ApplyOptions::new(catalog);
    configure(&mut options);
    apply_patches(&document(BASE), &patches, &options)
}

fn apply(patches: Json) -> PatchResult {
    apply_with(patches, |_| {})
}

fn markup(patches: Json) -> String {
    let result = apply(patches);
    let Some(document) = result.document else {
        panic!("{:#?}", result.diagnostics)
    };
    serialize(&document)
}

fn rejected(result: &PatchResult) -> &[Diagnostic] {
    assert!(result.document.is_none());
    &result.diagnostics
}

fn entry(id: &str) -> Json {
    json!({"id": id, "kind": "decision", "by": "agent", "name": "m", "for": "reset", "text": "Its own screen."})
}

#[test]
fn the_review_example_applies_in_order() {
    let out = markup(json!([
        {"op": "add-context", "entry": entry("reset-screen")},
        {"op": "resolve-context", "id": "reset-where"},
        {"op": "set-context", "id": "submit-disabled", "field": "text", "value": "Disabled  until\nan email & a password."},
        {"op": "remove-context", "id": "show-password"}
    ]));
    assert!(out.contains(r#"<entry id="reset-where" by="agent" for="reset" kind="question" name="m" status="resolved">"#), "{out}");
    assert!(
        out.contains(">Disabled until an email &amp; a password.</entry>"),
        "{out}"
    );
    assert!(!out.contains("show-password"), "{out}");
    let last = out.find("reset-screen").unwrap();
    assert!(
        last > out.find("submit-disabled").unwrap(),
        "added entries go last: {out}"
    );
}

#[test]
fn set_context_changes_one_field_and_keeps_status_consistent() {
    let out = markup(json!([
        {"op": "set-context", "id": "why", "field": "kind", "value": "question"},
        {"op": "set-context", "id": "show-password", "field": "kind", "value": "decision"},
        {"op": "set-context", "id": "reset-where", "field": "for", "value": null},
        {"op": "set-context", "id": "submit-disabled", "field": "for", "value": "email"}
    ]));
    assert!(
        out.contains(r#"<entry id="why" by="human" kind="question" name="Ivan" status="open">"#),
        "{out}"
    );
    assert!(
        out.contains(r#"<entry id="show-password" by="agent" kind="decision" name="m">"#),
        "{out}"
    );
    assert!(
        out.contains(
            r#"<entry id="reset-where" by="agent" kind="question" name="m" status="open">"#
        ),
        "{out}"
    );
    assert!(out.contains(r#"for="email""#), "{out}");
}

#[test]
fn values_are_left_to_validation() {
    let bad_kind =
        apply(json!([{"op": "set-context", "id": "why", "field": "kind", "value": "note"}]));
    assert_eq!(codes(rejected(&bad_kind)), ["W203"]);
    let bad_for =
        apply(json!([{"op": "set-context", "id": "why", "field": "for", "value": "nope"}]));
    assert_eq!(codes(rejected(&bad_for)), ["W309"]);
    let resolve_intent = apply(json!([{"op": "resolve-context", "id": "why"}]));
    assert_eq!(codes(rejected(&resolve_intent)), ["W227"]);
    let twice = markup(json!([
        {"op": "resolve-context", "id": "reset-where"},
        {"op": "resolve-context", "id": "reset-where"}
    ]));
    assert!(twice.contains(r#"status="resolved""#));
}

#[test]
fn shapes_are_exact() {
    let cases = [
        json!({"op": "set-context", "id": "why", "field": "text", "value": null}),
        json!({"op": "set-context", "id": "why", "field": "kind", "value": null}),
        json!({"op": "set-context", "id": "why", "field": "status", "value": "resolved"}),
        json!({"op": "set-context", "id": "why", "field": "text", "value": 7}),
        json!({"op": "add-context", "entry": {"id": "a", "kind": "intent", "by": "agent", "text": "x"}}),
        json!({"op": "add-context", "entry": entry("a"), "extra": 1}),
        json!({"op": "resolve-context"}),
        json!({"op": "remove-context", "id": 3}),
    ];
    for patch in cases {
        let result = apply(json!([patch]));
        assert_eq!(codes(rejected(&result)), ["W501"], "{patch}");
        assert!(result.diagnostics[0].path.starts_with("#/patches/0"));
    }
    let unknown = apply(json!([{"op": "resolve"}]));
    let hint = unknown.diagnostics[0].hint.as_deref().unwrap();
    assert!(hint.contains("\"resolve-context\""), "{hint}");
}

#[test]
fn an_added_id_must_be_new_and_the_hint_offers_one() {
    for taken in ["go", "why"] {
        let result = apply(json!([{"op": "add-context", "entry": entry(taken)}]));
        let d = rejected(&result);
        assert_eq!(codes(d), ["W510"]);
        assert_eq!(d[0].path, "#/patches/0/entry/id");
        assert_eq!(
            d[0].hint.as_deref(),
            Some(format!("use \"{taken}-2\"").as_str())
        );
    }
    let missing = apply(
        json!([{"op": "add-context", "entry": {"kind": "intent", "by": "agent", "name": "m", "text": "x"}}]),
    );
    assert_eq!(codes(rejected(&missing)), ["W202"]);
}

#[test]
fn context_patches_name_entries() {
    let typo = apply(json!([{"op": "remove-context", "id": "reset-were"}]));
    let d = rejected(&typo);
    assert_eq!(codes(d), ["W511"]);
    assert!(d[0].hint.as_deref().unwrap().contains("reset-where"));
    let element = apply(json!([{"op": "resolve-context", "id": "go"}]));
    let d = rejected(&element);
    assert_eq!(codes(d), ["W511"]);
    assert!(
        d[0].hint.as_deref().unwrap().contains("element"),
        "{:?}",
        d[0].hint
    );
    let removed = apply(json!([
        {"op": "remove-context", "id": "why"},
        {"op": "set-context", "id": "why", "field": "text", "value": "x"}
    ]));
    assert_eq!(rejected(&removed)[0].path, "#/patches/1/id");
}

#[test]
fn the_host_fixes_the_author() {
    let author = Author {
        by: "agent".into(),
        name: Some("m".into()),
    };
    let as_agent = |patches: Json| apply_with(patches, |o| o.author = Some(&author));
    assert!(
        as_agent(json!([{"op": "add-context", "entry": entry("n")}]))
            .document
            .is_some()
    );
    let mut human = entry("n");
    human["by"] = json!("human");
    let d = as_agent(json!([{"op": "add-context", "entry": human}]));
    assert_eq!(codes(rejected(&d)), ["W512"]);
    assert_eq!(d.diagnostics[0].path, "#/patches/0/entry/by");
    let mut other = entry("n");
    other["name"] = json!("someone");
    let d = as_agent(json!([{"op": "add-context", "entry": other}]));
    assert_eq!(d.diagnostics[0].path, "#/patches/0/entry/name");
    let any_name = Author {
        by: "agent".into(),
        name: None,
    };
    let mut named = entry("n");
    named["name"] = json!("someone");
    let ok = apply_with(json!([{"op": "add-context", "entry": named}]), |o| {
        o.author = Some(&any_name)
    });
    assert!(ok.document.is_some());
}

#[test]
fn read_only_context_refuses_context_patches_only() {
    let read_only = |patches: Json| apply_with(patches, |o| o.read_only_context = true);
    for patch in [
        json!({"op": "add-context", "entry": entry("n")}),
        json!({"op": "resolve-context", "id": "reset-where"}),
        json!({"op": "remove-context", "id": "nope"}),
    ] {
        let result = read_only(json!([patch]));
        assert_eq!(codes(rejected(&result)), ["W512"], "{patch}");
    }
    let element =
        read_only(json!([{"op": "set", "id": "go", "prop": "variant", "value": "danger"}]));
    assert!(element.document.is_some());
}

#[test]
fn removing_a_named_element_fails_and_names_the_entry() {
    let result = apply(json!([{"op": "remove", "id": "reset"}]));
    let d = rejected(&result);
    assert_eq!(codes(d), ["W309"]);
    assert_eq!(d[0].path, "/screen#login/context/entry#reset-where/@for");
    assert_eq!(
        d[0].hint.as_deref(),
        Some("remove-context reset-where, or set-context its for")
    );
    let ancestor = apply(json!([{"op": "remove", "id": "f1"}]));
    assert_eq!(codes(rejected(&ancestor)), ["W309", "W309"]);
    let repaired = markup(json!([
        {"op": "remove-context", "id": "reset-where"},
        {"op": "remove", "id": "reset"}
    ]));
    assert!(!repaired.contains("reset"), "{repaired}");
    let moved = markup(json!([{"op": "move", "id": "reset", "parent": "f1"}]));
    assert!(moved.contains(r#"for="reset""#));
}
