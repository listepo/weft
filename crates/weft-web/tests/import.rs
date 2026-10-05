//! Source that the generators did not write: hand-written components and pages (the corpus's own
//! `screen.jsx` and `screen.html`, and the samples in `tests/fixtures/handwritten`) import to
//! pinned documents with pinned losses, and any input at all imports without panicking to a
//! document that validates in lenient mode (SPEC §9). Regenerate the pins with
//! WEFT_UPDATE_FIXTURES=1.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, RngSeed};
use weft_catalog::{DEFAULT_TOKENS_JSON, Token, core_catalog, load_tokens};
use weft_core::{
    Catalog, Mode, ValidateOptions, has_errors, parse_json, serialize, validate_document,
};
use weft_import::ImportResult;
use weft_web::{ImportOptions, import_html, import_jsx};

const HANDWRITTEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/handwritten");

fn setup() -> (Catalog, IndexMap<String, Token>) {
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    (core_catalog().unwrap(), tokens)
}

fn import(path: &Path, catalog: &Catalog, tokens: &IndexMap<String, Token>) -> ImportResult {
    let source = std::fs::read_to_string(path).unwrap();
    let options = ImportOptions { catalog, tokens };
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => import_html(&source, &options),
        Some("tsx") => import_jsx(&source, true, &options),
        _ => import_jsx(&source, false, &options),
    }
}

fn assert_lenient_valid(result: &ImportResult, catalog: &Catalog) {
    let diagnostics = validate_document(
        &result.document,
        &ValidateOptions {
            catalog: Some(catalog),
            mode: Mode::Lenient,
            tokens: None,
            actions: None,
        },
    );
    assert!(!has_errors(&diagnostics), "{diagnostics:#?}");
}

/// Compares `actual` with the pinned file, or rewrites the pin when asked to.
fn pinned(name: &str, actual: &str) {
    let path = format!("{HANDWRITTEN}/expected/{name}");
    if std::env::var("WEFT_UPDATE_FIXTURES").as_deref() == Ok("1") {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        actual, expected,
        "{name} changed; regenerate with WEFT_UPDATE_FIXTURES=1"
    );
}

/// Every hand-written input with the name its pins carry.
fn inputs() -> Vec<(String, PathBuf)> {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(corpus).unwrap() {
        let dir = entry.unwrap().path();
        let screen = dir.file_name().unwrap().to_string_lossy().into_owned();
        for file in ["screen.jsx", "screen.html"] {
            let path = dir.join(file);
            if path.exists() {
                let ext = file.rsplit('.').next().unwrap();
                out.push((format!("corpus-{screen}.{ext}"), path));
            }
        }
    }
    for entry in std::fs::read_dir(HANDWRITTEN).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                path,
            ));
        }
    }
    out.sort();
    out
}

#[test]
fn hand_written_sources_import_with_the_expected_losses() {
    let (catalog, tokens) = setup();
    let inputs = inputs();
    assert!(inputs.len() >= 20, "{}", inputs.len());
    for (name, path) in inputs {
        let result = import(&path, &catalog, &tokens);
        assert!(
            !has_errors(&result.diagnostics),
            "{name}: {:#?}",
            result.diagnostics
        );
        assert_lenient_valid(&result, &catalog);
        pinned(&format!("{name}.weft"), &serialize(&result.document));
        let losses = serde_json::to_string_pretty(&result.losses).unwrap() + "\n";
        pinned(&format!("{name}.losses.json"), &losses);
    }
}

#[test]
fn unreadable_sources_say_so() {
    let (catalog, tokens) = setup();
    let options = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    for (source, code) in [
        ("export default function (", "W601"),
        ("const x = 1;", "W601"),
        ("export default function A() { return 1; }", "W601"),
    ] {
        let result = import_jsx(source, false, &options);
        let codes: Vec<&str> = result.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, [code], "{source}");
        assert_lenient_valid(&result, &catalog);
    }
    let deep = format!(
        "export default () => {}<b/>{};",
        "<a>{x ? (".repeat(300),
        ") : null}</a>".repeat(300)
    );
    let result = import_jsx(&deep, false, &options);
    let codes: Vec<&str> = result.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["W602"]);
}

fn config() -> Config {
    Config {
        cases: 256,
        max_shrink_iters: 512,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0x3_35),
        ..Config::default()
    }
}

/// Pieces of components, joined in any order: mostly broken source that still reaches the reader.
fn jsx_fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("export default function A({ data, actions }) {"),
        Just("export default function B(props) {"),
        Just("const x = data.items;"),
        Just("return ("),
        Just("<main aria-label=\"m\">"),
        Just("</main>"),
        Just("<ul className=\"row gap-sm\">"),
        Just("</ul>"),
        Just("{data.items.map((item, i) => (<li key={i}>{item.name}</li>))}"),
        Just("{data.items.length > 0 ? <p>a</p> : <p>b</p>}"),
        Just("{data.on && <dialog open>d</dialog>}"),
        Just("<For each={props.data.list}>{(x) => <b>{x.y}</b>}</For>"),
        Just("<Show when={props.data.ok} fallback={<i/>}>"),
        Just("</Show>"),
        Just("<button onClick={() => actions.a.b()}>go</button>"),
        Just("<input value={data.q} onChange={(e) => actions.set(\"q\", e.target.value)} />"),
        Just("<input type=\"radio\" checked={data.p === \"x\"} />"),
        Just("<Thing {...rest} />"),
        Just(
            "{(() => { const _o = [{ el: <option>o</option> }]; return _o.map((_x) => _x.el); })()}"
        ),
        Just("style={{ gap: \"var(--weft-space-sm)\", padding: 4 }}"),
        Just("`${data.a}`"),
        Just("/re(g)ex/"),
        Just("{"),
        Just("}"),
        Just("("),
        Just(")"),
        Just(");"),
        Just("<"),
        Just(">"),
        Just("\""),
    ]
    .prop_map(String::from)
}

fn jsx_source() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        prop::collection::vec(jsx_fragment(), 0..40).prop_map(|parts| parts.join("\n")),
        prop::collection::vec(jsx_fragment(), 0..40).prop_map(|parts| format!(
            "export default function S({{ data, actions }}) {{\nreturn (<main>\n{}\n</main>);\n}}\n",
            parts.join("\n")
        )),
    ]
}

fn html_fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("<main aria-label=\"m\" data-weft-id=\"m\">"),
        Just("</main>"),
        Just("<ul class=\"stack gap-sm\">"),
        Just("</ul>"),
        Just("<template data-each=\"$.items\" data-as=\"item\">"),
        Just("<template data-empty>"),
        Just("</template>"),
        Just("<li data-bind=\"text:$item.name; hidden:!$.x\">x</li>"),
        Just("<button data-action=\"press:a.b; bogus\">b</button>"),
        Just("<input data-bind=\"value:$.q\" aria-label=\"q\">"),
        Just("<dialog open data-weft-slot=\"actions\">"),
        Just("<div data-prop-tone=\"{token.color.text}\" data-weft-kind=\"text\">"),
        Just("<footer>"),
        Just("<!-- weft:source\n<screen id=\"s\"/>\n-->"),
        Just("<"),
        Just(">"),
        Just("\""),
    ]
    .prop_map(String::from)
}

fn html_source() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        prop::collection::vec(html_fragment(), 0..40).prop_map(|parts| parts.join("\n")),
    ]
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn any_jsx_imports_to_a_lenient_valid_document(source in jsx_source()) {
        let (catalog, tokens) = setup();
        let options = ImportOptions { catalog: &catalog, tokens: &tokens };
        assert_lenient_valid(&import_jsx(&source, false, &options), &catalog);
        assert_lenient_valid(&import_jsx(&source, true, &options), &catalog);
    }

    #[test]
    fn any_html_imports_to_a_lenient_valid_document(source in html_source()) {
        let (catalog, tokens) = setup();
        let options = ImportOptions { catalog: &catalog, tokens: &tokens };
        assert_lenient_valid(&import_html(&source, &options), &catalog);
    }
}
