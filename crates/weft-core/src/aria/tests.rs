//! Claims about the instance tree and its readings, on the core catalog, with the outcomes
//! `@weft/render-react` (`expand.ts`) gives for the same documents.

use std::sync::OnceLock;

use serde_json::{Value as Json, json};

use super::inst::{Inst, InstChild, content_text, expand_root};
use crate::{Catalog, Document, ParseOptions, parse};

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let text = include_str!("../../../../packages/catalog/catalog.json");
        serde_json::from_str(text).unwrap()
    })
}

fn document(inner: &str) -> Document {
    let markup = format!(r#"<screen weft="0.1" id="s" label="S">{inner}</screen>"#);
    let options = ParseOptions {
        catalog: Some(catalog()),
        ..ParseOptions::default()
    };
    parse(&markup, &options).document.unwrap()
}

/// Runs `check` on the instance tree of `inner` with `data`.
fn with(inner: &str, data: Json, check: impl FnOnce(&Inst<'_>)) {
    let document = document(inner);
    let root = expand_root(&document.root, catalog(), &data).unwrap();
    check(&root);
}

fn find<'t, 'a>(n: &'t Inst<'a>, id: &str) -> Option<&'t Inst<'a>> {
    if n.id == id {
        return Some(n);
    }
    let slots = n.slots.iter().flat_map(|(_, list)| list.iter());
    n.children
        .iter()
        .chain(slots)
        .filter_map(InstChild::node)
        .find_map(|c| find(c, id))
}

fn ids(n: &Inst<'_>) -> Vec<String> {
    n.nodes().map(|c| c.id.clone()).collect()
}

#[test]
fn loops_expand_with_instance_ids_and_hidden_nodes_go() {
    let data = json!({ "rows": [{ "t": "a", "off": false }, { "t": "b", "off": true }] });
    let inner = r#"<list id="l"><each id="e" as="row" in="{$.rows}"><item id="i" text="{$row.t}" hidden="{$row.off}"/></each><item id="gone" hidden="true">x</item></list>"#;
    with(inner, data, |root| {
        let list = find(root, "l").unwrap();
        assert_eq!(ids(list), ["i[0]"]);
        let item = find(root, "i[0]").unwrap();
        assert_eq!(item.doc_id, "i");
        // The `text` prop is the content of a text-bearing kind.
        assert!(matches!(&item.children[..], [InstChild::Text(t)] if t == "a"));
    });
}

#[test]
fn a_loop_needs_a_plain_binding_to_an_array_and_a_valid_name() {
    let inner = r#"<list id="l"><each id="e" as="row" in="{!$.rows}"><item id="i">x</item></each><each id="f" as="row" in="{$.one}"><item id="j">y</item></each></list>"#;
    with(inner, json!({ "rows": [1], "one": 1 }), |root| {
        assert!(ids(find(root, "l").unwrap()).is_empty());
    });
}

#[test]
fn adjacent_text_is_one_run() {
    // An empty loop leaves the text around it side by side, as one DOM text node.
    let inner =
        r#"<stack id="t">a<each id="e" as="x" in="{$.xs}"><text id="b">b</text></each>c</stack>"#;
    with(inner, json!({ "xs": [] }), |root| {
        let stack = find(root, "t").unwrap();
        assert!(matches!(&stack.children[..], [InstChild::Text(t)] if t == "a c"));
    });
}

#[test]
fn the_empty_slot_stands_in_when_there_is_nothing_to_show() {
    let inner = r#"<list id="full"><each id="e" as="x" in="{$.xs}"><item id="i">x</item></each><slot name="empty"><text id="none">None.</text></slot></list><list id="said" state="empty"><item id="k">k</item><slot name="empty"><text id="none2">None.</text></slot></list><list id="hid"><item id="h" hidden="true">h</item><slot name="empty"><text id="none3">None.</text></slot></list>"#;
    with(inner, json!({ "xs": [] }), |root| {
        let full = find(root, "full").unwrap();
        assert!(full.shows_empty());
        assert_eq!(content_text(full.ordered()), "None.");
        assert!(find(root, "said").unwrap().shows_empty());
        // A static element counts even when it is hidden.
        let hid = find(root, "hid").unwrap();
        assert!(!hid.shows_empty());
        assert_eq!(content_text(hid.ordered()), "");
    });
    with(
        r#"<list id="full"><each id="e" as="x" in="{$.xs}"><item id="i">x</item></each><slot name="empty"><text id="none">None.</text></slot></list>"#,
        json!({ "xs": [1] }),
        |root| assert!(!find(root, "full").unwrap().shows_empty()),
    );
}

#[test]
fn bound_numbers_come_into_the_declared_range() {
    let inner = r#"<heading id="a" level="{$.big}">A</heading><heading id="b" level="{$.text}">B</heading><heading id="c" level="{$.hex}">C</heading><heading id="d" level="{$.half}">D</heading>"#;
    let data = json!({ "big": 9, "text": " 3 ", "hex": "0x4", "half": 2.5 });
    with(inner, data, |root| {
        let level = |id| find(root, id).unwrap().heading_level();
        assert_eq!(
            [level("a"), level("b"), level("c"), level("d")],
            [6, 3, 4, 3]
        );
    });
}

#[test]
fn controls_read_their_shown_values() {
    let inner = r##"<field id="n" label="N" type="number" value="{$.n}"/><field id="m" label="M" type="number" value="{$.m}" error="Bad"/><slider id="sl" label="S" min="0" max="10" step="3" value="{$.v}"/><stepper id="st" label="T" min="1" max="5" value="{$.big}"/><date-picker id="dp" label="D" value="2024-02-30"/><color-picker id="cp" label="C" value="#ABCDEF"/>"##;
    let data = json!({ "n": "1e3", "m": " 5", "v": 8, "big": 50 });
    with(inner, data, |root| {
        let n = |id| find(root, id).unwrap();
        assert_eq!(n("n").field_value(), "1e3");
        assert_eq!(n("n").field_role(), "spinbutton");
        // A number input sanitizes text that is not a valid float, a leading space included.
        assert_eq!(n("m").field_value(), "");
        assert!(n("m").field_invalid());
        assert_eq!(n("sl").slider_value(), 9.0);
        assert_eq!(n("st").stepper_value(), 5.0);
        assert_eq!(n("dp").date_value(), "");
        assert_eq!(n("cp").color_value(), "#abcdef");
    });
}

#[test]
fn choices_read_the_selected_option_tab_and_radio() {
    let inner = r#"<select id="sel" label="Pick" value="b"><option id="o1" value="a">A</option><option id="o2" value="b">B</option></select><select id="none" label="None" value="z"><option id="o3" value="a">First</option></select><tabs id="tabs" selected="t2"><tab id="t1" label="One">1</tab><tab id="t2" label="Two"><text id="x">two</text></tab></tabs><radio-group id="g" label="G" value="y"><radio id="r1" value="x">X</radio><radio id="r2" value="y">Y</radio></radio-group>"#;
    with(inner, json!({}), |root| {
        let n = |id| find(root, id).unwrap();
        let (option, text) = n("sel").selected_option().unwrap();
        assert_eq!((option.id.as_str(), text.as_str()), ("o2", "B"));
        assert_eq!(n("none").selected_option().unwrap().1, "First");
        let tabs = n("tabs").tabs();
        assert_eq!(n("tabs").selected_tab(&tabs), 1);
        let group = Some(n("g"));
        assert!(!n("r1").radio_checked(group));
        assert!(n("r2").radio_checked(group));
        assert!(!n("r2").radio_checked(None));
    });
}

#[test]
fn names_from_content_take_values_and_skip_closed_dialogs() {
    let inner = r#"<button id="b"><field id="f" label="Email" value="a@b.c" error="Taken"/> <checkbox id="c" label="Remember"/> go</button><item id="i"><dialog id="d" label="D">secret</dialog> <tabs id="t"><tab id="t1" label="One">first</tab><tab id="t2" label="Two">second</tab></tabs></item>"#;
    with(inner, json!({}), |root| {
        let b = find(root, "b").unwrap();
        // A label is a name before anything else; the value of a field is its contribution.
        assert_eq!(content_text(b.ordered()), "a@b.c Taken Remember go");
        let i = find(root, "i").unwrap();
        assert_eq!(content_text(i.ordered()), "One Two first");
    });
}

#[test]
fn a_hidden_root_has_no_instance() {
    let document = document("");
    let mut root = document.root.clone();
    root.props.insert("hidden".into(), crate::Value::Bool(true));
    assert!(expand_root(&root, catalog(), &Json::Null).is_none());
}

// ---- The tree and its snapshot; the cases of `@weft/render-react`'s `aria.test.ts`. ----

fn tree(inner: &str, data: Json) -> Json {
    let tree = super::expected_tree(&document(inner), catalog(), &data);
    serde_json::to_value(tree).unwrap()
}

/// The children of `main`.
fn main_of(inner: &str, data: Json) -> Json {
    tree(inner, data)["children"][0]["children"].clone()
}

#[test]
fn a_login_form_declares_roles_names_and_states() {
    let inner = r#"<form id="f" label="Sign in"><field id="email" type="email" label="Email" value="{$.email}"/><checkbox id="keep" label="Keep me signed in" checked="true"/><slot name="footer"><button id="go" disabled="{!$.email}">Sign in</button></slot></form>"#;
    assert_eq!(
        tree(inner, json!({ "email": "" })),
        json!({ "role": "fragment", "name": "", "children": [{ "role": "main", "name": "S", "children": [{
            "role": "form", "name": "Sign in", "children": [
                { "role": "textbox", "name": "Email" },
                { "role": "checkbox", "name": "Keep me signed in", "states": { "checked": true } },
                { "role": "button", "name": "Sign in", "states": { "disabled": true } },
            ],
        }] }] })
    );
}

#[test]
fn tables_get_a_header_row_and_tabs_their_selected_panel() {
    let inner = r#"<table id="t" label="Orders"><column id="c">Order</column><each id="e" in="{$.rows}" as="r"><row id="row" selected="{$r.sel}"><cell id="x"><text id="tx" text="{$r.id}"/></cell></row></each></table><tabs id="tabs" selected="b"><tab id="a" label="A">Alpha</tab><tab id="b" label="B">Beta</tab></tabs>"#;
    let data = json!({ "rows": [{ "id": "1", "sel": true }, { "id": "2" }] });
    assert_eq!(
        main_of(inner, data),
        json!([
            { "role": "table", "name": "Orders", "children": [
                { "role": "rowgroup", "name": "", "children": [
                    { "role": "row", "name": "Order", "children": [{ "role": "columnheader", "name": "Order" }] },
                ] },
                { "role": "rowgroup", "name": "", "children": [
                    { "role": "row", "name": "1", "states": { "selected": true }, "children": [{ "role": "cell", "name": "1" }] },
                    { "role": "row", "name": "2", "children": [{ "role": "cell", "name": "2" }] },
                ] },
            ] },
            { "role": "tablist", "name": "", "children": [
                { "role": "tab", "name": "A" },
                { "role": "tab", "name": "B", "states": { "selected": true } },
            ] },
            { "role": "tabpanel", "name": "B", "children": [{ "role": "text", "name": "Beta" }] },
        ])
    );
}

#[test]
fn unnamed_landmarks_layout_and_role_none_dissolve() {
    let inner = r#"<section id="s"><form id="f"><stack id="st"><text id="t">Hi</text></stack></form></section><x-acme-spacer id="sp" role="none">There</x-acme-spacer>"#;
    assert_eq!(
        main_of(inner, json!({})),
        json!([{ "role": "text", "name": "Hi There" }])
    );
}

#[test]
fn a_link_keeps_a_safe_url_only() {
    let inner =
        r#"<link id="a" href="https://x.test">A</link><link id="c" href="javascript:x">C</link>"#;
    assert_eq!(
        main_of(inner, json!({})),
        json!([
            { "role": "link", "name": "A", "url": "https://x.test" },
            { "role": "link", "name": "C" },
        ])
    );
}

#[test]
fn controls_show_their_role_name_and_value() {
    let inner = r##"<slider id="s" label="Volume" value="3" min="0" max="10" step="2"/><stepper id="t" label="Guests" value="2"/><date-picker id="d" label="Due" value="2026-10-05"/><color-picker id="c" label="Accent" value="#3B82F6" disabled="true"/><combobox id="b" label="Fruit" value="app"><option id="o" value="apple">Apple</option></combobox><segmented-control id="g" label="View" value="day"><segment id="a" value="day">Day</segment><segment id="w" value="week">Week</segment></segmented-control><dialog id="dl" label="Closed">x</dialog>"##;
    assert_eq!(
        main_of(inner, json!({})),
        json!([
            { "role": "slider", "name": "Volume", "children": [{ "role": "text", "name": "4" }] },
            { "role": "spinbutton", "name": "Guests", "children": [{ "role": "text", "name": "2" }] },
            { "role": "textbox", "name": "Due", "children": [{ "role": "text", "name": "2026-10-05" }] },
            { "role": "textbox", "name": "Accent", "states": { "disabled": true }, "children": [{ "role": "text", "name": "#3b82f6" }] },
            { "role": "combobox", "name": "Fruit", "children": [{ "role": "text", "name": "app" }] },
            { "role": "radiogroup", "name": "View", "children": [
                { "role": "radio", "name": "Day", "states": { "checked": true } },
                { "role": "radio", "name": "Week" },
            ] },
        ])
    );
}

fn aria(value: Json) -> super::AriaNode {
    serde_json::from_value(value).unwrap()
}

#[test]
fn normalizing_merges_text_dissolves_generic_nodes_and_drops_a_repeated_name() {
    let raw = aria(json!({ "role": "fragment", "name": "", "children": [
        { "role": "button", "name": " Go ", "children": [{ "role": "text", "name": "Go" }] },
        { "role": "generic", "name": "", "children": [
            { "role": "text", "name": "a" },
            { "role": "text", "name": " b  c" },
        ] },
        { "role": "text", "name": "d" },
    ] }));
    assert_eq!(
        serde_json::to_value(super::normalize_aria(&raw)).unwrap(),
        json!({ "role": "fragment", "name": "", "children": [
            { "role": "button", "name": "Go" },
            { "role": "text", "name": "a b c d" },
        ] })
    );
}

#[test]
fn the_snapshot_is_the_yaml_playwright_prints() {
    let tricky = aria(json!({ "role": "fragment", "name": "", "children": [
        { "role": "button", "name": "say \"hi\": now", "states": { "pressed": "mixed", "disabled": true } },
        { "role": "text", "name": "42" },
        { "role": "link", "name": "x", "url": "https://x.test/#a b", "children": [{ "role": "text", "name": "y" }] },
        { "role": "heading", "name": "", "states": { "level": 2 }, "children": [{ "role": "text", "name": "- z" }] },
        { "role": "textbox", "name": "Note", "children": [{ "role": "text", "name": "1_000.5e3" }] },
        { "role": "textbox", "name": "Plain", "children": [{ "role": "text", "name": "TRUE story" }] },
    ] }));
    assert_eq!(
        super::format_aria_snapshot(&tricky),
        [
            r#"- 'button "say \"hi\": now" [disabled] [pressed=mixed]'"#,
            r#"- text: "42""#,
            r#"- link "x":"#,
            "  - /url: https://x.test/#a b",
            "  - text: y",
            r#"- heading [level=2]: "- z""#,
            r#"- textbox "Note": "1_000.5e3""#,
            r#"- textbox "Plain": TRUE story"#,
        ]
        .join("\n")
    );
    let empty = aria(json!({ "role": "fragment", "name": "" }));
    assert_eq!(super::format_aria_snapshot(&empty), "");
}
