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
    let input = json!({
        "api": 1,
        "plugin_id": "weft",
        "config": config,
        "session": { "id": "s", "cwd": "/w" },
        "granted": { "tools": [] },
        "surface": "terminal",
    });
    host.call(Lane::Control, "cox_init", &input, Duration::from_secs(10))
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
    let (native, _) = weft_cox::Weft::from_config(&json!({})).unwrap();
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
