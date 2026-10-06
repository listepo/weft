//! Claims about validation (SPEC §6, §8): every schema, semantic and compatibility code can be
//! reached, severities follow the mode, and JSON input is checked for shape before meaning.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, codes, json_codes, markup_codes, parse_lenient, screen, tokens};
use serde_json::{Value as Json, json};
use weft_core::{
    Code, JsonError, MAX_DEPTH, Mode, ParseOptions, Severity, ValidateOptions, parse, parse_json,
    validate, validate_document,
};

fn only(markup: &str) -> Vec<&'static str> {
    markup_codes(&screen(markup))
}

fn doc(root: &Json) -> Json {
    json!({"weft": "0.1", "root": root})
}

fn strict_json(input: &Json) -> Vec<(&'static str, Severity)> {
    let catalog = catalog();
    validate(
        input,
        &ValidateOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            ..ValidateOptions::default()
        },
    )
    .iter()
    .map(|d| (d.code.as_str(), d.severity))
    .collect()
}

// ---- every code is reachable --------------------------------------------------------------

#[test]
fn a_valid_screen_has_no_diagnostics() {
    assert!(only("<stack id=\"a\"><text id=\"t\">Hi</text></stack>").is_empty());
}

// ---- the 3D tilt, a universal attribute (SPEC §2.2) ---------------------------------------

#[test]
fn any_element_takes_a_tilt_and_a_perspective() {
    assert!(
        only("<stack id=\"a\" rotate-x=\"-20.5\" rotate-y=\"360\" rotate-z=\"0\" perspective=\"800\"><text id=\"t\" rotate-y=\"30\">Hi</text></stack>")
            .is_empty()
    );
}

#[test]
fn a_tilt_is_a_number_in_range_and_a_perspective_is_at_least_one() {
    assert_eq!(only("<stack id=\"a\" rotate-x=\"361\"/>"), ["W224"]);
    assert_eq!(only("<stack id=\"a\" perspective=\"0\"/>"), ["W224"]);
    assert_eq!(only("<stack id=\"a\" rotate-z=\"steep\"/>"), ["W204"]);
}

#[test]
fn a_tilt_cannot_be_bound() {
    assert_eq!(only("<stack id=\"a\" rotate-y=\"{$.angle}\"/>"), ["W217"]);
}

#[test]
fn a_model_takes_relative_paths_and_https_urls_with_the_right_extension() {
    let model = |attrs: &str| only(&format!("<model id=\"a\" label=\"L\" {attrs}/>"));
    let good = "src=\"assets/a.GLB\" usdz=\"assets/a.usdz\" fallback=\"a.png\"";
    assert!(model(good).is_empty());
    assert!(
        model("src=\"https://cdn.example/a.gltf?v=1\" fallback=\"https://cdn.example/a.webp\"")
            .is_empty()
    );
    let bad = [
        "src=\"../a.glb\" fallback=\"a.png\"",
        "src=\"a/../../b.glb\" fallback=\"a.png\"",
        "src=\"/etc/a.glb\" fallback=\"a.png\"",
        "src=\"a\\b.glb\" fallback=\"a.png\"",
        "src=\"a%2f..%2fb.glb\" fallback=\"a.png\"",
        "src=\"http://cdn.example/a.glb\" fallback=\"a.png\"",
        "src=\"javascript:alert(1)//a.glb\" fallback=\"a.png\"",
        "src=\"data:model/gltf-binary;base64,AA.glb\" fallback=\"a.png\"",
        "src=\"https://u:p@cdn.example/a.glb\" fallback=\"a.png\"",
        "src=\"https:///a.glb\" fallback=\"a.png\"",
        "src=\"a.obj\" fallback=\"a.png\"",
        "src=\"a.glb\" usdz=\"a.glb\" fallback=\"a.png\"",
        "src=\"a.glb\" fallback=\"a.svg\"",
        "src=\"a&#9;.glb\" fallback=\"a.png\"",
    ];
    for attrs in bad {
        assert_eq!(model(attrs), ["W317"], "{attrs}");
    }
    let long = format!("src=\"{}.glb\" fallback=\"a.png\"", "a".repeat(2048));
    assert_eq!(model(&long), ["W317"]);
}

#[test]
fn a_model_asset_cannot_be_bound() {
    assert_eq!(
        only("<model id=\"a\" label=\"L\" src=\"{$.m}\" fallback=\"a.png\"/>"),
        ["W217"]
    );
}

// ---- modes --------------------------------------------------------------------------------

#[test]
fn unknown_elements_and_attributes_and_newer_versions_warn_when_lenient() {
    let r = parse_lenient(
        "<screen id=\"s\" weft=\"0.9\"><mystery id=\"a\" foo=\"1\"/><stack id=\"b\" nope=\"1\"/></screen>",
    );
    let got: Vec<_> = r
        .diagnostics
        .iter()
        .map(|d| (d.code.as_str(), d.severity))
        .collect();
    assert_eq!(
        got,
        [
            ("W403", Severity::Warning),
            ("W401", Severity::Warning),
            ("W402", Severity::Warning)
        ]
    );
}

#[test]
fn the_same_markup_is_rejected_when_strict() {
    let catalog = catalog();
    let r = parse(
        "<screen id=\"s\" weft=\"0.9\"><mystery id=\"a\"/><stack id=\"b\" nope=\"1\"/></screen>",
        &ParseOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            ..ParseOptions::default()
        },
    );
    assert!(r.diagnostics.iter().all(|d| d.severity == Severity::Error));
    assert_eq!(codes(&r.diagnostics), ["W403", "W401", "W402"]);
}

#[test]
fn an_unsupported_major_version_is_an_error_in_every_mode() {
    let input = json!({"weft": "1.0", "root": {"kind": "screen", "id": "s"}});
    assert_eq!(strict_json(&input), [("W404", Severity::Error)]);
    let catalog = catalog();
    let lenient = validate(
        &input,
        &ValidateOptions {
            catalog: Some(&catalog),
            ..ValidateOptions::default()
        },
    );
    assert_eq!(lenient[0].severity, Severity::Error);
}

#[test]
fn version_checks_follow_major_dot_minor() {
    for (version, expect) in [
        ("0.1", vec![]),
        ("0.0", vec![]),
        ("0.2", vec!["W403"]),
        ("0.10", vec!["W403"]),
        ("1.0", vec!["W404"]),
        ("2.1", vec!["W404"]),
        ("0", vec!["W219"]),
        ("0.1.0", vec!["W219"]),
        ("00.1", vec!["W219"]),
        ("v0.1", vec!["W219"]),
        ("", vec!["W205"]),
    ] {
        let input = json!({"weft": version, "root": {"kind": "screen", "id": "s"}});
        assert_eq!(json_codes(&input), expect, "{version:?}");
    }
}

#[test]
fn a_huge_version_number_is_a_newer_major_not_a_panic() {
    let input =
        json!({"weft": "99999999999999999999999999.0", "root": {"kind": "screen", "id": "s"}});
    assert_eq!(json_codes(&input), ["W404"]);
}

// ---- schema details -----------------------------------------------------------------------

#[test]
fn a_wrong_enum_value_names_the_allowed_values_and_the_nearest_one() {
    let r = parse_lenient(&screen("<stack id=\"a\" align=\"cente\"/>"));
    let d = &r.diagnostics[0];
    assert_eq!(d.code, Code::W203);
    assert_eq!(d.got.as_deref(), Some("\"cente\""));
    assert_eq!(
        d.expected.as_deref(),
        Some("one of: \"start\", \"center\", \"end\", \"stretch\"")
    );
    assert_eq!(d.hint.as_deref(), Some("did you mean \"center\"?"));
}

#[test]
fn state_values_come_from_the_component() {
    assert!(only("<form id=\"f\" state=\"submitting\"/>").is_empty());
    assert_eq!(only("<form id=\"f\" state=\"loading\"/>"), ["W203"]);
}

#[test]
fn numbers_are_checked_against_min_max_and_integer() {
    assert!(only("<heading id=\"h\" level=\"1\">T</heading>").is_empty());
    assert!(only("<heading id=\"h\" level=\"6\">T</heading>").is_empty());
    for bad in ["0", "7", "2.5", "-1"] {
        assert_eq!(
            only(&format!("<heading id=\"h\" level=\"{bad}\">T</heading>")),
            ["W224"],
            "{bad}"
        );
    }
}

#[test]
fn the_range_hint_names_the_nearest_valid_number() {
    let r = parse_lenient(&screen("<heading id=\"h\" level=\"9\">T</heading>"));
    assert_eq!(r.diagnostics[0].hint.as_deref(), Some("use 6"));
    assert_eq!(
        r.diagnostics[0].expected.as_deref(),
        Some("an integer from 1 to 6")
    );
    let r = parse_lenient(&screen("<heading id=\"h\" level=\"2.5\">T</heading>"));
    assert_eq!(r.diagnostics[0].hint.as_deref(), Some("use 3"));
}

#[test]
fn a_required_prop_is_reported_once_per_element() {
    let r = parse_lenient(&screen(
        "<heading id=\"h\">T</heading><heading id=\"g\">T</heading>",
    ));
    assert_eq!(codes(&r.diagnostics), ["W205", "W205"]);
}

#[test]
fn a_component_that_requires_a_label_needs_one() {
    assert_eq!(
        only("<dialog id=\"d\"><slot name=\"actions\"><button id=\"b\">T</button></slot></dialog>"),
        ["W205"]
    );
    assert!(only("<dialog id=\"d\" label=\"L\"><slot name=\"actions\"><button id=\"b\">T</button></slot></dialog>").is_empty());
}

#[test]
fn a_missing_version_is_w205_at_the_root() {
    let r = parse_lenient("<screen id=\"s\"/>");
    assert_eq!(codes(&r.diagnostics), ["W205"]);
    assert_eq!(r.diagnostics[0].path, "/screen#s/@weft");
}

#[test]
fn the_root_must_be_a_screen_and_screen_may_only_be_the_root() {
    assert_eq!(markup_codes("<stack id=\"s\" weft=\"0.1\"/>"), ["W201"]);
    // `screen` also declares that it has no parent, so a nested one breaks that rule as well.
    assert_eq!(only("<screen id=\"x\" weft=\"0.1\"/>"), ["W303", "W312"]);
}

#[test]
fn extension_elements_need_a_vendor_prefix_a_role_and_a_wai_aria_role() {
    assert!(
        only(
            "<x-acme-box id=\"a\" role=\"group\" anything=\"1\"><stack id=\"b\"/>text</x-acme-box>"
        )
        .is_empty()
    );
    assert_eq!(only("<x-box id=\"a\" role=\"group\"/>"), ["W220"]);
    assert_eq!(only("<x-acme-box id=\"a\"/>"), ["W210"]);
    assert_eq!(only("<x-acme-box id=\"a\" role=\"Group\"/>"), ["W211"]);
    assert_eq!(only("<x-acme-box id=\"a\" role=\"{$.r}\"/>"), ["W217"]);
}

#[test]
fn extension_attributes_are_allowed_on_components_in_both_modes() {
    let markup = screen("<stack id=\"a\" x-acme-hint=\"1\"/>");
    assert!(markup_codes(&markup).is_empty());
    assert!(common::parse_strict(&markup).diagnostics.is_empty());
}

#[test]
fn unknown_elements_are_opaque_to_parent_rules_but_not_to_the_content_model() {
    // allowedChildren and allowedParents skip an unknown element ...
    assert_eq!(
        only("<select id=\"a\" label=\"L\"><x-acme-opt id=\"b\" role=\"option\"/></select>"),
        Vec::<&str>::new()
    );
    // ... but a `none` component still takes no content.
    assert_eq!(
        only("<field id=\"a\" label=\"L\"><x-acme-opt id=\"b\" role=\"option\"/></field>"),
        ["W304"]
    );
}

#[test]
fn an_unknown_element_suggests_the_nearest_component() {
    let r = parse_lenient(&screen("<stak id=\"a\"/>"));
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("did you mean \"stack\"?")
    );
    let r = parse_lenient(&screen("<zzzzzz id=\"a\"/>"));
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("use a catalog component, or an extension named x-<vendor>-zzzzzz")
    );
}

// ---- values -------------------------------------------------------------------------------

#[test]
fn bindings_and_tokens_must_fit_the_prop() {
    assert!(only("<field id=\"f\" label=\"L\" value=\"{$.v}\"/>").is_empty());
    assert!(only("<stack id=\"a\" gap=\"{token.space.sm}\"/>").is_empty());
    // A token where a string is expected is a type error; a binding may stand in for a token.
    assert_eq!(only("<text id=\"a\" text=\"{token.space.sm}\"/>"), ["W204"]);
    assert!(only("<stack id=\"a\" gap=\"{$.g}\"/>").is_empty());
}

#[test]
fn negation_applies_to_boolean_props_that_are_not_two_way() {
    assert!(only("<button id=\"b\" disabled=\"{!$.x}\">T</button>").is_empty());
    assert_eq!(only("<text id=\"a\" text=\"{!$.x}\"/>"), ["W218"]);
    assert_eq!(
        only("<field id=\"a\" label=\"L\" value=\"{!$.x}\"/>"),
        ["W218"]
    );
    assert_eq!(
        only(
            "<dialog id=\"a\" label=\"L\" open=\"{!$.x}\"><slot name=\"actions\"><button id=\"b\">T</button></slot></dialog>"
        ),
        ["W218"]
    );
}

#[test]
fn binding_paths_follow_the_binding_grammar() {
    for ok in ["$.a", "$.a.b", "$.a.0", "$.a.10", "$.a_b.C1"] {
        assert!(
            only(&format!("<text id=\"t\" text=\"{{{ok}}}\"/>")).is_empty(),
            "{ok}"
        );
    }
    for bad in ["$.", "$..a", "$.a.", "$.a.01", "$.a-b", "$.1a", "$"] {
        assert_eq!(
            only(&format!("<text id=\"t\" text=\"{{{bad}}}\"/>")),
            ["W214"],
            "{bad}"
        );
    }
}

#[test]
fn a_loop_variable_is_in_scope_only_inside_its_each() {
    assert!(only("<each id=\"e\" as=\"item\" in=\"{$.items}\"><text id=\"t\" text=\"{$item.title}\"/></each>").is_empty());
    assert_eq!(
        only(
            "<each id=\"e\" as=\"item\" in=\"{$.items}\"><stack id=\"s\"/></each><text id=\"t\" text=\"{$item.title}\"/>"
        ),
        ["W305"]
    );
}

#[test]
fn a_loop_variable_that_is_not_in_scope_suggests_the_one_that_is() {
    let r = parse_lenient(&screen(
        "<each id=\"e\" as=\"item\" in=\"{$.items}\"><text id=\"t\" text=\"{$itm.title}\"/></each>",
    ));
    assert_eq!(r.diagnostics[0].code, Code::W305);
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("did you mean \"item\"?")
    );
}

#[test]
fn nested_each_elements_see_every_enclosing_variable() {
    assert!(only("<each id=\"a\" as=\"x\" in=\"{$.xs}\"><each id=\"b\" as=\"y\" in=\"{$x.ys}\"><text id=\"t\" text=\"{$y.z}\"/></each></each>").is_empty());
    assert!(only("<each id=\"a\" as=\"x\" in=\"{$.xs}\"><each id=\"b\" as=\"y\" in=\"{$x.ys}\"><text id=\"t\" text=\"{$x.z}\"/></each></each>").is_empty());
}

#[test]
fn each_needs_a_plain_binding_a_loop_variable_name_and_an_element() {
    assert_eq!(
        only("<each id=\"e\" as=\"x\" in=\"{!$.xs}\"><stack id=\"s\"/></each>"),
        ["W222"]
    );
    assert_eq!(
        only("<each id=\"e\" as=\"x\" in=\"plain\"><stack id=\"s\"/></each>"),
        ["W222"]
    );
    assert_eq!(
        only("<each id=\"e\" as=\"X\" in=\"{$.xs}\"><stack id=\"s\"/></each>"),
        ["W222"]
    );
    assert_eq!(
        only("<each id=\"e\" in=\"{$.xs}\"><stack id=\"s\"/></each>"),
        ["W222"]
    );
    assert_eq!(
        only("<each id=\"e\" as=\"x\" in=\"{$.xs}\">text</each>"),
        ["W314", "W304"]
    );
}

#[test]
fn each_takes_only_in_and_as() {
    assert_eq!(
        only("<each id=\"e\" as=\"x\" in=\"{$.xs}\" gap=\"1\"><stack id=\"s\"/></each>"),
        ["W402"]
    );
}

#[test]
fn each_is_transparent_for_parent_and_child_rules() {
    assert!(only("<list id=\"l\"><each id=\"e\" as=\"x\" in=\"{$.xs}\"><item id=\"i\">T</item></each></list>").is_empty());
    assert_eq!(
        only(
            "<list id=\"l\"><each id=\"e\" as=\"x\" in=\"{$.xs}\"><stack id=\"s\"/></each></list>"
        ),
        ["W302"]
    );
}

#[test]
fn a_literal_may_not_mix_text_and_a_reference() {
    for value in ["Hi {$.name}", "a {!$.x}", "a {token.space.sm}"] {
        assert_eq!(
            only(&format!("<text id=\"t\" text=\"{value}\"/>")),
            ["W213"],
            "{value}"
        );
    }
    // An escaped brace at the start makes the whole value a literal.
    assert!(only("<text id=\"t\" text=\"{{$.x}\"/>").is_empty());
}

#[test]
fn token_paths_follow_the_token_grammar_and_the_token_set() {
    let catalog = catalog();
    let tokens = tokens();
    let check = |markup: &str| {
        codes(
            &parse(
                &screen(markup),
                &ParseOptions {
                    catalog: Some(&catalog),
                    tokens: Some(&tokens),
                    ..ParseOptions::default()
                },
            )
            .diagnostics,
        )
    };
    assert!(check("<stack id=\"a\" gap=\"{token.space.md}\"/>").is_empty());
    assert_eq!(check("<stack id=\"a\" gap=\"{token..md}\"/>"), ["W215"]);
    assert_eq!(check("<stack id=\"a\" gap=\"{token.}\"/>"), ["W215"]);
    assert_eq!(
        check("<stack id=\"a\" gap=\"{token.space.none}\"/>"),
        ["W306"]
    );
    assert_eq!(
        check("<stack id=\"a\" gap=\"{token.color.accent}\"/>"),
        ["W307"]
    );
}

#[test]
fn action_names_follow_the_action_grammar() {
    for ok in ["go", "auth.login", "a.b.c", "doIt2"] {
        assert!(
            only(&format!("<button id=\"b\" on-press=\"{ok}\">T</button>")).is_empty(),
            "{ok}"
        );
    }
    for bad in ["Go", "a..b", "a.", ".a", "1a", "a-b", "a b", ""] {
        assert_eq!(
            only(&format!("<button id=\"b\" on-press=\"{bad}\">T</button>")),
            ["W216"],
            "{bad:?}"
        );
    }
}

#[test]
fn an_undeclared_event_slot_or_state_suggests_the_declared_one() {
    let r = parse_lenient(&screen("<button id=\"b\" on-pres=\"go\">T</button>"));
    assert_eq!(r.diagnostics[0].code, Code::W206);
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("did you mean \"press\"?")
    );
    let r = parse_lenient(&screen(
        "<form id=\"f\"><slot name=\"foter\"><stack id=\"a\"/></slot></form>",
    ));
    assert_eq!(r.diagnostics[0].code, Code::W207);
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("did you mean \"footer\"?")
    );
}

#[test]
fn elements_without_a_declared_event_list_take_no_events() {
    assert_eq!(only("<stack id=\"a\" on-press=\"go\"/>"), ["W206"]);
    // An extension element declares nothing, so every event is fine.
    assert!(only("<x-acme-box id=\"a\" role=\"group\" on-anything=\"go\"/>").is_empty());
}

#[test]
fn a_slot_may_not_hold_text_and_obeys_its_allowed_children() {
    assert!(only("<dialog id=\"d\" label=\"L\"><slot name=\"actions\"><button id=\"b\">T</button></slot></dialog>").is_empty());
    assert_eq!(
        only(
            "<dialog id=\"d\" label=\"L\"><slot name=\"actions\"><stack id=\"b\"/></slot></dialog>"
        ),
        ["W302"]
    );
    assert_eq!(
        only(
            "<dialog id=\"d\" label=\"L\"><slot name=\"actions\">text<button id=\"b\">T</button></slot></dialog>"
        ),
        ["W304"]
    );
}

#[test]
fn a_required_slot_must_be_present() {
    assert_eq!(only("<dialog id=\"d\" label=\"L\"/>"), ["W208"]);
}

#[test]
fn text_content_models_reject_elements_and_node_models_reject_text() {
    assert_eq!(
        only("<button id=\"b\"><stack id=\"s\"/></button>"),
        ["W304"]
    );
    assert_eq!(only("<stack id=\"a\">loose text</stack>"), ["W304"]);
    assert_eq!(
        only("<field id=\"f\" label=\"L\"><stack id=\"s\"/></field>"),
        ["W304"]
    );
    // `mixed` takes both.
    assert!(
        only("<list id=\"l\"><item id=\"i\">text <button id=\"b\">T</button></item></list>")
            .is_empty()
    );
}

#[test]
fn a_submit_button_needs_a_form_among_its_ancestors() {
    assert!(only("<form id=\"f\"><stack id=\"s\"><button id=\"b\" submit=\"true\">T</button></stack></form>").is_empty());
    assert!(only("<form id=\"f\"><slot name=\"footer\"><button id=\"b\" submit=\"true\">T</button></slot></form>").is_empty());
    assert_eq!(
        only("<button id=\"b\" submit=\"true\">T</button>"),
        ["W313"]
    );
    assert!(only("<button id=\"b\" submit=\"false\">T</button>").is_empty());
}

#[test]
fn id_references_and_the_root_follow_the_catalog() {
    let messages = |catalog: &weft_core::Catalog, markup: &str| -> Vec<String> {
        let options = ParseOptions {
            catalog: Some(catalog),
            ..ParseOptions::default()
        };
        let result = parse(markup, &options);
        result.diagnostics.into_iter().map(|d| d.message).collect()
    };
    let tabs = screen("<tabs id=\"t\" selected=\"nope\"><tab id=\"a\" label=\"A\"/></tabs>");
    let stack = "<stack id=\"s\" weft=\"0.1\"/>";
    let mut catalog = catalog();
    assert_eq!(
        messages(&catalog, &tabs),
        ["\"nope\" is not the id of a <tab>."]
    );
    assert_eq!(
        messages(&catalog, stack),
        ["The root element must be <screen>."]
    );

    let selected = |catalog: &mut weft_core::Catalog, target: Option<&str>| {
        let props = catalog.components.get_mut("tabs").unwrap().props.as_mut();
        props.unwrap()["selected"].references = target.map(str::to_owned);
    };
    selected(&mut catalog, Some("button"));
    assert_eq!(
        messages(&catalog, &tabs),
        ["\"nope\" is not the id of a <button>."]
    );
    selected(&mut catalog, None);
    assert!(messages(&catalog, &tabs).is_empty());

    catalog.components.get_mut("screen").unwrap().root = None;
    assert!(messages(&catalog, stack).is_empty());
}

#[test]
fn tabs_selected_must_name_a_tab() {
    assert!(only("<tabs id=\"t\" selected=\"a\"><tab id=\"a\" label=\"A\"/></tabs>").is_empty());
    assert_eq!(
        only("<stack id=\"s\"/><tabs id=\"t\" selected=\"s\"><tab id=\"a\" label=\"A\"/></tabs>"),
        ["W309"]
    );
    // A binding is a runtime choice, not an id reference.
    assert!(
        only("<tabs id=\"t\" selected=\"{$.sel}\"><tab id=\"a\" label=\"A\"/></tabs>").is_empty()
    );
}

#[test]
fn a_tab_reference_names_the_tabs_that_exist() {
    let r = parse_lenient(&screen(
        "<tabs id=\"t\" selected=\"b\"><tab id=\"a\" label=\"A\"/></tabs>",
    ));
    assert_eq!(r.diagnostics[0].expected.as_deref(), Some("one of: \"a\""));
}

#[test]
fn text_may_come_from_content_or_from_the_text_prop_but_not_both() {
    assert!(only("<text id=\"a\" text=\"x\"/>").is_empty());
    assert!(only("<text id=\"a\">x</text>").is_empty());
    assert_eq!(only("<text id=\"a\" text=\"x\">y</text>"), ["W310"]);
}

#[test]
fn ids_are_unique_and_follow_the_id_grammar() {
    assert_eq!(only("<stack id=\"a\"><stack id=\"a\"/></stack>"), ["W301"]);
    for bad in ["1a", "a b", "-a", "a.b", ""] {
        assert_eq!(only(&format!("<stack id=\"{bad}\"/>")), ["W212"], "{bad:?}");
    }
    for ok in ["a", "A", "a-b_c9"] {
        assert!(only(&format!("<stack id=\"{ok}\"/>")).is_empty(), "{ok}");
    }
}

#[test]
fn a_duplicate_id_points_at_the_first_use() {
    let r = parse_lenient(&screen("<stack id=\"a\"/><stack id=\"a\"/>"));
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("first used at /screen#root/stack#a; pick another id")
    );
}

#[test]
fn an_element_without_an_id_is_addressed_by_its_index() {
    let r = parse_lenient(&screen("text<stack/>"));
    let d = r.diagnostics.iter().find(|d| d.code == Code::W202).unwrap();
    assert_eq!(d.path, "/screen#root/stack[1]");
}

#[test]
fn text_with_characters_xml_cannot_carry_is_w221_in_values_and_content() {
    let in_value = doc(
        &json!({"kind": "screen", "id": "s", "children": [{"kind": "text", "id": "t", "props": {"text": "a\u{1}"}}]}),
    );
    assert_eq!(json_codes(&in_value), ["W221"]);
    let in_content = doc(
        &json!({"kind": "screen", "id": "s", "children": [{"kind": "text", "id": "t", "children": ["a\u{c}"]}]}),
    );
    assert_eq!(json_codes(&in_content), ["W221"]);
    let tab_ok = doc(
        &json!({"kind": "screen", "id": "s", "children": [{"kind": "text", "id": "t", "props": {"text": "tab\there"}}]}),
    );
    assert!(json_codes(&tab_ok).is_empty());
}

// ---- JSON input ---------------------------------------------------------------------------

#[test]
fn json_that_has_the_wrong_shape_gets_w200_only_with_json_pointer_paths() {
    let bad = json!({"weft": "0.1", "root": {"kind": "screen", "id": "s", "children": [{"id": 1}], "props": {"a": [1]}}});
    let catalog = catalog();
    let d = validate(
        &bad,
        &ValidateOptions {
            catalog: Some(&catalog),
            ..ValidateOptions::default()
        },
    );
    assert!(d.iter().all(|d| d.code == Code::W200));
    assert!(d.iter().all(|d| d.path.starts_with("#/root")));
    assert!(d.iter().all(|d| d.line.is_none() && d.column.is_none()));
}

#[test]
fn shape_errors_name_what_is_wrong_in_each_part() {
    for (input, path) in [
        (json!(null), "#/"),
        (json!([]), "#/"),
        (json!({"root": {"kind": "screen"}}), "#/weft"),
        (json!({"weft": 1, "root": {"kind": "screen"}}), "#/weft"),
        (json!({"weft": "0.1"}), "#/root"),
        (json!({"weft": "0.1", "root": {"id": "s"}}), "#/root/kind"),
        (
            json!({"weft": "0.1", "root": {"kind": "screen", "extra": 1}}),
            "#/root",
        ),
        (
            json!({"weft": "0.1", "root": {"kind": "screen", "on": {"a": 1}}}),
            "#/root/on/a",
        ),
        (
            json!({"weft": "0.1", "root": {"kind": "screen", "slots": {"a": {}}}}),
            "#/root/slots/a",
        ),
        (
            json!({"weft": "0.1", "root": {"kind": "screen", "props": {"a": {"bind": 1}}}}),
            "#/root/props/a",
        ),
    ] {
        let d = validate(&input, &ValidateOptions::default());
        assert!(!d.is_empty(), "{input}");
        assert_eq!(d[0].code, Code::W200, "{input}");
        assert!(
            d[0].path.starts_with(path),
            "{input}: {} vs {path}",
            d[0].path
        );
    }
}

#[test]
fn without_a_catalog_only_the_shape_is_checked() {
    let valid_shape = json!({"weft": "banana", "root": {"kind": "whatever"}});
    assert!(validate(&valid_shape, &ValidateOptions::default()).is_empty());
    assert!(!validate(&json!(5), &ValidateOptions::default()).is_empty());
}

#[test]
fn json_deeper_than_any_document_is_one_w200_and_is_never_walked() {
    common::on_big_stack(|| {
        // Each element level costs two JSON levels, and the bound is on JSON levels.
        let mut v = json!({"kind": "stack", "id": "leaf"});
        for i in 0..MAX_DEPTH * 3 {
            v = json!({"kind": "stack", "id": format!("n{i}"), "children": [v]});
        }
        let input = doc(&json!({"kind": "screen", "id": "s", "children": [v]}));
        let d = validate(
            &input,
            &ValidateOptions {
                catalog: Some(&catalog()),
                ..ValidateOptions::default()
            },
        );
        assert_eq!(codes(&d), ["W200"]);
        assert_eq!(d[0].path, "#");
    });
}

#[test]
fn a_document_exactly_at_the_depth_limit_is_still_read() {
    common::on_big_stack(|| {
        // The deepest element of a document at the limit sits 256 elements down.
        let mut v = json!({"kind": "stack", "id": "leaf"});
        for i in 0..MAX_DEPTH - 2 {
            v = json!({"kind": "stack", "id": format!("n{i}"), "children": [v]});
        }
        let input = doc(&json!({"kind": "screen", "id": "s", "children": [v]}));
        assert!(json_codes(&input).is_empty());
    });
}

#[test]
fn validate_never_changes_what_json_parse_json_returned() {
    let input =
        parse_json(r#"{"weft":"0.1","root":{"kind":"screen","id":"s","props":{"b":"1","a":"2"}}}"#)
            .unwrap();
    let before = input.clone();
    let _ = validate(
        &input,
        &ValidateOptions {
            catalog: Some(&catalog()),
            ..ValidateOptions::default()
        },
    );
    assert_eq!(input, before);
}

#[test]
fn json_and_markup_agree_on_the_same_document() {
    let markup =
        screen("<heading id=\"h\" level=\"9\">T</heading><stack id=\"a\" direction=\"diagonal\"/>");
    let r = parse_lenient(&markup);
    let json = serde_json::to_value(r.document.as_ref().unwrap()).unwrap();
    let from_json = json_codes(&json);
    assert_eq!(codes(&r.diagnostics), from_json);
}

#[test]
fn validate_document_reports_positions_only_for_parsed_documents() {
    let markup = screen("<heading id=\"h\" level=\"9\">T</heading>");
    let parsed = parse_lenient(&markup).document.unwrap();
    let catalog = catalog();
    let options = ValidateOptions {
        catalog: Some(&catalog),
        ..ValidateOptions::default()
    };
    let with = validate_document(&parsed, &options);
    assert!(with[0].line.is_some());
    // The same content rebuilt from JSON has no markup positions.
    let rebuilt = validate(&serde_json::to_value(&parsed).unwrap(), &options);
    assert_eq!(codes(&with), codes(&rebuilt));
    assert!(rebuilt[0].line.is_none());
}

#[test]
fn validate_document_without_a_catalog_has_nothing_to_report() {
    let parsed = parse_lenient(&screen("")).document.unwrap();
    assert!(validate_document(&parsed, &ValidateOptions::default()).is_empty());
}

#[test]
fn json_errors_come_from_parse_json_not_from_validate() {
    assert!(matches!(parse_json("{"), Err(JsonError::Syntax(_))));
}

#[test]
fn a_path_uses_kind_and_id_per_element_and_slot_and_text_markers() {
    let r = parse_lenient(&screen(
        "<form id=\"f\"><slot name=\"footer\"><button id=\"b\" level=\"1\">T</button></slot></form>",
    ));
    let d = &r.diagnostics[0];
    assert_eq!(d.code, Code::W402);
    assert_eq!(d.path, "/screen#root/form#f/slot[footer]/button#b/@level");
    let r = parse_lenient(&screen("<stack id=\"a\">text</stack>"));
    assert_eq!(r.diagnostics[0].path, "/screen#root/stack#a/#text[0]");
}
