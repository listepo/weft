//! Claims about patches (SPEC §7): the list is untrusted JSON, applies atomically to a copy, and
//! its result is canonical and validated.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, codes, document, tokens};
use indexmap::IndexMap;
use serde_json::{Value as Json, json};
use weft_core::{
    ApplyOptions, Child, Code, Document, Mode, PatchResult, Severity, apply_patches, serialize,
    stringify,
};

const BASE: &str = "<screen id=\"root\" weft=\"0.1\">
  <stack id=\"main\">
    <text id=\"t1\">One</text>
    <text id=\"t2\">Two</text>
    <text id=\"t3\">Three</text>
  </stack>
  <form id=\"f\">
    <button id=\"go\">Go</button>
    <slot name=\"footer\">
      <link id=\"reset\">Reset</link>
    </slot>
  </form>
  <list id=\"l\">
    <item id=\"i1\">A</item>
    <each id=\"e\" as=\"x\" in=\"{$.xs}\">
      <item id=\"i2\">B</item>
    </each>
  </list>
  <dialog id=\"dlg\" label=\"D\">
    <slot name=\"actions\">
      <button id=\"ok\">OK</button>
    </slot>
  </dialog>
  <x-acme-box id=\"box\" role=\"group\">
    note
    <text id=\"bt\">x</text>
  </x-acme-box>
</screen>
";

fn base() -> Document {
    document(BASE)
}

fn apply(patches: &Json) -> PatchResult {
    apply_with(&base(), patches, |_| {})
}

fn apply_with(
    document: &Document,
    patches: &Json,
    configure: impl FnOnce(&mut ApplyOptions<'_>),
) -> PatchResult {
    let catalog = catalog();
    let mut options = ApplyOptions::new(&catalog);
    configure(&mut options);
    apply_patches(document, patches, &options)
}

fn apply_checked(
    patches: &Json,
    tokens: Option<&IndexMap<String, String>>,
    actions: Option<&[String]>,
) -> PatchResult {
    let catalog = catalog();
    let options = ApplyOptions {
        tokens,
        actions,
        ..ApplyOptions::new(&catalog)
    };
    apply_patches(&base(), patches, &options)
}

/// The patched document as markup, or the diagnostic codes when it was rejected.
fn outcome(patches: &Json) -> Result<String, Vec<&'static str>> {
    let r = apply(patches);
    match r.document {
        Some(d) => Ok(serialize(&d)),
        None => Err(codes(&r.diagnostics)),
    }
}

fn rejected(patches: &Json) -> Vec<&'static str> {
    outcome(patches).unwrap_err()
}

/// Ids in document order, read off canonical markup.
fn ids(markup: &str) -> Vec<String> {
    markup
        .split(" id=\"")
        .skip(1)
        .map(|rest| rest.split('"').next().unwrap().to_owned())
        .collect()
}

fn ids_of(patches: &Json) -> Vec<String> {
    ids(&outcome(patches).unwrap())
}

// ---- the list ----------------------------------------------------------------------------

#[test]
fn an_empty_list_is_valid_and_changes_nothing_but_the_form() {
    let r = apply(&json!([]));
    assert!(r.diagnostics.is_empty());
    assert_eq!(serialize(&r.document.unwrap()), serialize(&base()));
}

#[test]
fn the_result_is_canonical() {
    let r = apply(
        &json!([{"op": "set", "id": "main", "prop": "zz", "value": true},
                          {"op": "set", "id": "main", "prop": "aa", "value": 1}]),
    );
    let doc = r.document.unwrap();
    assert_eq!(
        doc.root.children[0]
            .as_node()
            .unwrap()
            .props
            .keys()
            .collect::<Vec<_>>(),
        ["aa", "zz"]
    );
    assert!(doc.root.source.0.is_none());
}

#[test]
fn the_given_document_is_never_changed() {
    let before = stringify(&base());
    let document = base();
    let _ = apply_with(&document, &json!([{"op": "remove", "id": "go"}]), |_| {});
    let _ = apply_with(&document, &json!([{"op": "remove", "id": "nope"}]), |_| {});
    assert_eq!(stringify(&document), before);
}

#[test]
fn a_list_that_is_not_an_array_is_w501_at_the_list() {
    for bad in [
        json!(null),
        json!({}),
        json!("x"),
        json!(1),
        json!(true),
        json!({"op": "remove", "id": "go"}),
    ] {
        let r = apply(&bad);
        assert_eq!(codes(&r.diagnostics), ["W501"], "{bad}");
        assert_eq!(r.diagnostics[0].path, "#/patches");
        assert!(r.document.is_none());
    }
}

#[test]
fn the_type_of_a_bad_list_is_named() {
    assert_eq!(
        apply(&json!(5)).diagnostics[0].got.as_deref(),
        Some("number")
    );
    assert_eq!(
        apply(&json!("x")).diagnostics[0].got.as_deref(),
        Some("string")
    );
    assert_eq!(
        apply(&json!(true)).diagnostics[0].got.as_deref(),
        Some("boolean")
    );
    assert_eq!(
        apply(&json!({})).diagnostics[0].got.as_deref(),
        Some("object")
    );
}

#[test]
fn a_malformed_patch_is_w501_and_points_into_the_list() {
    for (patch, path) in [
        (json!(null), "#/patches/0"),
        (json!("remove"), "#/patches/0"),
        (json!({"op": "explode"}), "#/patches/0"),
        (json!({}), "#/patches/0"),
        (json!({"op": "remove"}), "#/patches/0/id"),
        (json!({"op": "remove", "id": 5}), "#/patches/0/id"),
        (
            json!({"op": "remove", "id": "go", "extra": 1}),
            "#/patches/0",
        ),
        (json!({"op": "set", "id": "go"}), "#/patches/0/prop"),
        (
            json!({"op": "insert", "parent": "main", "markup": 5}),
            "#/patches/0/markup",
        ),
        (
            json!({"op": "insert", "parent": "main", "markup": "<a/>", "index": -1}),
            "#/patches/0/index",
        ),
        (
            json!({"op": "insert", "parent": "main", "markup": "<a/>", "index": 1.5}),
            "#/patches/0/index",
        ),
        (
            json!({"op": "insert", "parent": "main", "markup": "<a/>", "index": "1"}),
            "#/patches/0/index",
        ),
        (
            json!({"op": "move", "id": "go", "parent": "main", "slot": 1}),
            "#/patches/0/slot",
        ),
    ] {
        let r = apply(&json!([patch.clone()]));
        assert_eq!(r.diagnostics[0].code, Code::W501, "{patch}");
        assert!(
            r.diagnostics[0].path.starts_with(path),
            "{patch}: {}",
            r.diagnostics[0].path
        );
        assert!(r.document.is_none());
    }
}

#[test]
fn an_index_must_be_a_safe_non_negative_integer() {
    let patch = |index: Json| json!([{"op": "insert", "parent": "main", "index": index, "markup": "<text id=\"n\">x</text>"}]);
    assert!(outcome(&patch(json!(0))).is_ok());
    assert!(outcome(&patch(json!(3))).is_ok());
    assert!(outcome(&patch(json!(1.0))).is_ok());
    assert_eq!(rejected(&patch(json!(9_007_199_254_740_992_u64))), ["W501"]);
    assert_eq!(rejected(&patch(json!(1e300))), ["W501"]);
    assert_eq!(rejected(&patch(json!(-0.5))), ["W501"]);
    // Safe but past the end of the list.
    assert_eq!(rejected(&patch(json!(9_007_199_254_740_991_u64))), ["W505"]);
}

#[test]
fn the_expected_form_of_a_malformed_patch_names_the_operation() {
    let r = apply(&json!([{"op": "move", "id": "go"}]));
    assert!(
        r.diagnostics[0]
            .expected
            .as_deref()
            .unwrap()
            .contains("\"op\":\"move\"")
    );
    assert!(
        r.diagnostics[0]
            .hint
            .as_deref()
            .unwrap()
            .starts_with("write the patch as")
    );
    let r = apply(&json!([{"op": "explode"}]));
    assert_eq!(
        r.diagnostics[0].expected.as_deref(),
        Some("one of: \"set\", \"insert\", \"remove\", \"move\"")
    );
}

#[test]
fn every_malformed_patch_is_reported_and_each_at_most_three_times() {
    let r = apply(&json!([
        {"op": "remove"},
        {"op": "remove", "id": "go"},
        {"op": "insert", "parent": 1, "markup": 2, "slot": 3, "index": 4, "x": 5}
    ]));
    let paths: Vec<_> = r.diagnostics.iter().map(|d| d.path.as_str()).collect();
    assert!(paths[0].starts_with("#/patches/0"));
    assert!(paths.iter().all(|p| !p.starts_with("#/patches/1")));
    assert!(
        paths
            .iter()
            .filter(|p| p.starts_with("#/patches/2"))
            .count()
            <= 3
    );
    assert!(paths.iter().any(|p| p.starts_with("#/patches/2")));
}

#[test]
fn a_malformed_patch_stops_everything_even_when_others_are_fine() {
    let r = apply(&json!([{"op": "remove", "id": "go"}, {"op": "bogus"}]));
    assert!(r.document.is_none());
    assert_eq!(codes(&r.diagnostics), ["W501"]);
}

// ---- atomicity ---------------------------------------------------------------------------

#[test]
fn a_failing_later_patch_leaves_no_trace_of_the_earlier_ones() {
    let r = apply(&json!([
        {"op": "remove", "id": "t1"},
        {"op": "set", "id": "t2", "prop": "tone", "value": "muted"},
        {"op": "remove", "id": "nope"}
    ]));
    assert!(r.document.is_none());
    assert_eq!(codes(&r.diagnostics), ["W502"]);
    assert_eq!(r.diagnostics[0].path, "#/patches/2/id");
}

#[test]
fn the_first_failing_patch_is_the_one_reported() {
    let r = apply(&json!([
        {"op": "remove", "id": "nope1"},
        {"op": "remove", "id": "nope2"}
    ]));
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].path, "#/patches/0/id");
}

#[test]
fn a_later_patch_sees_what_an_earlier_one_did() {
    let out = outcome(&json!([
        {"op": "insert", "parent": "main", "markup": "<text id=\"new\">x</text>"},
        {"op": "set", "id": "new", "prop": "tone", "value": "muted"},
        {"op": "move", "id": "new", "parent": "f", "index": 0}
    ]))
    .unwrap();
    assert!(
        out.contains("<text id=\"new\" tone=\"muted\">x</text>"),
        "{out}"
    );
    assert!(ids(&out).iter().position(|i| i == "new") < ids(&out).iter().position(|i| i == "go"));
}

#[test]
fn a_removed_element_cannot_be_addressed_afterwards() {
    let r = apply(
        &json!([{"op": "remove", "id": "t1"}, {"op": "set", "id": "t1", "prop": "tone", "value": "muted"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W502"]);
    assert_eq!(r.diagnostics[0].path, "#/patches/1/id");
}

#[test]
fn a_result_with_errors_is_rejected_whole() {
    let r = apply(&json!([
        {"op": "remove", "id": "t1"},
        {"op": "set", "id": "t2", "prop": "tone", "value": "loud"}
    ]));
    assert!(r.document.is_none());
    assert_eq!(codes(&r.diagnostics), ["W203"]);
}

#[test]
fn a_result_with_warnings_only_is_kept_and_carries_them() {
    let r = apply(&json!([{"op": "insert", "parent": "main", "markup": "<mystery id=\"m\"/>"}]));
    assert!(r.document.is_some());
    assert_eq!(codes(&r.diagnostics), ["W401"]);
    assert_eq!(r.diagnostics[0].severity, Severity::Warning);
}

#[test]
fn strict_mode_turns_those_warnings_into_a_rejection() {
    let r = apply_with(
        &base(),
        &json!([{"op": "insert", "parent": "main", "markup": "<mystery id=\"m\"/>"}]),
        |o| o.mode = Mode::Strict,
    );
    assert!(r.document.is_none());
    assert_eq!(r.diagnostics[0].severity, Severity::Error);
}

#[test]
fn a_document_that_was_already_invalid_reports_its_errors() {
    let mut broken = base();
    broken.root.children.clear();
    broken.weft = String::new();
    let r = apply_with(&broken, &json!([]), |_| {});
    assert!(r.document.is_none());
    assert_eq!(codes(&r.diagnostics), ["W205"]);
}

#[test]
fn tokens_and_actions_are_checked_in_the_result() {
    let tokens = tokens();
    let r = apply_checked(
        &json!([{"op": "set", "id": "main", "prop": "gap", "value": {"token": "space.huge"}}]),
        Some(&tokens),
        None,
    );
    assert_eq!(codes(&r.diagnostics), ["W306"]);
    let r = apply(
        &json!([{"op": "set", "id": "main", "prop": "gap", "value": {"token": "space.huge"}}]),
    );
    assert!(r.document.is_some());
    let actions = vec!["known".to_owned()];
    let r = apply_checked(
        &json!([{"op": "set", "id": "go", "prop": "on-press", "value": "other"}]),
        None,
        Some(&actions),
    );
    assert_eq!(codes(&r.diagnostics), ["W308"]);
}

// ---- addressing --------------------------------------------------------------------------

#[test]
fn an_unknown_id_is_w502_with_the_nearest_id_as_the_hint() {
    let r = apply(&json!([{"op": "remove", "id": "rset"}]));
    assert_eq!(r.diagnostics[0].code, Code::W502);
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("did you mean \"reset\"?")
    );
    let r = apply(&json!([{"op": "remove", "id": "zzzzzzzz"}]));
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("copy an id from the document")
    );
}

#[test]
fn a_parent_that_does_not_exist_is_w502_at_the_parent() {
    let r =
        apply(&json!([{"op": "insert", "parent": "nope", "markup": "<text id=\"n\">x</text>"}]));
    assert_eq!(r.diagnostics[0].path, "#/patches/0/parent");
    let r = apply(&json!([{"op": "move", "id": "go", "parent": "nope"}]));
    assert_eq!(r.diagnostics[0].path, "#/patches/0/parent");
}

#[test]
fn the_parent_of_a_move_is_checked_before_the_moved_id() {
    let r = apply(&json!([{"op": "move", "id": "nope1", "parent": "nope2"}]));
    assert_eq!(r.diagnostics[0].path, "#/patches/0/parent");
}

#[test]
fn each_is_addressed_by_its_id() {
    let out =
        outcome(&json!([{"op": "insert", "parent": "e", "markup": "<item id=\"i9\">Z</item>"}]))
            .unwrap();
    assert_eq!(
        &ids(&out)[ids(&out).iter().position(|i| i == "e").unwrap()..][..3],
        ["e", "i2", "i9"]
    );
}

#[test]
fn a_duplicated_id_still_reaches_validation_after_a_patch() {
    let mut doc = base();
    let Child::Node(main) = &mut doc.root.children[0] else {
        panic!("main is the first child");
    };
    let Child::Node(t3) = &mut main.children[2] else {
        panic!("t3 is an element");
    };
    t3.id = Some("go".into());
    // The patch resolves to the first holder of the id; the duplicate is the result's error.
    let r = apply_with(
        &doc,
        &json!([{"op": "set", "id": "go", "prop": "tone", "value": "muted"}]),
        |_| {},
    );
    assert_eq!(codes(&r.diagnostics), ["W301"]);
    assert!(r.document.is_none());
}

// ---- set ---------------------------------------------------------------------------------

#[test]
fn set_stores_the_value_as_typed_json() {
    let out = outcome(&json!([
        {"op": "set", "id": "main", "prop": "wrap", "value": true},
        {"op": "set", "id": "main", "prop": "gap", "value": {"token": "space.md"}},
        {"op": "set", "id": "go", "prop": "disabled", "value": {"bind": "$.busy", "not": true}},
        {"op": "set", "id": "t1", "prop": "tone", "value": "muted"}
    ]))
    .unwrap();
    assert!(
        out.contains("<stack id=\"main\" gap=\"{token.space.md}\" wrap=\"true\">"),
        "{out}"
    );
    assert!(
        out.contains("<button id=\"go\" disabled=\"{!$.busy}\">"),
        "{out}"
    );
    assert!(out.contains("<text id=\"t1\" tone=\"muted\">"), "{out}");
}

#[test]
fn a_string_that_starts_with_a_brace_stays_a_literal() {
    let out =
        outcome(&json!([{"op": "set", "id": "go", "prop": "label", "value": "{$.x}"}])).unwrap();
    assert!(out.contains("label=\"{{$.x}\""), "{out}");
}

#[test]
fn a_number_set_on_a_string_prop_is_a_type_error_of_the_result() {
    assert_eq!(
        rejected(&json!([{"op": "set", "id": "go", "prop": "label", "value": 5}])),
        ["W204"]
    );
}

#[test]
fn set_null_removes_the_prop_and_removing_an_absent_prop_does_nothing() {
    let with =
        outcome(&json!([{"op": "set", "id": "t1", "prop": "tone", "value": "muted"}])).unwrap();
    let base_markup = serialize(&base());
    assert_ne!(with, base_markup);
    let doc = document(&with);
    let r = apply_with(
        &doc,
        &json!([{"op": "set", "id": "t1", "prop": "tone", "value": null}]),
        |_| {},
    );
    assert_eq!(serialize(&r.document.unwrap()), base_markup);
    assert_eq!(
        outcome(&json!([{"op": "set", "id": "t1", "prop": "tone", "value": null}])).unwrap(),
        base_markup
    );
}

#[test]
fn the_id_and_the_root_version_cannot_be_set() {
    let r = apply(&json!([{"op": "set", "id": "go", "prop": "id", "value": "x"}]));
    assert_eq!(codes(&r.diagnostics), ["W503"]);
    assert_eq!(r.diagnostics[0].path, "#/patches/0/prop");
    assert!(r.diagnostics[0].hint.as_deref().unwrap().contains("insert"));
    let r = apply(&json!([{"op": "set", "id": "root", "prop": "weft", "value": "0.2"}]));
    assert_eq!(codes(&r.diagnostics), ["W503"]);
}

#[test]
fn a_prop_called_weft_on_another_element_is_an_ordinary_prop() {
    let r = apply(&json!([{"op": "set", "id": "go", "prop": "weft", "value": "x"}]));
    assert_eq!(codes(&r.diagnostics), ["W402"]);
}

#[test]
fn a_prop_name_must_follow_the_name_grammar() {
    for bad in ["Tone", "a_b", "1a", "", "a b", "on-", "on-Press", "-a"] {
        let r = apply(&json!([{"op": "set", "id": "t1", "prop": bad, "value": "x"}]));
        assert_eq!(codes(&r.diagnostics), ["W503"], "{bad:?}");
    }
}

#[test]
fn set_binds_and_unbinds_events_by_the_on_prefix() {
    let out =
        outcome(&json!([{"op": "set", "id": "go", "prop": "on-press", "value": "auth.login"}]))
            .unwrap();
    assert!(
        out.contains("<button id=\"go\" on-press=\"auth.login\">"),
        "{out}"
    );
    let doc = document(&out);
    let r = apply_with(
        &doc,
        &json!([{"op": "set", "id": "go", "prop": "on-press", "value": null}]),
        |_| {},
    );
    assert_eq!(serialize(&r.document.unwrap()), serialize(&base()));
}

#[test]
fn an_event_takes_an_action_name_string_never_another_value() {
    for bad in [json!(5), json!(true), json!({"bind": "$.x"})] {
        let r =
            apply(&json!([{"op": "set", "id": "go", "prop": "on-press", "value": bad.clone()}]));
        assert_eq!(codes(&r.diagnostics), ["W503"], "{bad}");
    }
    // A string that is not an action name passes the patch and fails validation.
    assert_eq!(
        rejected(&json!([{"op": "set", "id": "go", "prop": "on-press", "value": "Bad Name"}])),
        ["W216"]
    );
}

#[test]
fn an_undeclared_event_is_left_to_validation() {
    assert_eq!(
        rejected(&json!([{"op": "set", "id": "go", "prop": "on-nope", "value": "x"}])),
        ["W206"]
    );
}

#[test]
fn text_set_on_an_element_that_holds_its_text_as_content_replaces_the_content() {
    let out = outcome(&json!([{"op": "set", "id": "t1", "prop": "text", "value": "Uno"}])).unwrap();
    assert!(out.contains("<text id=\"t1\">Uno</text>"), "{out}");
}

#[test]
fn text_set_to_a_reference_replaces_the_content_with_the_text_prop() {
    let out =
        outcome(&json!([{"op": "set", "id": "t1", "prop": "text", "value": {"bind": "$.one"}}]))
            .unwrap();
    assert!(out.contains("<text id=\"t1\" text=\"{$.one}\"/>"), "{out}");
}

#[test]
fn text_set_to_null_removes_the_content() {
    let out = outcome(&json!([{"op": "set", "id": "t1", "prop": "text", "value": null}])).unwrap();
    assert!(out.contains("<text id=\"t1\"/>"), "{out}");
}

#[test]
fn text_set_on_an_element_that_keeps_its_text_in_the_prop_stays_there() {
    let doc = document(&common::screen("<text id=\"t\" text=\"old\"/>"));
    let r = apply_with(
        &doc,
        &json!([{"op": "set", "id": "t", "prop": "text", "value": "new"}]),
        |_| {},
    );
    assert!(serialize(&r.document.unwrap()).contains("<text id=\"t\" text=\"new\"/>"));
}

#[test]
fn text_set_on_an_element_whose_content_holds_elements_is_a_plain_prop() {
    // Content that is not text alone is left in place, so the prop and the content then clash.
    let doc = document(&common::screen(
        "<list id=\"l\"><item id=\"i\">a<button id=\"b\">B</button></item></list>",
    ));
    let r = apply_with(
        &doc,
        &json!([{"op": "set", "id": "i", "prop": "text", "value": "x"}]),
        |_| {},
    );
    assert_eq!(codes(&r.diagnostics), ["W402", "W310"]);
}

#[test]
fn text_set_rewrites_the_content_of_a_text_element_inside_an_extension_element() {
    let out = outcome(&json!([{"op": "set", "id": "bt", "prop": "text", "value": "y"}]));
    assert!(out.unwrap().contains("<text id=\"bt\">y</text>"));
}

// ---- insert ------------------------------------------------------------------------------

#[test]
fn insert_appends_to_the_default_slot_by_default() {
    let ids =
        ids_of(&json!([{"op": "insert", "parent": "main", "markup": "<text id=\"n\">x</text>"}]));
    let main = ids.iter().position(|i| i == "main").unwrap();
    assert_eq!(&ids[main..main + 5], ["main", "t1", "t2", "t3", "n"]);
}

#[test]
fn insert_index_counts_entries_from_zero() {
    for (index, expected) in [
        (0, ["n", "t1", "t2", "t3"]),
        (1, ["t1", "n", "t2", "t3"]),
        (3, ["t1", "t2", "t3", "n"]),
    ] {
        let ids = ids_of(
            &json!([{"op": "insert", "parent": "main", "index": index, "markup": "<text id=\"n\">x</text>"}]),
        );
        let main = ids.iter().position(|i| i == "main").unwrap();
        assert_eq!(&ids[main + 1..main + 5], expected, "index {index}");
    }
}

#[test]
fn insert_index_past_the_end_is_w505_and_names_the_length() {
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "index": 4, "markup": "<text id=\"n\">x</text>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W505"]);
    let d = &r.diagnostics[0];
    assert_eq!(d.path, "#/patches/0/index");
    assert_eq!(
        d.expected.as_deref(),
        Some("an integer from 0 to 3 (text counts as an entry)")
    );
    assert_eq!(d.got.as_deref(), Some("4"));
    assert_eq!(
        d.message,
        "Index 4 is past the end of a list with 3 entries."
    );
}

#[test]
fn text_counts_as_an_entry_of_the_list() {
    // The extension element holds a text entry and an element: two entries.
    let out = outcome(&json!([{"op": "insert", "parent": "box", "index": 1, "markup": "<text id=\"n\">x</text>"}])).unwrap();
    let at = out.find("note").unwrap();
    assert!(out.find("id=\"n\"").unwrap() > at);
    assert!(out.find("id=\"n\"").unwrap() < out.find("id=\"bt\"").unwrap());
    assert_eq!(
        rejected(
            &json!([{"op": "insert", "parent": "box", "index": 3, "markup": "<text id=\"n\">x</text>"}])
        ),
        ["W505"]
    );
}

#[test]
fn insert_into_an_empty_list_accepts_only_index_zero() {
    let doc = document(&common::screen("<stack id=\"a\"/>"));
    let ok = apply_with(
        &doc,
        &json!([{"op": "insert", "parent": "a", "index": 0, "markup": "<text id=\"n\">x</text>"}]),
        |_| {},
    );
    assert!(ok.document.is_some());
    let bad = apply_with(
        &doc,
        &json!([{"op": "insert", "parent": "a", "index": 1, "markup": "<text id=\"n\">x</text>"}]),
        |_| {},
    );
    assert_eq!(codes(&bad.diagnostics), ["W505"]);
}

#[test]
fn insert_puts_several_sibling_elements_in_order() {
    let ids = ids_of(&json!([{"op": "insert", "parent": "main", "index": 1,
        "markup": "<text id=\"n1\">1</text> <text id=\"n2\">2</text><text id=\"n3\">3</text>"}]));
    let main = ids.iter().position(|i| i == "main").unwrap();
    assert_eq!(
        &ids[main + 1..main + 7],
        ["t1", "n1", "n2", "n3", "t2", "t3"]
    );
}

#[test]
fn inserted_markup_is_typed_by_the_catalog() {
    let out = outcome(&json!([{"op": "insert", "parent": "main", "markup": "<heading id=\"h\" level=\"2\">T</heading>"}])).unwrap();
    assert!(out.contains("<heading id=\"h\" level=\"2\">T</heading>"));
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "<heading id=\"h\" level=\"two\">T</heading>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W204"]);
}

#[test]
fn insert_into_a_declared_slot_goes_to_that_slot() {
    let out = outcome(
        &json!([{"op": "insert", "parent": "dlg", "slot": "actions", "index": 0,
        "markup": "<button id=\"cancel\">Cancel</button>"}]),
    )
    .unwrap();
    assert!(
        ids(&out)
            .windows(2)
            .any(|w| w == ["actions".to_owned(), "cancel".to_owned()])
            || out.find("cancel").unwrap() < out.find("id=\"ok\"").unwrap()
    );
}

#[test]
fn insert_into_a_slot_the_parent_does_not_declare_is_w504() {
    let r = apply(
        &json!([{"op": "insert", "parent": "dlg", "slot": "footr", "markup": "<button id=\"n\">x</button>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W504"]);
    let d = &r.diagnostics[0];
    assert_eq!(d.path, "#/patches/0/slot");
    assert_eq!(d.expected.as_deref(), Some("one of: \"actions\""));
    let r = apply(
        &json!([{"op": "insert", "parent": "f", "slot": "footr", "markup": "<link id=\"n\">x</link>"}]),
    );
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("did you mean \"footer\"?")
    );
}

#[test]
fn a_slot_name_that_breaks_the_name_grammar_is_w504() {
    let r = apply(
        &json!([{"op": "insert", "parent": "box", "slot": "Bad Name", "markup": "<text id=\"n\">x</text>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W504"]);
}

#[test]
fn each_declares_no_slot_and_an_extension_accepts_any_name() {
    let r = apply(
        &json!([{"op": "insert", "parent": "e", "slot": "x", "markup": "<item id=\"n\">x</item>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W504"]);
    assert_eq!(
        r.diagnostics[0].expected.as_deref(),
        Some("no slot (omit `slot` to use the default slot)")
    );
    let out = outcome(&json!([{"op": "insert", "parent": "box", "slot": "anything", "markup": "<text id=\"n\">x</text>"}])).unwrap();
    assert!(out.contains("<slot name=\"anything\">"));
}

#[test]
fn insert_markup_must_be_elements_and_nothing_else() {
    for (markup, got) in [
        ("", "no elements"),
        ("   ", "no elements"),
        ("just text", "no elements"),
        ("<!-- only a comment -->", "no elements"),
        (
            "<text id=\"n\">x</text> loose",
            "text or <slot> next to elements",
        ),
        (
            "loose <text id=\"n\">x</text>",
            "text or <slot> next to elements",
        ),
    ] {
        let r = apply(&json!([{"op": "insert", "parent": "main", "markup": markup}]));
        assert_eq!(codes(&r.diagnostics), ["W508"], "{markup:?}");
        assert_eq!(r.diagnostics[0].got.as_deref(), Some(got), "{markup:?}");
        assert_eq!(r.diagnostics[0].path, "#/patches/0/markup");
    }
}

#[test]
fn a_slot_in_inserted_markup_is_w508() {
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "<slot name=\"x\"><text id=\"n\">x</text></slot>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W508"]);
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "<text id=\"m\">x</text><slot name=\"x\"><text id=\"n\">x</text></slot>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W508"]);
}

#[test]
fn inserted_ids_may_not_clash_with_the_document_and_the_hint_offers_a_free_one() {
    let r =
        apply(&json!([{"op": "insert", "parent": "main", "markup": "<text id=\"go\">x</text>"}]));
    assert_eq!(codes(&r.diagnostics), ["W509"]);
    assert_eq!(r.diagnostics[0].hint.as_deref(), Some("use \"go-2\""));
    assert_eq!(r.diagnostics[0].got.as_deref(), Some("go"));
    // go-2 is taken too: the next free one is offered.
    let r = apply(&json!([
        {"op": "insert", "parent": "main", "markup": "<text id=\"go-2\">x</text>"},
        {"op": "insert", "parent": "main", "markup": "<text id=\"go\">x</text>"}
    ]));
    assert_eq!(r.diagnostics[0].hint.as_deref(), Some("use \"go-3\""));
}

#[test]
fn every_clashing_id_is_reported_once() {
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "<text id=\"go\">x</text><text id=\"f\">y</text><text id=\"ok\">z</text>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W509", "W509", "W509"]);
}

#[test]
fn ids_repeated_inside_the_inserted_markup_are_found_by_validation() {
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "<text id=\"n\">x</text><text id=\"n\">y</text>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W301"]);
}

#[test]
fn syntax_errors_in_inserted_markup_keep_their_codes_and_point_into_the_patch() {
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "<text id=\"n\">&nope;</text>"}]),
    );
    assert_eq!(codes(&r.diagnostics), ["W112"]);
    let d = &r.diagnostics[0];
    assert_eq!(d.path, "#/patches/0/markup/text#n");
    // Columns are relative to the markup the agent wrote, not to the wrapper around it.
    assert_eq!((d.line, d.column), (Some(1), Some(14)));
}

#[test]
fn columns_of_inserted_markup_count_utf16_units_and_later_lines_are_untouched() {
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "<text id=\"n\">\u{1F600}&nope;</text>"}]),
    );
    assert_eq!(r.diagnostics[0].column, Some(16));
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "<text id=\"n\">\n  &nope;</text>"}]),
    );
    assert_eq!(
        (r.diagnostics[0].line, r.diagnostics[0].column),
        (Some(2), Some(3))
    );
}

#[test]
fn a_bare_root_less_fragment_with_a_mismatched_closing_tag_is_a_syntax_error() {
    let r = apply(&json!([{"op": "insert", "parent": "main", "markup": "</fragment-end>"}]));
    assert!(
        codes(&r.diagnostics).iter().all(|c| c.starts_with("W1")),
        "{:?}",
        r.diagnostics
    );
}

#[test]
fn inserted_markup_cannot_close_the_wrapper_to_escape_it() {
    let r = apply(
        &json!([{"op": "insert", "parent": "main", "markup": "</x-weft-fragment><text id=\"n\">x</text>"}]),
    );
    assert!(r.document.is_none());
    assert!(!r.diagnostics.is_empty());
}

#[test]
fn insert_validates_the_content_model_of_the_result() {
    assert_eq!(
        rejected(&json!([{"op": "insert", "parent": "go", "markup": "<text id=\"n\">x</text>"}])),
        ["W304"]
    );
    assert_eq!(
        rejected(&json!([{"op": "insert", "parent": "l", "markup": "<text id=\"n\">x</text>"}])),
        ["W302"]
    );
}

// ---- remove ------------------------------------------------------------------------------

#[test]
fn remove_deletes_the_element_and_everything_inside_it() {
    let ids = ids_of(&json!([{"op": "remove", "id": "f"}]));
    for gone in ["f", "go", "reset"] {
        assert!(!ids.iter().any(|i| i == gone), "{gone}");
    }
    assert!(ids.iter().any(|i| i == "l"));
}

#[test]
fn remove_reaches_into_slots_and_each() {
    let ids = ids_of(&json!([{"op": "remove", "id": "reset"}]));
    assert!(!ids.iter().any(|i| i == "reset"));
    // The only child of `each` can be addressed too, and an empty `each` is the result's error.
    assert_eq!(rejected(&json!([{"op": "remove", "id": "i2"}])), ["W314"]);
    assert!(outcome(&json!([{"op": "remove", "id": "e"}])).is_ok());
}

#[test]
fn removing_the_last_element_of_a_slot_leaves_no_empty_slot_behind() {
    let out = outcome(&json!([{"op": "remove", "id": "reset"}])).unwrap();
    assert!(!out.contains("footer"), "{out}");
}

#[test]
fn the_root_cannot_be_removed_or_moved() {
    let r = apply(&json!([{"op": "remove", "id": "root"}]));
    assert_eq!(codes(&r.diagnostics), ["W507"]);
    assert_eq!(r.diagnostics[0].path, "#/patches/0/id");
    assert!(
        r.diagnostics[0]
            .hint
            .as_deref()
            .unwrap()
            .contains("whole markup")
    );
    let r = apply(&json!([{"op": "move", "id": "root", "parent": "main"}]));
    assert_eq!(codes(&r.diagnostics), ["W507"]);
}

#[test]
fn removing_a_required_element_fails_validation_of_the_result() {
    assert_eq!(
        rejected(
            &json!([{"op": "remove", "id": "ok"}, {"op": "remove", "id": "dlg"}, {"op": "set", "id": "l", "prop": "ordered", "value": 1}])
        ),
        ["W204"]
    );
    // The slot is required, so removing its only entry empties it and the slot disappears.
    assert_eq!(rejected(&json!([{"op": "remove", "id": "ok"}])), ["W208"]);
}

// ---- move --------------------------------------------------------------------------------

#[test]
fn move_keeps_the_id_and_the_content_of_the_element() {
    let out = outcome(&json!([{"op": "move", "id": "t1", "parent": "f", "index": 0}])).unwrap();
    assert!(out.contains("<text id=\"t1\">One</text>"));
    let ids = ids(&out);
    assert!(
        ids.iter().position(|i| i == "t1").unwrap() > ids.iter().position(|i| i == "f").unwrap()
    );
    assert!(ids.iter().filter(|i| *i == "t1").count() == 1);
}

#[test]
fn move_index_counts_the_target_list_after_the_element_has_left_it() {
    // Within one list the index is the final position of the element.
    for index in 0..=2 {
        let ids = ids_of(&json!([{"op": "move", "id": "t1", "parent": "main", "index": index}]));
        let main = ids.iter().position(|i| i == "main").unwrap();
        let mut order = vec!["t2", "t3"];
        order.insert(index, "t1");
        assert_eq!(&ids[main + 1..main + 4], order, "index {index}");
    }
}

#[test]
fn move_to_the_end_of_its_own_list_by_default() {
    let ids = ids_of(&json!([{"op": "move", "id": "t1", "parent": "main"}]));
    let main = ids.iter().position(|i| i == "main").unwrap();
    assert_eq!(&ids[main + 1..main + 4], ["t2", "t3", "t1"]);
}

#[test]
fn move_index_past_the_shortened_list_is_w505() {
    // After t1 leaves, `main` holds two entries, so 3 is past the end but 2 is not.
    assert_eq!(
        rejected(&json!([{"op": "move", "id": "t1", "parent": "main", "index": 3}])),
        ["W505"]
    );
    assert!(outcome(&json!([{"op": "move", "id": "t1", "parent": "main", "index": 2}])).is_ok());
    let r = apply(&json!([{"op": "move", "id": "t1", "parent": "main", "index": 3}]));
    assert_eq!(
        r.diagnostics[0].message,
        "Index 3 is past the end of a list with 2 entries."
    );
}

#[test]
fn a_failed_move_does_not_lose_the_element() {
    let r = apply(&json!([{"op": "move", "id": "t1", "parent": "main", "index": 9}]));
    assert!(r.document.is_none());
    // Nothing was applied, and the given document still has it.
    assert!(ids(&serialize(&base())).contains(&"t1".to_owned()));
}

#[test]
fn move_into_a_named_slot() {
    let out = outcome(
        &json!([{"op": "move", "id": "go", "parent": "dlg", "slot": "actions", "index": 0}]),
    )
    .unwrap();
    assert!(out.find("id=\"go\"").unwrap() < out.find("id=\"ok\"").unwrap());
}

#[test]
fn move_into_an_undeclared_slot_is_w504() {
    let r = apply(&json!([{"op": "move", "id": "t1", "parent": "dlg", "slot": "nope"}]));
    assert_eq!(codes(&r.diagnostics), ["W504"]);
}

#[test]
fn an_element_cannot_move_into_itself_or_its_descendants() {
    for (id, parent) in [
        ("main", "main"),
        ("main", "t1"),
        ("f", "go"),
        ("f", "reset"),
        ("dlg", "ok"),
    ] {
        let r = apply(&json!([{"op": "move", "id": id, "parent": parent}]));
        assert_eq!(codes(&r.diagnostics), ["W506"], "{id} into {parent}");
        assert_eq!(r.diagnostics[0].path, "#/patches/0/parent");
    }
}

#[test]
fn an_element_may_move_to_a_sibling_branch_or_to_the_root() {
    assert!(outcome(&json!([{"op": "move", "id": "bt", "parent": "root"}])).is_ok());
    assert!(outcome(&json!([{"op": "move", "id": "bt", "parent": "main"}])).is_ok());
}

#[test]
fn move_checks_the_result_like_any_other_patch() {
    // A text element in a list is a content error.
    assert_eq!(
        rejected(&json!([{"op": "move", "id": "t1", "parent": "l"}])),
        ["W302"]
    );
    // An item must stay under a list.
    assert_eq!(
        rejected(&json!([{"op": "move", "id": "i1", "parent": "main"}])),
        ["W303"]
    );
}

// ---- robustness --------------------------------------------------------------------------

#[test]
fn many_patches_in_one_list_apply_in_linear_time_order() {
    let patches: Vec<Json> = (0..300)
        .map(|i| json!({"op": "insert", "parent": "main", "markup": format!("<text id=\"n{i}\">x</text>")}))
        .collect();
    let out = outcome(&Json::Array(patches)).unwrap();
    assert!(out.contains("id=\"n299\""));
}

#[test]
fn patches_that_nest_deeper_and_deeper_are_refused_without_overflowing_the_stack() {
    common::on_big_stack(|| {
        let mut doc = document(&common::screen("<stack id=\"d0\"/>"));
        let mut last = "d0".to_owned();
        // Every insert is shallow and valid on its own; together they pass the depth limit.
        for round in 0..60 {
            let mut markup = String::new();
            for level in 0..20 {
                markup.push_str(&format!("<stack id=\"r{round}-{level}\">"));
            }
            for _ in 0..20 {
                markup.push_str("</stack>");
            }
            let r = apply_with(
                &doc,
                &json!([{"op": "insert", "parent": last, "markup": markup}]),
                |_| {},
            );
            match r.document {
                Some(next) => doc = next,
                None => {
                    assert_eq!(codes(&r.diagnostics), ["W200"]);
                    return;
                }
            }
            last = format!("r{round}-19");
        }
        panic!("the depth limit never applied");
    });
}
