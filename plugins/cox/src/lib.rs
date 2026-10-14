//! `weft-cox`: the Weft agent tools as a cox plugin (plan.md T106.3). The tools are the ones the
//! MCP server gives other hosts, from the same Rust (`weft_binding::tools`), so their texts, limits
//! and checks cannot drift between hosts. cox names them `wasm__weft__<tool>`.
//!
//! The decisions are plain functions so the tests run them on the host; the exports, which need
//! extism's memory, build only for `wasm32`.

use cox_plugin_sdk::{InitOut, ToolCallIn, Value};
use serde_json::json;
use weft_binding::tools::{Outcome, ToolHost, ToolHostError};

/// `weft_render` waits for the accessibility snapshot in Rust (T106.4): its page and tree come
/// from `@weft/render-react` in the MCP server.
const NOT_YET: &[&str] = &["weft_render"];

/// The tools this plugin serves, in listing order. `plugin.toml` grants the same names.
pub fn tool_names() -> Vec<String> {
    weft_binding::tools::decls::NAMES
        .iter()
        .filter(|name| !NOT_YET.contains(name))
        .map(|name| (*name).to_owned())
        .collect()
}

/// One session's tool host, from the user's `[plugins.weft]` table.
pub struct Weft {
    host: ToolHost,
}

impl Weft {
    /// Reads `InitIn.config`: a `weft.json` whose members hold their files' content (catalog,
    /// tokens, actions, data, fragments) plus its tool settings (`validate.mode`, `mcp.limits`,
    /// `mcp.context`). cox sends `{}` when the table is absent, which is the core catalog alone.
    /// The plugin has no file access, so inline content is the only way to give it a project.
    /// Returns the project's warnings beside it; an `Err` is the text cox shows when init fails.
    pub fn from_config(config: &Value) -> Result<(Weft, Vec<String>), String> {
        let empty = json!({});
        // A table cox could not read reaches the plugin as null, which means no project too.
        let config = if config.is_null() { &empty } else { config };
        match ToolHost::from_project(config) {
            Ok((host, warnings)) => Ok((Weft { host }, warnings.iter().map(line).collect())),
            Err(ToolHostError::Project(errors)) => {
                let lines: Vec<String> = errors.iter().map(line).collect();
                Err(format!("[plugins.weft] has errors:\n{}", lines.join("\n")))
            }
            Err(e) => Err(format!("[plugins.weft]: {e}")),
        }
    }

    /// One `ToolSpec` per tool. Every tool only reads its arguments, and cox runs the calls into
    /// one plugin one at a time, so each is `read_only` and `exclusive`; cox defers plugin tools
    /// behind `tool_search`.
    pub fn specs(&self) -> Vec<Value> {
        let names = tool_names();
        self.host
            .decls()
            .into_iter()
            .filter(|decl| names.iter().any(|name| decl["name"] == name.as_str()))
            .map(|decl| {
                json!({
                    "name": decl["name"],
                    "description": decl["description"],
                    "input_schema": decl["inputSchema"],
                    "deferred": true,
                    "risk": "read_only",
                    "concurrency": "exclusive",
                })
            })
            .collect()
    }

    pub fn declarations(&self) -> InitOut {
        InitOut {
            tools: self.specs(),
            ..InitOut::default()
        }
    }

    /// Runs one call and returns its `ToolOutput`. The MCP server's content blocks become one
    /// text, a blank line apart; a failure the model should fix stays a result, never an error of
    /// the export.
    pub fn call(&self, call: &ToolCallIn) -> Value {
        if !tool_names().contains(&call.name) {
            return output(&format!("Unknown tool: {}.", call.name), true);
        }
        // cox hands over the model's input as parsed JSON; the host reads it as text, as an MCP
        // call's arguments, so both hosts run the same checks on it.
        let args = call.input.to_string();
        match self.host.call(&call.name, &args) {
            Outcome::Content { blocks, is_error } => output(&blocks.join("\n\n"), is_error),
            // Unreachable while `weft_render` is held back; kept total for when it is served.
            Outcome::Render(_) => output("weft_render is not available in cox yet.", true),
        }
    }
}

fn output(text: &str, is_error: bool) -> Value {
    json!({ "text": text, "is_error": is_error, "diff": null, "structured": null })
}

/// A diagnostic as the tools print one: its JSON.
fn line(diagnostic: &impl serde::Serialize) -> String {
    serde_json::to_string(diagnostic).unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
mod guest {
    use std::sync::Mutex;

    use cox_plugin_sdk::{InitIn, InitOut, NoticeLevel, ToolCallIn, Value, notify};

    use super::Weft;

    // cox keeps one instance per plugin and session and calls it one export at a time; the lock
    // only satisfies `static`. `init` replaces the host, so a re-initialised instance never keeps
    // the previous session's project.
    static WEFT: Mutex<Option<Weft>> = Mutex::new(None);

    fn init(init: InitIn) -> Result<InitOut, String> {
        let (weft, warnings) = Weft::from_config(&init.config)?;
        let out = weft.declarations();
        *WEFT.lock().map_err(|_| "the plugin state is poisoned")? = Some(weft);
        for warning in warnings {
            // A refused notice only loses the warning; the project still loaded.
            let _ = notify(NoticeLevel::Warn, &format!("[plugins.weft]: {warning}"));
        }
        Ok(out)
    }

    fn tool_call(call: ToolCallIn) -> Result<Value, String> {
        let state = WEFT.lock().map_err(|_| "the plugin state is poisoned")?;
        let weft = state.as_ref().ok_or("cox_tool_call before cox_init")?;
        Ok(weft.call(&call))
    }

    cox_plugin_sdk::register!(init => init, tool_call => tool_call);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weft(config: Value) -> Weft {
        Weft::from_config(&config).unwrap().0
    }

    fn call(weft: &Weft, name: &str, input: Value) -> Value {
        weft.call(&ToolCallIn {
            name: name.into(),
            input,
        })
    }

    #[test]
    fn every_mcp_tool_but_render_is_a_read_only_spec() {
        let specs = weft(json!({})).specs();
        let names: Vec<&str> = specs.iter().filter_map(|s| s["name"].as_str()).collect();
        assert_eq!(names, tool_names());
        assert_eq!(names.len(), weft_binding::tools::decls::NAMES.len() - 1);
        assert!(!names.contains(&"weft_render"));
        for spec in &specs {
            assert_eq!(spec["risk"], "read_only");
            assert_eq!(spec["concurrency"], "exclusive");
            assert_eq!(spec["deferred"], true);
            assert_eq!(spec["input_schema"]["type"], "object");
            assert!(spec["description"].as_str().is_some_and(|d| !d.is_empty()));
        }
    }

    #[test]
    fn a_call_returns_the_mcp_text() {
        let weft = weft(Value::Null);
        let valid = call(
            &weft,
            "weft_validate",
            json!({ "markup": "<screen weft=\"0.1\" id=\"s\" label=\"S\"><text id=\"t\">Hi</text></screen>" }),
        );
        assert_eq!(valid["is_error"], false, "{valid}");
        let text: Value = serde_json::from_str(valid["text"].as_str().unwrap()).unwrap();
        assert_eq!(text["valid"], true, "{text}");

        let host = ToolHost::core(Default::default()).unwrap();
        let primer = call(&weft, "weft_primer", json!({}));
        let Outcome::Content { blocks, .. } = host.call("weft_primer", "{}") else {
            panic!("the primer is text");
        };
        assert_eq!(primer["text"], blocks.join("\n\n"));
    }

    #[test]
    fn bad_calls_are_results_the_model_can_fix() {
        let weft = weft(json!({}));
        let missing = call(&weft, "weft_validate", json!({}));
        assert_eq!(missing["is_error"], true);
        assert!(
            missing["text"]
                .as_str()
                .unwrap()
                .contains("markup is required")
        );
        for name in ["weft_render", "read"] {
            let unknown = call(&weft, name, json!({}));
            assert_eq!(unknown["is_error"], true);
            assert_eq!(unknown["text"], format!("Unknown tool: {name}."));
        }
    }

    #[test]
    fn the_config_table_is_the_project_and_its_settings() {
        let weft = weft(json!({
            "actions": ["save"],
            "validate": { "mode": "strict" },
            "mcp": { "context": "read-only", "limits": { "markupChars": 100 } },
        }));
        let capabilities = call(&weft, "weft_capabilities", json!({}));
        let text: Value = serde_json::from_str(capabilities["text"].as_str().unwrap()).unwrap();
        assert_eq!(text["actions"]["names"], json!(["save"]));
        let long = "x".repeat(101);
        let refused = call(&weft, "weft_validate", json!({ "markup": long }));
        assert!(
            refused["text"]
                .as_str()
                .unwrap()
                .contains("longer than 100")
        );
        let schema = &weft.specs()[0]["input_schema"];
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn a_broken_config_fails_init_with_its_diagnostics() {
        let Err(message) = Weft::from_config(&json!({ "catalog": 3 })) else {
            panic!("a catalog that is a number is an error");
        };
        assert!(
            message.starts_with("[plugins.weft] has errors:\n"),
            "{message}"
        );
        assert!(message.contains("\"code\""), "{message}");
    }
}
