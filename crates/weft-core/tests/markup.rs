//! Claims about reading markup (SPEC §2–§4): the syntax layer, literal typing, slots, positions,
//! whitespace and the options of `parse`.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, document, markup_codes, parse_lenient, parse_strict, screen, tokens};
use weft_core::{
    Child, Code, MAX_DEPTH, Mode, ParseOptions, Severity, Value, has_errors, parse, stringify,
};

fn bare(markup: &str) -> weft_core::ParseResult {
    parse(markup, &ParseOptions::default())
}

fn bare_codes(markup: &str) -> Vec<&'static str> {
    common::codes(&bare(markup).diagnostics)
}

fn first(markup: &str) -> (Option<u32>, Option<u32>) {
    let r = bare(markup);
    let d = &r.diagnostics[0];
    (d.line, d.column)
}

// ---- syntax layer -------------------------------------------------------------------------

#[test]
fn every_syntax_code_can_be_reached_from_markup() {
    let cases: [(&str, &str); 19] = [
        ("W101", "<a><</a>"),
        ("W102", "<?xml version=\"1.0\"?><a/>"),
        ("W103", "<!DOCTYPE a><a/>"),
        ("W104", "<a><![CDATA[x]]></a>"),
        ("W105", "<A/>"),
        ("W106", "<a b=c/>"),
        ("W107", "<a b/>"),
        ("W108", "<a b=\"1\" b=\"2\"/>"),
        ("W109", "<a><b></c></a>"),
        ("W110", "<a><b></b>"),
        ("W111", "<a/></b>"),
        ("W112", "<a>&nope;</a>"),
        ("W113", "<a b=\"<\"/>"),
        ("W114", "<a/>text"),
        ("W115", "<a><!-- x -- y --></a>"),
        ("W116", "<a b=\"{oops}\"/>"),
        ("W117", &"<a>".repeat(MAX_DEPTH + 1)),
        ("W118", "<a><slot/></a>"),
        ("W119", "<a><slot name=\"s\"/><slot name=\"s\"/></a>"),
    ];
    for (code, markup) in cases {
        assert!(
            bare_codes(markup).contains(&code),
            "{code} is not reported for {markup:?}: {:?}",
            bare_codes(markup)
        );
    }
}

#[test]
fn a_syntax_error_leaves_no_document() {
    for markup in ["<a>", "<a b=c/>", "<a>&bad;</a>", "<a/><b/>", ""] {
        let r = bare(markup);
        assert!(r.document.is_none(), "{markup:?}");
        assert!(has_errors(&r.diagnostics), "{markup:?}");
    }
}

#[test]
fn valid_markup_without_a_catalog_has_no_diagnostics_and_every_literal_is_a_string() {
    let r = bare("<anything id=\"a\" level=\"2\" on=\"true\"/>");
    assert!(r.diagnostics.is_empty());
    let json = stringify(&r.document.unwrap());
    assert!(json.contains("\"level\": \"2\""), "{json}");
    assert!(json.contains("\"on\": \"true\""), "{json}");
}

#[test]
fn the_empty_document_reports_a_missing_root_at_its_end() {
    let r = bare("");
    assert_eq!(common::codes(&r.diagnostics), ["W114"]);
    assert_eq!(
        (r.diagnostics[0].line, r.diagnostics[0].column),
        (Some(1), Some(1))
    );
    assert_eq!(r.diagnostics[0].path, "/");
}

#[test]
fn comments_and_whitespace_around_the_root_are_allowed() {
    assert!(
        bare("<!-- a -->\n  <a/>\n<!-- b -->\n")
            .diagnostics
            .is_empty()
    );
}

#[test]
fn text_or_a_second_element_outside_the_root_is_reported() {
    assert_eq!(bare_codes("hi <a/>"), ["W114"]);
    assert_eq!(bare_codes("<a/> hi"), ["W114"]);
    assert_eq!(bare_codes("<a/><b/>"), ["W114"]);
}

#[test]
fn an_unterminated_comment_and_a_double_dash_are_both_w115() {
    assert_eq!(bare_codes("<a><!-- never closed"), ["W110", "W115"]);
    assert_eq!(bare_codes("<a><!-- a--b --></a>"), ["W115"]);
    assert_eq!(bare_codes("<a><!-- a---></a>"), ["W115"]);
    assert_eq!(bare_codes("<a><!-- a- --></a>"), Vec::<&str>::new());
    assert_eq!(bare_codes("<a><!----></a>"), Vec::<&str>::new());
}

#[test]
fn only_the_five_predefined_entities_and_valid_numeric_references_are_read() {
    let r = bare("<a t=\"&lt;&gt;&amp;&quot;&apos;&#65;&#x1F600;\"/>");
    assert!(r.diagnostics.is_empty());
    assert!(stringify(&r.document.unwrap()).contains("\"t\": \"<>&\\\"'A😀\""));
    for bad in [
        "<a>&nbsp;</a>",
        "<a>&#0;</a>",
        "<a>&#xD800;</a>",
        "<a>&#x110000;</a>",
        "<a>&#99999999999999999999;</a>",
        "<a>&;</a>",
        "<a>&#;</a>",
        "<a>&#x;</a>",
        "<a>a & b</a>",
        "<a>&amp</a>",
    ] {
        assert_eq!(bare_codes(bad), ["W112"], "{bad}");
    }
}

#[test]
fn characters_xml_cannot_carry_are_reported_at_their_position() {
    let r = bare("<a>\u{1}</a>");
    assert_eq!(common::codes(&r.diagnostics), ["W113"]);
    assert_eq!(r.diagnostics[0].column, Some(4));
    assert_eq!(bare_codes("<a>\u{FFFE}</a>"), ["W113"]);
    assert_eq!(bare_codes("<a>]]></a>"), ["W113"]);
    assert_eq!(bare_codes("<a b=\"x<y\"/>"), ["W113"]);
}

#[test]
fn quoting_mistakes_have_their_own_codes() {
    assert_eq!(bare_codes("<a b='1'/>"), ["W106"]);
    assert_eq!(bare_codes("<a b=1/>"), ["W106"]);
    assert_eq!(bare_codes("<a b/>"), ["W107"]);
    // The unclosed value swallows the rest of the input, so the element is never closed either.
    assert_eq!(bare_codes("<a b=\"1/>"), ["W110", "W110"]);
}

#[test]
fn names_that_break_the_grammar_get_a_kebab_case_suggestion() {
    let r = bare("<a myAttr=\"1\"/>");
    let d = &r.diagnostics[0];
    assert_eq!(d.code, Code::W105);
    assert_eq!(d.hint.as_deref(), Some("did you mean \"my-attr\"?"));
    assert_eq!(d.path, "/a/@myAttr");
}

#[test]
fn a_mismatched_closing_tag_recovers_to_the_matching_open_element() {
    // `</a>` closes <c> and <b> too, so the second `</a>` has nothing left to close.
    assert_eq!(bare_codes("<a><b><c></a></a>"), ["W109", "W111"]);
    assert!(bare_codes("<a><b></a>").contains(&"W109"));
    assert!(bare_codes("<a></b></a>").contains(&"W109"));
}

#[test]
fn nesting_is_limited_to_the_maximum_depth() {
    let at_limit = format!("{}{}", "<a>".repeat(MAX_DEPTH), "</a>".repeat(MAX_DEPTH));
    assert!(bare(&at_limit).diagnostics.is_empty());
    let over = format!(
        "{}{}",
        "<a>".repeat(MAX_DEPTH + 1),
        "</a>".repeat(MAX_DEPTH + 1)
    );
    let r = bare(&over);
    assert_eq!(common::codes(&r.diagnostics), ["W117"]);
    assert!(r.document.is_none());
}

#[test]
fn hostile_nesting_is_cut_off_without_overflowing_the_stack() {
    let r = bare(&"<a>".repeat(200_000));
    assert_eq!(common::codes(&r.diagnostics), ["W117"]);
}

#[test]
fn a_bom_is_ignored_and_line_ends_are_normalized() {
    let r = bare("\u{FEFF}<a>\r\n  x\r  y\n</a>");
    assert!(r.diagnostics.is_empty());
    assert!(stringify(&r.document.unwrap()).contains("\"x y\""));
    // The BOM does not shift the column of the first token.
    assert_eq!(first("\u{FEFF}<A/>"), (Some(1), Some(2)));
}

// ---- positions ----------------------------------------------------------------------------

#[test]
fn positions_are_one_based_and_count_lines() {
    assert_eq!(first("<a>\n  <B/>\n</a>"), (Some(2), Some(4)));
    assert_eq!(first("<A/>"), (Some(1), Some(2)));
}

#[test]
fn columns_count_utf16_code_units_not_code_points_or_bytes() {
    // U+1F600 is two UTF-16 code units, four UTF-8 bytes and one code point.
    assert_eq!(first("<a>\u{1F600}&bad;</a>"), (Some(1), Some(6)));
    // U+00E9 is one unit and two bytes; U+20AC is one unit and three bytes.
    assert_eq!(first("<a>\u{e9}\u{20ac}&bad;</a>"), (Some(1), Some(6)));
}

#[test]
fn columns_restart_after_a_line_that_holds_astral_characters() {
    assert_eq!(
        first("<a>\n\u{1F600}\u{1F600}&bad;</a>"),
        (Some(2), Some(5))
    );
}

#[test]
fn a_carriage_return_alone_counts_as_a_line_end() {
    assert_eq!(first("<a>\r<B/></a>"), (Some(2), Some(2)));
    assert_eq!(first("<a>\r\n<B/></a>"), (Some(2), Some(2)));
}

#[test]
fn syntax_diagnostics_name_the_open_elements_only() {
    let r = bare("<a id=\"x\"><b><C/></b></a>");
    assert_eq!(r.diagnostics[0].path, "/a#x/b/C");
}

#[test]
fn diagnostics_are_sorted_by_position() {
    let r = bare("<a x='1'>\n&bad;<b y/></a>");
    let lines: Vec<_> = r
        .diagnostics
        .iter()
        .map(|d| (d.line.unwrap(), d.column.unwrap()))
        .collect();
    let mut sorted = lines.clone();
    sorted.sort_unstable();
    assert_eq!(lines, sorted);
}

#[test]
fn schema_diagnostics_from_markup_carry_the_position_of_the_attribute() {
    let r = parse_lenient(&screen("<stack id=\"a\" direction=\"diagonal\"/>"));
    let d = r.diagnostics.iter().find(|d| d.code == Code::W203).unwrap();
    assert_eq!(d.path, "/screen#root/stack#a/@direction");
    assert_eq!((d.line, d.column), (Some(1), Some(44)));
}

// ---- whitespace and text ------------------------------------------------------------------

#[test]
fn text_is_trimmed_and_inner_whitespace_collapses_to_one_space() {
    let doc = document(&screen("<text id=\"t\">  a \t\n  b  </text>"));
    let Child::Node(text) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(text.children, [Child::Text("a b".into())]);
}

#[test]
fn only_xml_whitespace_counts_so_a_no_break_space_is_kept() {
    let doc = document(&screen("<text id=\"t\">a\u{a0}\u{a0}b</text>"));
    let Child::Node(text) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(text.children, [Child::Text("a\u{a0}\u{a0}b".into())]);
}

#[test]
fn whitespace_between_elements_is_insignificant() {
    let doc = document(&screen("\n  <stack id=\"a\"/>\n  <stack id=\"b\"/>\n"));
    assert_eq!(doc.root.children.len(), 2);
}

#[test]
fn text_on_both_sides_of_a_comment_is_one_run() {
    let doc = document(&screen("<text id=\"t\">a <!-- c --> b</text>"));
    let Child::Node(text) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(text.children, [Child::Text("a b".into())]);
}

#[test]
fn text_runs_that_end_up_adjacent_join_with_one_space() {
    let r = parse_lenient(&screen("<item id=\"i\">a<slot name=\"x\"/>b</item>"));
    // `item` takes no slots; the claim is only about joining, so look at the document.
    let Child::Node(item) = &r.document.unwrap().root.children[0].clone() else {
        panic!()
    };
    assert_eq!(item.children, [Child::Text("a b".into())]);
}

#[test]
fn literal_tabs_and_line_breaks_in_attribute_values_read_as_spaces() {
    let doc = document(&screen(
        "<text id=\"t\" tone=\"default\" text=\"a\tb\nc\"/>",
    ));
    let Child::Node(text) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(text.props["text"], Value::String("a b c".into()));
}

#[test]
fn character_references_keep_tabs_and_line_breaks_in_attribute_values() {
    let doc = document(&screen("<text id=\"t\" text=\"a&#9;b&#10;c\"/>"));
    let Child::Node(text) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(text.props["text"], Value::String("a\tb\nc".into()));
}

// ---- literal typing and values ------------------------------------------------------------

#[test]
fn literals_are_typed_by_the_catalog() {
    let doc = document(&screen(
        "<heading id=\"h\" level=\"2\">T</heading><stack id=\"k\" wrap=\"true\" gap=\"{token.space.sm}\"/>",
    ));
    let Child::Node(h) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(h.props["level"], Value::Number(2.0));
    let Child::Node(k) = &doc.root.children[1] else {
        panic!()
    };
    assert_eq!(k.props["wrap"], Value::Bool(true));
    assert_eq!(k.props["gap"], Value::Token("space.sm".into()));
}

#[test]
fn a_literal_that_does_not_fit_its_type_stays_a_string_and_is_reported() {
    let r = parse_lenient(&screen("<heading id=\"h\" level=\"two\">T</heading>"));
    assert_eq!(common::codes(&r.diagnostics), ["W204"]);
    let doc = r.document.unwrap();
    let Child::Node(h) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(h.props["level"], Value::String("two".into()));
}

#[test]
fn json_number_forms_are_read_as_numbers_and_other_forms_are_not() {
    for (raw, ok) in [
        ("1", true),
        ("-0", true),
        ("1.5e2", true),
        ("01", false),
        ("1.", false),
        (".5", false),
        ("+1", false),
        ("0x10", false),
        ("1e400", false),
    ] {
        let r = parse_lenient(&screen(&format!(
            "<heading id=\"h\" level=\"{raw}\">T</heading>"
        )));
        let doc = r.document.unwrap();
        let Child::Node(h) = &doc.root.children[0] else {
            panic!()
        };
        assert_eq!(matches!(h.props["level"], Value::Number(_)), ok, "{raw}");
    }
}

#[test]
fn negative_zero_is_written_as_zero() {
    let r = parse_lenient(&screen("<heading id=\"h\" level=\"-0\">T</heading>"));
    let json = stringify(&r.document.unwrap());
    assert!(json.contains("\"level\": 0\n"), "{json}");
}

#[test]
fn a_value_starting_with_a_double_brace_is_a_literal_without_the_first_brace() {
    let doc = document(&screen("<text id=\"t\" text=\"{{$.not-a-binding}\"/>"));
    let Child::Node(t) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(t.props["text"], Value::String("{$.not-a-binding}".into()));
}

#[test]
fn references_are_read_as_bindings_negated_bindings_and_tokens() {
    let doc = document(&screen(
        "<button id=\"b\" disabled=\"{!$.busy}\">Go</button><field id=\"f\" label=\"L\" value=\"{$.v}\"/>",
    ));
    let Child::Node(b) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(
        b.props["disabled"],
        Value::Bind {
            bind: "$.busy".into(),
            not: true
        }
    );
    let Child::Node(f) = &doc.root.children[1] else {
        panic!()
    };
    assert_eq!(
        f.props["value"],
        Value::Bind {
            bind: "$.v".into(),
            not: false
        }
    );
}

#[test]
fn a_brace_that_does_not_start_a_whole_reference_is_w116() {
    for value in ["{oops}", "{", "{$.a} tail", "{!x}", "{token}", "{$.a"] {
        let markup = format!("<a b=\"{value}\"/>");
        assert_eq!(bare_codes(&markup), ["W116"], "{value}");
    }
}

#[test]
fn id_and_event_attributes_are_plain_text_never_references() {
    let r = bare("<button id=\"{$.x}\" on-press=\"{x}\"/>");
    assert!(r.diagnostics.is_empty());
    let root = r.document.unwrap().root;
    assert_eq!(root.id.as_deref(), Some("{$.x}"));
    assert_eq!(root.on["press"], "{x}");
}

#[test]
fn the_version_attribute_of_the_root_becomes_the_document_version_and_not_a_prop() {
    let doc = document(&screen(""));
    assert_eq!(doc.weft, "0.1");
    assert!(doc.root.props.is_empty());
}

// ---- slots --------------------------------------------------------------------------------

#[test]
fn named_slots_are_lifted_out_of_the_children() {
    let doc = document(&screen(
        "<form id=\"f\"><stack id=\"a\"/><slot name=\"footer\"><stack id=\"b\"/></slot></form>",
    ));
    let Child::Node(f) = &doc.root.children[0] else {
        panic!()
    };
    assert_eq!(f.children.len(), 1);
    assert_eq!(f.slots["footer"].len(), 1);
}

#[test]
fn every_misplaced_slot_is_w118() {
    assert_eq!(bare_codes("<slot name=\"a\"/>"), ["W118"]);
    assert_eq!(bare_codes("<a><slot/></a>"), ["W118"]);
    assert_eq!(bare_codes("<a><slot name=\"A\"/></a>"), ["W118"]);
    assert_eq!(bare_codes("<a><slot name=\"a\" x=\"1\"/></a>"), ["W118"]);
    assert_eq!(
        bare_codes("<a><slot name=\"a\"><slot name=\"b\"/></slot></a>"),
        ["W118"]
    );
    assert_eq!(bare_codes("<each><slot name=\"a\"/></each>"), ["W118"]);
}

#[test]
fn a_repeated_slot_name_is_w119_and_the_first_slot_is_kept() {
    let r = bare("<a><slot name=\"s\"><x/></slot><slot name=\"s\"><y/></slot></a>");
    assert_eq!(common::codes(&r.diagnostics), ["W119"]);
    assert!(r.document.is_none());
}

#[test]
fn an_empty_slot_leaves_no_trace_in_the_model() {
    let doc = document(&screen("<form id=\"f\"><slot name=\"footer\"/></form>"));
    let Child::Node(f) = &doc.root.children[0] else {
        panic!()
    };
    assert!(f.slots.is_empty());
}

// ---- options ------------------------------------------------------------------------------

#[test]
fn unknown_content_is_a_warning_when_lenient_and_an_error_when_strict() {
    let markup = screen("<mystery id=\"m\"/>");
    let lenient = parse_lenient(&markup);
    assert_eq!(common::codes(&lenient.diagnostics), ["W401"]);
    assert_eq!(lenient.diagnostics[0].severity, Severity::Warning);
    assert!(lenient.document.is_some());
    let strict = parse_strict(&markup);
    assert_eq!(common::codes(&strict.diagnostics), ["W401"]);
    assert_eq!(strict.diagnostics[0].severity, Severity::Error);
}

#[test]
fn the_default_mode_is_lenient() {
    assert_eq!(Mode::default(), Mode::Lenient);
}

#[test]
fn the_document_is_kept_when_validation_reports_errors_but_not_when_building_does() {
    let r = parse_lenient(&screen("<heading id=\"h\">T</heading>"));
    assert_eq!(common::codes(&r.diagnostics), ["W205"]);
    assert!(r.document.is_some());
    let r = parse_lenient(&screen("<heading id=\"h\" level=\"{oops}\">T</heading>"));
    assert_eq!(common::codes(&r.diagnostics), ["W116"]);
    assert!(r.document.is_none());
}

#[test]
fn token_references_are_checked_only_when_a_token_set_is_given() {
    let markup = screen("<stack id=\"k\" gap=\"{token.space.mmd}\"/>");
    assert!(markup_codes(&markup).is_empty());
    let catalog = catalog();
    let tokens = tokens();
    let r = parse(
        &markup,
        &ParseOptions {
            catalog: Some(&catalog),
            tokens: Some(&tokens),
            ..ParseOptions::default()
        },
    );
    assert_eq!(common::codes(&r.diagnostics), ["W306"]);
    assert_eq!(
        r.diagnostics[0].hint.as_deref(),
        Some("did you mean \"space.md\"?")
    );
}

#[test]
fn a_token_of_the_wrong_type_is_w307() {
    let catalog = catalog();
    let tokens = tokens();
    let r = parse(
        &screen("<stack id=\"k\" gap=\"{token.color.accent}\"/>"),
        &ParseOptions {
            catalog: Some(&catalog),
            tokens: Some(&tokens),
            ..ParseOptions::default()
        },
    );
    assert_eq!(common::codes(&r.diagnostics), ["W307"]);
}

#[test]
fn action_names_are_checked_only_when_an_action_list_is_given() {
    let markup = screen("<button id=\"b\" on-press=\"auth.login\">Go</button>");
    assert!(markup_codes(&markup).is_empty());
    let catalog = catalog();
    let actions = vec!["auth.logout".to_owned()];
    let r = parse(
        &markup,
        &ParseOptions {
            catalog: Some(&catalog),
            actions: Some(&actions),
            ..ParseOptions::default()
        },
    );
    assert_eq!(common::codes(&r.diagnostics), ["W308"]);
    let known = vec!["auth.login".to_owned()];
    let r = parse(
        &markup,
        &ParseOptions {
            catalog: Some(&catalog),
            actions: Some(&known),
            ..ParseOptions::default()
        },
    );
    assert!(r.diagnostics.is_empty());
}

#[test]
fn an_empty_action_list_rejects_every_action() {
    let catalog = catalog();
    let none: Vec<String> = vec![];
    let r = parse(
        &screen("<button id=\"b\" on-press=\"go\">Go</button>"),
        &ParseOptions {
            catalog: Some(&catalog),
            actions: Some(&none),
            ..ParseOptions::default()
        },
    );
    assert_eq!(common::codes(&r.diagnostics), ["W308"]);
}

#[test]
fn without_a_catalog_the_token_and_action_options_have_nothing_to_check() {
    let tokens = tokens();
    let actions: Vec<String> = vec![];
    let r = parse(
        "<screen id=\"s\" weft=\"0.1\" gap=\"{token.nope}\" on-x=\"y\"/>",
        &ParseOptions {
            tokens: Some(&tokens),
            actions: Some(&actions),
            ..ParseOptions::default()
        },
    );
    assert!(r.diagnostics.is_empty());
}

#[test]
fn parsed_nodes_ignore_their_positions_when_compared() {
    let a = document(&screen("<stack id=\"a\"/>"));
    let b = document(&screen("\n\n  <stack   id=\"a\"/>"));
    assert_eq!(a, b);
}
