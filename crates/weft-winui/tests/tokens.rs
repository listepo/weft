//! `WeftTokens.xaml`: the dimension tokens as `x:Double` resources in effective pixels, the other
//! types left out with a reason, and token paths from a project's files kept inside their
//! attribute. Also the file set `Generated::files` hands the CLI.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{controls, options, setup};
use weft_catalog::{Appearance, load_tokens};
use weft_core::parse_json;
use weft_winui::{TOKENS_FILE, generate, generate_tokens};

#[test]
fn dimensions_are_written_in_effective_pixels_and_the_rest_is_left_out() {
    let tokens = load_tokens(
        &parse_json(
            r#"{
  "space": { "$type": "dimension", "sm": { "$value": { "value": 12, "unit": "px" } },
             "lg": { "$value": { "value": 1.5, "unit": "rem" } } },
  "slow": { "$type": "duration", "$value": { "value": 200, "unit": "ms" } }
}"#,
        )
        .unwrap(),
    )
    .tokens;
    let (xaml, skipped) = generate_tokens(&tokens, None);
    assert!(
        xaml.contains("    <x:Double x:Key=\"Weft.space.sm\">12</x:Double>\n"),
        "{xaml}"
    );
    assert!(
        xaml.contains("    <x:Double x:Key=\"Weft.space.lg\">24</x:Double>\n"),
        "{xaml}"
    );
    assert!(!xaml.contains("slow"), "{xaml}");
    assert_eq!(
        skipped,
        ["slow: only dimension, colour and material tokens are written, not duration"]
    );
    roxmltree::Document::parse(&xaml).unwrap();
}

/// The resources of one theme dictionary, by key, as `Color`'s text or a brush's attributes.
fn theme(xaml: &str, name: &str) -> Vec<(String, String)> {
    let doc = roxmltree::Document::parse(xaml).unwrap();
    let key = |n: roxmltree::Node| {
        n.attribute(("http://schemas.microsoft.com/winfx/2006/xaml", "Key"))
            .unwrap_or_default()
            .to_owned()
    };
    let dictionary = doc
        .descendants()
        .find(|n| n.tag_name().name() == "ResourceDictionary" && key(*n) == name)
        .unwrap();
    dictionary
        .children()
        .filter(roxmltree::Node::is_element)
        .map(|n| {
            let mut what = n.tag_name().name().to_owned();
            for a in n.attributes().filter(|a| a.namespace().is_none()) {
                what += &format!(" {}={}", a.name(), a.value());
            }
            what += n.text().unwrap_or_default();
            (key(n), what)
        })
        .collect()
}

#[test]
fn colours_and_materials_are_theme_resources_with_their_dark_values() {
    let light = load_tokens(
        &parse_json(
            r##"{
  "ink": { "$type": "color", "$value": { "colorSpace": "srgb", "components": [0, 0, 0], "alpha": 0.5 } },
  "accent": { "$type": "color", "$value": "#336699" },
  "glass": { "$type": "color",
    "$value": { "colorSpace": "srgb", "components": [1, 1, 1], "alpha": 0.28, "hex": "#ffffff" },
    "$extensions": { "dev.weft.material": { "blur": { "value": 20, "unit": "px" } } } }
}"##,
        )
        .unwrap(),
    )
    .tokens;
    let mut dark = light.clone();
    dark["ink"].value = parse_json(r##""#ffffff""##).unwrap();
    let appearance = Appearance {
        modifier: "theme",
        light_context: "light",
        dark_context: "dark",
        light: &light,
        dark: &dark,
    };
    let (xaml, skipped) = generate_tokens(&light, Some(appearance));
    assert!(skipped.is_empty(), "{skipped:?}");
    let pairs = |list: &[(&str, &str)]| -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    };
    let glass = "AcrylicBrush TintColor=#FFFFFFFF TintOpacity=0.28 FallbackColor=#47FFFFFF";
    assert_eq!(
        theme(&xaml, "Light"),
        pairs(&[
            ("Weft.ink.color", "Color#80000000"),
            ("Weft.ink", "SolidColorBrush Color=#80000000"),
            ("Weft.accent.color", "Color#FF336699"),
            ("Weft.accent", "SolidColorBrush Color=#FF336699"),
            ("Weft.glass", glass),
        ])
    );
    assert_eq!(theme(&xaml, "Dark")[0].1, "Color#FFFFFFFF");
    // High contrast keeps the light colours and draws a material as its plain fallback.
    let contrast = theme(&xaml, "HighContrast");
    assert_eq!(contrast[0].1, "Color#80000000");
    assert_eq!(
        contrast[4],
        pairs(&[("Weft.glass", "SolidColorBrush Color=#47FFFFFF")])[0]
    );
}

#[test]
fn the_default_tokens_give_a_well_formed_dictionary() {
    let (_, tokens) = setup();
    let (xaml, _) = generate_tokens(&tokens, None);
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
    let (xaml, _) = generate_tokens(&tokens, None);
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
