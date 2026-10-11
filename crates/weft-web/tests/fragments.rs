//! Fragments in the web generators (SPEC §10.7): the static page and the components draw a use as
//! its fragment's body, with instance paths for ids.

#![allow(clippy::unwrap_used, clippy::panic)]

use weft_catalog::{DEFAULT_TOKENS_JSON, core_catalog, load_tokens};
use weft_core::{Catalog, Document, Fragment, ParseOptions, parse, parse_json};
use weft_web::{Framework, HtmlOptions, JsxOptions, to_html, to_jsx};

fn document(markup: &str, catalog: &Catalog) -> Document {
    let options = ParseOptions {
        catalog: Some(catalog),
        ..ParseOptions::default()
    };
    parse(markup, &options).document.unwrap()
}

fn setup() -> (Catalog, Document) {
    let mut catalog = core_catalog().unwrap();
    let fragment = r#"<fragment weft="0.2"><param name="title" type="string"/><param name="back" type="action"/><stack id="bar"><button id="back" on-press="{$back}">Back</button><heading id="title" level="1" text="{$title}"/></stack></fragment>"#;
    let fragment = document(fragment, &catalog);
    catalog
        .fragments
        .insert("header".into(), Fragment::new(fragment));
    let screen = r#"<screen id="s" weft="0.2"><use id="top" fragment="header" title="Cart" on-back="nav.back"/></screen>"#;
    let screen = document(screen, &catalog);
    (catalog, screen)
}

#[test]
fn the_static_page_draws_the_expansion() {
    let (catalog, screen) = setup();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let options = HtmlOptions {
        catalog: &catalog,
        tokens: &tokens,
        source: true,
        appearance: None,
    };
    let html = to_html(&screen, &options).unwrap();
    assert!(html.contains(r#"data-weft-id="top/bar""#), "{html}");
    assert!(html.contains(r#"data-action="press:nav.back""#), "{html}");
    assert!(html.contains(">Cart</h1>"), "{html}");
    // The source comment keeps the use as written.
    assert!(html.contains(r#"<use id="top""#), "{html}");
}

#[test]
fn an_inline_fragment_is_its_own_component() {
    let catalog = core_catalog().unwrap();
    let markup = r#"<screen id="s" weft="0.2"><fragment name="price-row"><param name="label" type="string"/><stack id="row"><text id="name" text="{$label}"/></stack></fragment><use id="sub" fragment="price-row" label="Subtotal"/></screen>"#;
    let screen = document(markup, &catalog);
    let json = serde_json::to_value(&screen).unwrap();
    let options = JsxOptions {
        catalog: &catalog,
        component_name: None,
        framework: Framework::React,
        typescript: false,
        source: false,
    };
    let jsx = to_jsx(&json, &options).unwrap();
    assert!(jsx.contains("function PriceRow("), "{jsx}");
    assert!(jsx.contains("<PriceRow "), "{jsx}");
    assert!(jsx.contains("Subtotal"), "{jsx}");
    assert!(jsx.contains("sub/row") || jsx.contains("\"sub\""), "{jsx}");
}

#[test]
fn the_components_draw_the_expansion() {
    let (catalog, screen) = setup();
    let json = serde_json::to_value(&screen).unwrap();
    let options = JsxOptions {
        catalog: &catalog,
        component_name: None,
        framework: Framework::React,
        typescript: false,
        source: false,
    };
    let jsx = to_jsx(&json, &options).unwrap();
    assert!(jsx.contains(r#""top/bar""#), "{jsx}");
    assert!(jsx.contains("Cart"), "{jsx}");
    assert!(!jsx.contains("fragment"), "{jsx}");
}
