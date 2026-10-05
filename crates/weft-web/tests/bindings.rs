//! Bindings the generators write and the importers must read back: a tab's bound label and the
//! tablist's bound `selected`.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use weft_catalog::{DEFAULT_TOKENS_JSON, core_catalog, load_tokens};
use weft_core::{ParseOptions, has_errors, parse, parse_json, serialize};
use weft_import::ImportResult;
use weft_web::{HtmlOptions, ImportOptions, import_html, to_html};

const SCREEN: &str = r#"<screen id="s" label="S" weft="0.1">
  <tabs id="t" selected="{$.folder}">
    <tab id="a" label="One">
      <text id="ta">A</text>
    </tab>
    <tab id="b" label="{$.labels.two}">
      <text id="tb">B</text>
    </tab>
  </tabs>
  <field id="age" label="Age" type="number" value="{$.account.age}"/>
  <text id="lead" text="{$.players.0.name}"/>
</screen>
"#;

fn document() -> weft_core::Document {
    let catalog = core_catalog().unwrap();
    let parsed = parse(
        SCREEN,
        &ParseOptions {
            catalog: Some(&catalog),
            ..Default::default()
        },
    );
    parsed.document.unwrap()
}

fn import(from: impl FnOnce(&ImportOptions<'_>) -> ImportResult) -> String {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let result = from(&ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    });
    assert!(!has_errors(&result.diagnostics), "{:?}", result.diagnostics);
    serialize(&result.document)
}

fn assert_bound(markup: &str, what: &str) {
    for line in [
        r#"<tabs id="t" selected="{$.folder}">"#,
        r#"<tab id="a" label="One">"#,
        r#"<tab id="b" label="{$.labels.two}">"#,
    ] {
        assert!(markup.contains(line), "{what}: {line} is missing in\n{markup}");
    }
}

#[test]
fn the_html_importer_reads_the_tab_bindings_back() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let html = to_html(
        &document(),
        &HtmlOptions {
            catalog: &catalog,
            tokens: &tokens,
            source: false,
            appearance: None,
        },
    )
    .unwrap();
    assert_bound(&import(|o| import_html(&html, o)), "html");
}
