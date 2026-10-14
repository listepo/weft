//! The built module end to end: `weft.wasm`, as `cox plugin install plugins/cox` takes it, loaded
//! with the extism engine of cox's plugin host (no WASI, a memory cap, per-call deadlines) and
//! driven through `cox_init` and `cox_tool_call` with the JSON cox sends. `moon run
//! cox-plugin:test` builds the module first; a plain `cargo test` needs `moon run
//! cox-plugin:build` once.

// Helpers of a test file fail the test the same way a #[test] body does.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use cox_plugin_sdk::Value;
use serde_json::json;
use wasm_plugin_host::extism::{CurrentPlugin, Error, Function, PTR, UserData, Val};
use wasm_plugin_host::{HostEnv, Lane, Limits, Options, PluginError, PluginHost};

const OPTIONS: Options = Options::new("cox_init", "weft-e2e");

/// The host side of the one `cox:host/v1` import the module has, `cox_notify`, which it calls
/// only for a project's warnings: accepted and dropped.
struct Env;

impl HostEnv for Env {
    fn id(&self) -> &str {
        "weft"
    }

    fn functions(self: &Arc<Self>) -> Vec<Function> {
        let notify = Function::new(
            "cox_notify",
            [PTR],
            [PTR],
            UserData::new(()),
            |plugin: &mut CurrentPlugin, _: &[Val], outputs: &mut [Val], _: UserData<()>| {
                let reply = plugin.memory_new(r#"{"Ok":null}"#)?;
                outputs[0] = plugin.memory_to_val(reply);
                Ok::<(), Error>(())
            },
        )
        .with_namespace("cox:host/v1");
        vec![notify]
    }
}

fn load() -> PluginHost {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/weft.wasm");
    let wasm = std::fs::read(path)
        .unwrap_or_else(|e| panic!("{path}: {e}; run `moon run cox-plugin:build` first"));
    // The limits plugin.toml asks for, as cox would apply them.
    let limits = Limits {
        memory_mib: Some(64),
        call_ms: Some(10_000),
    };
    PluginHost::load(&wasm, &limits, &OPTIONS, Arc::new(Env)).unwrap()
}

fn init(host: &PluginHost, config: Value) -> Result<Option<Value>, PluginError> {
    init_on(host, config, "terminal", json!({ "tools": [] }))
}

fn init_on(
    host: &PluginHost,
    config: Value,
    surface: &str,
    granted: Value,
) -> Result<Option<Value>, PluginError> {
    let input = json!({
        "api": 1,
        "plugin_id": "weft",
        "config": config,
        "session": { "id": "s", "cwd": "/w" },
        "granted": granted,
        "surface": surface,
    });
    host.call(Lane::Control, "cox_init", &input, Duration::from_secs(10))
}

fn export(host: &PluginHost, name: &str, input: Value) -> Value {
    host.call(Lane::Control, name, &input, Duration::from_secs(10))
        .unwrap()
        .unwrap()
}

fn tool(host: &PluginHost, name: &str, input: Value) -> Value {
    let call = json!({ "name": name, "input": input });
    host.call(
        Lane::Control,
        "cox_tool_call",
        &call,
        Duration::from_secs(10),
    )
    .unwrap()
    .unwrap()
}

#[test]
fn init_lists_the_tools_and_a_call_checks_markup() {
    let host = load();
    let out = init(&host, json!({ "validate": { "mode": "strict" } }))
        .unwrap()
        .unwrap();
    let tools = out["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, weft_cox::tool_names());

    let invalid = tool(
        &host,
        "weft_validate",
        json!({ "markup": "<screen weft=\"0.1\" id=\"s\" label=\"S\"><text>Hi</text></screen>" }),
    );
    assert_eq!(invalid["is_error"], false, "{invalid}");
    let report: Value = serde_json::from_str(invalid["text"].as_str().unwrap()).unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(report["diagnostics"][0]["code"], "W202");
}

#[test]
fn the_module_answers_as_the_native_build() {
    let host = load();
    init(&host, Value::Null).unwrap();
    let (mut native, _) = weft_cox::Weft::from_config(&json!({})).unwrap();
    for (name, input) in [
        ("weft_primer", json!({})),
        ("weft_catalog", json!({ "kind": "button" })),
        (
            "weft_format",
            json!({ "markup": "<screen weft=\"0.1\" id=\"s\" label=\"S\"/>" }),
        ),
        ("weft_validate", json!({})),
        (
            "weft_render",
            json!({ "markup": "<screen weft=\"0.1\" id=\"s\" label=\"S\"><button id=\"b\" on-press=\"a.b\">Go</button></screen>", "html": true }),
        ),
    ] {
        let call = cox_plugin_sdk::ToolCallIn {
            name: name.into(),
            input: input.clone(),
        };
        assert_eq!(tool(&host, name, input), native.call(&call), "{name}");
    }
}

#[test]
fn a_broken_config_fails_init_and_the_tools_with_it() {
    let host = load();
    let Err(PluginError::Trap { message, .. }) = init(&host, json!({ "catalog": 3 })) else {
        panic!("a catalog that is a number fails init");
    };
    assert!(message.contains("[plugins.weft] has errors"), "{message}");
    let call = json!({ "name": "weft_primer", "input": {} });
    let refused = host.call::<_, Value>(
        Lane::Control,
        "cox_tool_call",
        &call,
        Duration::from_secs(10),
    );
    assert!(refused.is_err());
}

/// The Design tab through the module: the desktop gets the pane, a tool call fills it with the
/// native build's page, and a click on an element comes back as its attachment.
#[test]
fn the_desktop_canvas_shows_what_the_tools_saw() {
    let host = load();
    let granted = json!({ "tools": [], "desktop": { "canvas": true } });
    let out = init_on(&host, json!({}), "desktop", granted.clone())
        .unwrap()
        .unwrap();
    assert_eq!(out["desktop"]["canvas"]["select_attr"], "data-weft-path");
    let (mut native, _) = weft_cox::Weft::from_config(&json!({})).unwrap();
    native.open_canvas(Some(cox_plugin_sdk::Surface::Desktop), &granted);

    let markup = "<screen weft=\"0.1\" id=\"home\" label=\"Home\"><button id=\"buy\" on-press=\"shop.buy\" text=\"Buy\"/></screen>";
    let input = json!({ "markup": markup });
    tool(&host, "weft_validate", input.clone());
    native.call(&cox_plugin_sdk::ToolCallIn {
        name: "weft_validate".into(),
        input,
    });
    let frames = export(
        &host,
        "cox_canvas",
        json!({ "width_pt": 800, "height_pt": 600 }),
    );
    assert_eq!(frames, serde_json::to_value(native.frames()).unwrap());
    assert_eq!(frames["frames"][0]["title"], "home");
    let html = frames["frames"][0]["html"].as_str().unwrap();
    assert!(
        html.contains("data-weft-path=\"/screen#home/button#buy\""),
        "{html}"
    );

    let click = json!({ "frame": frames["frames"][0]["id"], "value": "/screen#home/button#buy" });
    let attach = export(&host, "cox_canvas_select", click);
    assert_eq!(attach["name"], "Weft: button#buy");
    assert!(
        attach["text"].as_str().unwrap().contains("Markup line 1:"),
        "{attach}"
    );
    let elsewhere = json!({ "frame": "s9", "value": "/screen#home" });
    let none: Option<Value> = host
        .call(
            Lane::Control,
            "cox_canvas_select",
            &elsewhere,
            Duration::from_secs(10),
        )
        .unwrap();
    assert!(none.as_ref().is_none_or(Value::is_null), "{none:?}");
}
