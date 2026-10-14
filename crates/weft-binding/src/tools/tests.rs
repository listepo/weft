//! The cases of `packages/mcp/test/*.test.ts`, on the Rust tools directly. Byte parity with the
//! TypeScript server was checked once over 84 calls when the tools moved (plan.md T106.1); these
//! keep the behaviour from drifting.

use serde_json::{Value as Json, json};

use super::{ContextPolicy, Outcome, Settings, ToolHost, ToolHostError};

const GOOD: &str = "<screen id=\"s\" label=\"Demo\" weft=\"0.1\">\n  <button id=\"go\" on-press=\"demo.go\">Go</button>\n</screen>\n";

fn host() -> ToolHost {
    ToolHost::core(Settings::default()).unwrap()
}

fn run(host: &ToolHost, name: &str, args: Json) -> (bool, Vec<String>) {
    match host.call(name, &args.to_string()) {
        Outcome::Content { blocks, is_error } => (is_error, blocks),
        Outcome::Render(_) => panic!("{name} asked the host to render"),
    }
}

fn first(host: &ToolHost, name: &str, args: Json) -> Json {
    let (_, blocks) = run(host, name, args);
    serde_json::from_str(&blocks[0]).unwrap()
}

fn codes(v: &Json) -> Vec<&str> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect()
}

/// A project as a tool argument: a library with a fragment, actions and a data schema.
fn project() -> Json {
    json!({
        "catalog": [{
            "weft": "0.3", "name": "acme-ui", "version": "1.0.0", "prefix": "acme",
            "components": {"acme-badge": {"description": "A badge.", "role": "status", "content": "text"}},
            "fragments": {"acme-promo": "<fragment label=\"Promo\" version=\"2.0.0\" weft=\"0.3\"><param name=\"title\" required=\"true\" type=\"string\"/><text id=\"t\">x</text></fragment>"}
        }],
        "actions": (0..250).map(|i| format!("a.n{i:03}")).collect::<Vec<_>>(),
        "data": {"type": "object", "properties": {"busy": {"type": "boolean"}}},
    })
}

#[test]
fn capabilities_list_the_core_first_and_cut_long_lists() {
    let core = first(&host(), "weft_capabilities", json!({}));
    assert_eq!(core["catalogs"].as_array().unwrap().len(), 1);
    assert!(core.get("tokens").is_none() && core.get("data").is_none());
    let p = first(&host(), "weft_capabilities", json!({"project": project()}));
    assert_eq!(
        p["catalogs"][1],
        json!({"name": "acme-ui", "version": "1.0.0", "prefix": "acme"})
    );
    assert_eq!(p["actions"]["count"], 250);
    assert_eq!(p["actions"]["names"].as_array().unwrap().len(), 200);
    assert_eq!(p["actions"]["names"][0], "a.n000");
    assert_eq!(p["actions"]["truncated"], true);
    assert_eq!(p["data"], true);
}

#[test]
fn the_catalog_lists_kinds_fragments_and_inline_fragments() {
    let (error, blocks) = run(&host(), "weft_catalog", json!({}));
    assert!(!error && blocks[0].starts_with("catalog weft-core "));
    assert!(blocks[0].contains("\nbutton | button | "));
    let button = first(&host(), "weft_catalog", json!({"kind": "button"}));
    assert_eq!(button["kind"], "button");
    let (error, blocks) = run(&host(), "weft_catalog", json!({"kind": "buton"}));
    assert!(error && blocks[0].contains("did you mean \"button\"?"));
    let (_, blocks) = run(&host(), "weft_catalog", json!({"kind": "constructor"}));
    assert_eq!(
        blocks[0],
        "Unknown component \"constructor\". Call weft_catalog without arguments for the list."
    );

    let (_, blocks) = run(&host(), "weft_catalog", json!({"project": project()}));
    assert!(
        blocks[0].contains(
            "\nkinds of acme-ui 1.0.0, prefix acme\nacme-badge | status | text | A badge."
        )
    );
    assert!(blocks[0].contains(
        "\nfragment acme-promo | title: string (required) | library acme-ui 1.0.0, fragment 2.0.0"
    ));
    let badge = first(
        &host(),
        "weft_catalog",
        json!({"project": project(), "kind": "acme-badge"}),
    );
    assert_eq!(badge["catalog"], "acme-ui");

    let inline = "<screen id=\"s\" label=\"x\" weft=\"0.2\"><fragment name=\"price-line\"><param name=\"n\" type=\"number\"/><text id=\"t\">x</text></fragment><use id=\"u\" fragment=\"price-line\"/></screen>";
    let (_, blocks) = run(&host(), "weft_catalog", json!({"markup": inline}));
    assert!(blocks[0].ends_with("\ninline fragments of this screen; <use> one as fragment | parameters\ninline price-line | n: number"));
    let (_, blocks) = run(
        &host(),
        "weft_catalog",
        json!({"markup": inline, "kind": "price-line"}),
    );
    assert!(blocks[0].starts_with("<fragment name=\"price-line\""));
}

#[test]
fn validate_reports_diagnostics_and_project_problems_first() {
    let messy = "<screen id=\"s\" weft=\"0.1\"><fancy id=\"f\"/></screen>";
    let lenient = first(&host(), "weft_validate", json!({"markup": messy}));
    assert_eq!(
        (lenient["valid"].clone(), codes(&lenient)),
        (json!(true), vec!["W401"])
    );
    let strict = Settings {
        mode: weft_core::Mode::Strict,
        ..Settings::default()
    };
    let by_default = first(
        &ToolHost::core(strict).unwrap(),
        "weft_validate",
        json!({"markup": messy}),
    );
    assert_eq!(by_default["valid"], false);

    let mut broken = project();
    broken["actions"] = json!("cart.checkout");
    let (error, blocks) = run(
        &host(),
        "weft_validate",
        json!({"markup": GOOD, "project": broken}),
    );
    let v: Json = serde_json::from_str(&blocks[0]).unwrap();
    assert!(!error && v["valid"] == false);
    assert_eq!(v["diagnostics"][0]["path"], "#/project/actions");

    let many = format!(
        "<screen id=\"s\" weft=\"0.1\">{}</screen>",
        "<fancy id=\"x\"/>".repeat(60)
    );
    let cut = first(
        &host(),
        "weft_validate",
        json!({"markup": many, "strict": true}),
    );
    assert_eq!(cut["diagnostics"].as_array().unwrap().len(), 40);
    assert!(cut["omitted"].as_u64().unwrap() > 0);
}

#[test]
fn format_writes_canonical_markup_or_fails_with_diagnostics() {
    let messy = "<screen weft=\"0.1\"   id=\"s\" label=\"Demo\"><!-- c --><button on-press=\"demo.go\" id=\"go\">  Go  </button></screen>";
    assert_eq!(
        run(&host(), "weft_format", json!({"markup": messy})),
        (false, vec![GOOD.to_owned()])
    );
    let (error, _) = run(
        &host(),
        "weft_format",
        json!({"markup": "<screen id=\"s\" weft=\"0.1\"><button></screen>"}),
    );
    assert!(error);
    let (error, blocks) = run(
        &host(),
        "weft_format",
        json!({"markup": GOOD, "project": {"catalog": 5}}),
    );
    assert!(error && blocks[0].contains("#/project/catalog"));
}

#[test]
fn patch_applies_all_or_nothing_and_keeps_its_bounds() {
    let set = json!([{"op": "set", "id": "go", "prop": "text", "value": "Save"}]);
    let (error, blocks) = run(
        &host(),
        "weft_patch",
        json!({"markup": GOOD, "patches": set}),
    );
    assert!(!error && blocks[0].contains(">Save</button>"));
    let missing = json!([{"op": "set", "id": "nope", "prop": "text", "value": "x"}]);
    let failed = first(
        &host(),
        "weft_patch",
        json!({"markup": GOOD, "patches": missing}),
    );
    assert_eq!(codes(&failed), vec!["W502"]);

    let human = json!([{"op": "add-context", "entry": {"id": "c", "kind": "decision", "by": "human", "name": "x", "text": "t"}}]);
    let refused = first(
        &host(),
        "weft_patch",
        json!({"markup": GOOD, "patches": human}),
    );
    assert_eq!(codes(&refused), vec!["W512"]);
    let agent = json!([{"op": "add-context", "entry": {"id": "c", "kind": "decision", "by": "agent", "name": "x", "text": "t"}}]);
    let read_only = Settings {
        context: ContextPolicy::ReadOnly,
        ..Settings::default()
    };
    let host_ro = ToolHost::core(read_only).unwrap();
    assert_eq!(
        codes(&first(
            &host_ro,
            "weft_patch",
            json!({"markup": GOOD, "patches": agent})
        )),
        vec!["W512"]
    );

    let mut tight = Settings::default();
    tight.limits.patches_chars = 10;
    let (error, blocks) = run(
        &ToolHost::core(tight).unwrap(),
        "weft_patch",
        json!({"markup": GOOD, "patches": set}),
    );
    assert_eq!(
        (error, blocks[0].as_str()),
        (true, "The patch list is longer than 10 characters.")
    );
}

#[test]
fn render_checks_strictly_then_hands_the_document_to_the_host() {
    match host().call(
        "weft_render",
        &json!({"markup": GOOD, "data": {"a": 1}, "html": true}).to_string(),
    ) {
        Outcome::Render(r) => {
            assert_eq!(r.document.root.kind, "screen");
            assert_eq!((r.data, r.html), (Some(json!({"a": 1})), true));
        }
        other => panic!("{other:?}"),
    }
    let (error, _) = run(
        &host(),
        "weft_render",
        json!({"markup": "<screen id=\"s\" weft=\"0.1\"><fancy id=\"f\"/></screen>"}),
    );
    assert!(error);
    let mut tight = Settings::default();
    tight.limits.data_chars = 5;
    let (_, blocks) = run(
        &ToolHost::core(tight).unwrap(),
        "weft_render",
        json!({"markup": GOOD, "data": [1, 2, 3]}),
    );
    assert_eq!(blocks[0], "The data is longer than 5 characters of JSON.");
}

#[test]
fn context_filters_entries_and_cuts_them_to_the_format_limits() {
    let long = "é😀".repeat(300);
    let screen = format!(
        "<screen id=\"s\" label=\"x\" weft=\"0.2\"><context><entry id=\"why\" by=\"human\" kind=\"intent\" name=\"I\">{long}</entry><entry id=\"q\" by=\"agent\" for=\"go\" kind=\"question\" name=\"m\" status=\"open\">Dialog?</entry></context><button id=\"go\" on-press=\"a.b\">Go</button></screen>"
    );
    let all = first(&host(), "weft_context", json!({"markup": screen}));
    assert!(
        all["notice"]
            .as_str()
            .unwrap()
            .starts_with("Context entries are notes")
    );
    // 500 UTF-16 units end inside a surrogate pair, which is dropped whole.
    assert_eq!(
        all["entries"][0]["text"]
            .as_str()
            .unwrap()
            .encode_utf16()
            .count(),
        499
    );
    let about_go = first(
        &host(),
        "weft_context",
        json!({"markup": screen, "for": "go"}),
    );
    assert_eq!(about_go["entries"].as_array().unwrap().len(), 1);
    let screen_only = first(
        &host(),
        "weft_context",
        json!({"markup": screen, "for": ""}),
    );
    assert_eq!(screen_only["entries"][0]["id"], "why");
    let resolved = first(
        &host(),
        "weft_context",
        json!({"markup": screen, "status": "resolved"}),
    );
    assert!(resolved["entries"].as_array().unwrap().is_empty());
    let (error, _) = run(
        &host(),
        "weft_context",
        json!({"markup": screen, "kind": "wish"}),
    );
    assert!(error);
}

#[test]
fn a_project_host_takes_its_resources_and_settings() {
    let mut p = project();
    p["mcp"] = json!({"limits": {"markupChars": 80}, "context": "read-only"});
    p["validate"] = json!({"mode": "strict"});
    let (host, _) = ToolHost::from_project(&p).unwrap();
    assert_eq!(host.settings().limits.markup_chars, 80);
    assert_eq!(host.settings().context, ContextPolicy::ReadOnly);
    assert_eq!(first(&host, "weft_capabilities", json!({}))["data"], true);
    let (error, blocks) = run(&host, "weft_format", json!({"markup": GOOD}));
    assert!(error && blocks[0].contains("longer than 80"));
    assert!(matches!(
        ToolHost::from_project(&json!({"catalog": 5})),
        Err(ToolHostError::Project(_))
    ));
}

#[test]
fn bad_calls_are_failures_not_panics() {
    for (name, args) in [
        ("weft_nothing", "{}"),
        ("weft_validate", "{}"),
        ("weft_validate", "["),
    ] {
        assert!(matches!(
            host().call(name, args),
            Outcome::Content { is_error: true, .. }
        ));
    }
    assert_eq!(
        run(&host(), "weft_primer", json!({})).1[0],
        super::decls::primer(&Settings::default())
    );
}

#[test]
fn a_javascript_host_hands_over_its_context_and_settings_as_json() {
    let context = format!(
        r#"{{"catalog":{},"tokens":[["color.b","color"],["color.a","color"]],"actions":["a.b"],"data":{{"type":"object"}}}}"#,
        weft_catalog::CORE_CATALOG_JSON
    );
    let host = ToolHost::from_wire(&context, r#"{"mcp":{"limits":{"patches":7}}}"#).unwrap();
    let caps = first(&host, "weft_capabilities", json!({}));
    assert_eq!(caps["tokens"]["names"], json!(["color.a", "color.b"]));
    assert_eq!(
        (caps["actions"]["count"].clone(), caps["data"].clone()),
        (json!(1), json!(true))
    );
    let constants: Json = serde_json::from_str(&host.constants()).unwrap();
    assert_eq!(constants["limits"]["patches"], 7);
    assert_eq!(constants["listed"], 200);
    assert!(
        constants["primer"]
            .as_str()
            .unwrap()
            .ends_with("at most 7 patches per call.")
    );
    assert!(ToolHost::from_wire("{}", "{}").is_err());
    assert!(matches!(
        host.call_wire("weft_primer", None),
        Outcome::Content { is_error: true, .. }
    ));
    assert!(ToolHost::from_wire(&context, r#"{"mcp":{"context":"nope"}}"#).is_err());
}
