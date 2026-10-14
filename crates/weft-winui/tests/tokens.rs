//! `WeftTokens.xaml`: the dimension tokens as `x:Double` resources in effective pixels, the other
//! types left out with a reason, and token paths from a project's files kept inside their
//! attribute. Also the file set `Generated::files` hands the CLI.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{controls, options, setup};
use weft_catalog::load_tokens;
use weft_core::parse_json;
use weft_winui::{TOKENS_FILE, generate, generate_tokens};

#[test]
fn dimensions_are_written_in_effective_pixels_and_the_rest_is_left_out() {
    let tokens = load_tokens(
        &parse_json(
            r#"{
  "space": { "$type": "dimension", "sm": { "$value": { "value": 12, "unit": "px" } },
             "lg": { "$value": { "value": 1.5, "unit": "rem" } } },
  "ink": { "$type": "color", "$value": { "colorSpace": "srgb", "components": [0, 0, 0] } }
}"#,
        )
        .unwrap(),
    )
    .tokens;
    let (xaml, skipped) = generate_tokens(&tokens);
    assert!(
        xaml.contains("    <x:Double x:Key=\"Weft.space.sm\">12</x:Double>\n"),
        "{xaml}"
    );
    assert!(
        xaml.contains("    <x:Double x:Key=\"Weft.space.lg\">24</x:Double>\n"),
        "{xaml}"
    );
    assert!(!xaml.contains("ink"), "{xaml}");
    assert_eq!(
        skipped,
        ["ink: only dimension tokens are written, not color"]
    );
    roxmltree::Document::parse(&xaml).unwrap();
}

#[test]
fn the_default_tokens_give_a_well_formed_dictionary() {
    let (_, tokens) = setup();
    let (xaml, _) = generate_tokens(&tokens);
    let doc = roxmltree::Document::parse(&xaml).unwrap();
    assert_eq!(doc.root_element().tag_name().name(), "ResourceDictionary");
    let keys: Vec<_> = doc
        .root_element()
        .children()
        .filter_map(|n| n.attribute(("http://schemas.microsoft.com/winfx/2006/xaml", "Key")))
        .collect();
    assert!(keys.contains(&"Weft.space.sm"), "{keys:?}");
    assert_eq!(TOKENS_FILE, "WeftTokens.xaml");
}

#[test]
fn a_hostile_token_path_stays_inside_its_attribute() {
    let mut tokens = load_tokens(
        &parse_json(
            r#"{ "space": { "$type": "dimension", "$value": { "value": 4, "unit": "px" } } }"#,
        )
        .unwrap(),
    )
    .tokens;
    let token = tokens.shift_remove("space").unwrap();
    tokens.insert("a\"><Evil x=\"".to_owned(), token);
    let (xaml, _) = generate_tokens(&tokens);
    let doc = roxmltree::Document::parse(&xaml).unwrap();
    assert!(
        doc.descendants().all(|n| n.tag_name().name() != "Evil"),
        "{xaml}"
    );
}

#[test]
fn the_file_set_names_the_page_by_its_class() {
    let (catalog, tokens) = setup();
    let generated = generate(
        &controls(&catalog, &tokens),
        &options(&catalog, &tokens, None),
    )
    .unwrap();
    let files = generated.files();
    assert_eq!(
        files[0],
        (format!("{}.xaml", generated.page), generated.xaml.as_str())
    );
}
