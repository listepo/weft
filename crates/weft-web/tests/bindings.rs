//! Bindings the generators write and the importers must read back: a tab's bound label and the
//! tablist's bound `selected` (HTML, React and SolidJS), a number field's value written through
//! `_float(...)`, and an array index written through `_get(...)`.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use weft_catalog::{DEFAULT_TOKENS_JSON, core_catalog, load_tokens};
use weft_core::{ParseOptions, has_errors, parse, parse_json, serialize};
use weft_import::{ImportResult, LossKind};
use weft_web::{
    Framework, HtmlOptions, ImportOptions, JsxOptions, import_html, import_jsx, to_html, to_jsx,
};

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

fn generated(framework: Framework, typescript: bool) -> String {
    let catalog = core_catalog().unwrap();
    to_jsx(
        &serde_json::to_value(document()).unwrap(),
        &JsxOptions {
            catalog: &catalog,
            component_name: None,
            framework,
            typescript,
            source: false,
        },
    )
    .unwrap()
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

#[test]
fn the_jsx_importers_read_the_tab_number_and_index_bindings_back() {
    for framework in [Framework::React, Framework::Solid] {
        for typescript in [false, true] {
            let code = generated(framework, typescript);
            let what = format!("{framework:?} ts={typescript}");
            let markup = import(|o| import_jsx(&code, typescript, o));
            assert_bound(&markup, &what);
            for line in [
                r#"value="{$.account.age}""#,
                r#"<text id="lead" text="{$.players.0.name}"/>"#,
            ] {
                assert!(markup.contains(line), "{what}: {line} is missing in\n{markup}");
            }
        }
    }
}

/// The source is untrusted: `_get` only reads back segments a binding can spell, and only from a
/// data path. Anything else is a loss, not a binding.
#[test]
fn a_get_with_a_segment_no_binding_can_spell_is_not_read() {
    let source = r#"export default function C({ data }) {
  return (
    <main>
      <p data-weft-id="ok">{_get(data, ["rows", "10", "name"])}</p>
      <p data-weft-id="dash">{_get(data, ["a-b"])}</p>
      <p data-weft-id="zero">{_get(data, ["rows", "01"])}</p>
      <p data-weft-id="open">{_get(window, ["name"])}</p>
      <p data-weft-id="dynamic">{_get(data, [data.key])}</p>
    </main>
  );
}
"#;
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let result = import_jsx(
        source,
        false,
        &ImportOptions {
            catalog: &catalog,
            tokens: &tokens,
        },
    );
    let markup = serialize(&result.document);
    assert!(markup.contains(r#"text="{$.rows.10.name}""#), "{markup}");
    assert_eq!(markup.matches("text=\"{").count(), 1, "{markup}");
    let lost = result
        .losses
        .iter()
        .filter(|l| l.kind == LossKind::Bindings)
        .count();
    assert_eq!(lost, 4, "{:?}", result.losses);
}
