//! `weft-cox`: the Weft agent tools as a cox plugin (plan.md T106.3). The tools are the ones the
//! MCP server gives other hosts, from the same Rust (`weft_binding::tools`), so their texts, limits
//! and checks cannot drift between hosts. cox names them `wasm__weft__<tool>`.
//!
//! The decisions are plain functions so the tests run them on the host; the exports, which need
//! extism's memory, build only for `wasm32`.

use cox_plugin_sdk::{InitOut, ToolCallIn, Value};
use serde_json::json;
use weft_binding::tools::{Outcome, RenderRequest, ToolHost, ToolHostError};
use weft_catalog::{DEFAULT_TOKENS_JSON, load_tokens};
use weft_web::{HtmlOptions, to_html_with_data};

/// Appended to `weft_render`'s description: the page here is not the MCP server's, and an agent
/// comparing them should know why they differ.
const RENDER_PAGE: &str = "In this host the html page comes from Weft's static HTML generator, not the React reference renderer, so it can differ from the page other hosts return.";

/// The tools this plugin serves, in listing order: all of the MCP server's. `plugin.toml` grants
/// the same names.
pub fn tool_names() -> Vec<String> {
    weft_binding::tools::decls::NAMES
        .iter()
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
        self.host
            .decls()
            .into_iter()
            .map(|decl| {
                let mut description = decl["description"].as_str().unwrap_or_default().to_owned();
                if decl["name"] == "weft_render" {
                    description.push(' ');
                    description.push_str(RENDER_PAGE);
                }
                json!({
                    "name": decl["name"],
                    "description": description,
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
            Outcome::Render(request) => page(&request),
        }
    }
}

/// `weft_render` with `html`: the tree, then the static page, as the MCP server's two blocks. The
/// page is checked against the tokens it is drawn with, so markup that names a token those do not
/// define gets the tree and the page's diagnostics, as an error.
fn page(request: &RenderRequest) -> Value {
    // Without project tokens the markup's references went unchecked; the page uses the defaults,
    // as `toHtml` does.
    let defaults;
    let tokens = match &request.tokens {
        Some(tokens) => tokens,
        None => {
            defaults = serde_json::from_str(DEFAULT_TOKENS_JSON)
                .map(|json| load_tokens(&json).tokens)
                .unwrap_or_default();
            &defaults
        }
    };
    let options = HtmlOptions {
        catalog: &request.catalog,
        tokens,
        source: false,
        appearance: None,
        paths: false,
    };
    match to_html_with_data(&request.document, &options, request.data.as_ref()) {
        Ok(html) => output(&format!("{}\n\n{html}", request.tree), false),
        Err(invalid) => {
            let diagnostics = json!({ "diagnostics": invalid.0 }).to_string();
            let text = format!(
                "{}\n\nThe page could not be drawn: {diagnostics}",
                request.tree
            );
            output(&text, true)
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
    fn every_mcp_tool_is_a_read_only_spec() {
        let specs = weft(json!({})).specs();
        let names: Vec<&str> = specs.iter().filter_map(|s| s["name"].as_str()).collect();
        assert_eq!(names, tool_names());
        assert_eq!(names, weft_binding::tools::decls::NAMES);
        let render = &specs[names.iter().position(|n| *n == "weft_render").unwrap()];
        assert!(
            render["description"]
                .as_str()
                .unwrap()
                .ends_with(RENDER_PAGE)
        );
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
        for name in ["render", "read"] {
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

    const SCREEN: &str = "<screen weft=\"0.1\" id=\"s\" label=\"Demo\"><stack id=\"k\" gap=\"{token.brand.gap}\"><button id=\"go\" on-press=\"demo.go\" text=\"{$.cta}\"/></stack></screen>";
    const TREE: &str = "- main \"Demo\":\n  - button \"Go\"";

    #[test]
    fn render_gives_the_tree_and_with_html_the_static_page() {
        let project = json!({ "tokens": [{ "brand": { "gap": { "$type": "dimension", "$value": "8px" } } }] });
        let weft = weft(project);
        let data = json!({ "cta": "Go" });
        let tree = call(
            &weft,
            "weft_render",
            json!({ "markup": SCREEN, "data": data }),
        );
        assert_eq!(tree, output(TREE, false));

        let page = call(
            &weft,
            "weft_render",
            json!({ "markup": SCREEN, "data": data, "html": true }),
        );
        assert_eq!(page["is_error"], false, "{page}");
        let text = page["text"].as_str().unwrap();
        let html = text.strip_prefix(&format!("{TREE}\n\n")).unwrap();
        assert!(html.starts_with("<!doctype html>"), "{html}");
        assert!(html.contains(">Go</button>"), "{html}");
        assert!(html.contains("8px"), "{html}");
    }

    #[test]
    fn a_page_the_default_tokens_cannot_draw_is_the_tree_and_its_diagnostics() {
        // The core host checks no token references, so the markup passes and the page refuses it.
        let page = call(
            &weft(json!({})),
            "weft_render",
            json!({ "markup": SCREEN, "data": { "cta": "Go" }, "html": true }),
        );
        assert_eq!(page["is_error"], true, "{page}");
        let text = page["text"].as_str().unwrap();
        let rest = text
            .strip_prefix(&format!("{TREE}\n\nThe page could not be drawn: "))
            .unwrap();
        let report: Value = serde_json::from_str(rest).unwrap();
        assert!(report["diagnostics"][0]["code"].is_string(), "{report}");
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
