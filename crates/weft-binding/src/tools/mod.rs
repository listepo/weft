//! The Weft agent tools (`weft_primer` … `weft_context`) as one Rust API that every host calls:
//! the MCP server of `packages/mcp` and the cox plugin of `plugins/cox`. A host lists `decls`,
//! passes a call's name and argument JSON to `ToolHost::call`, and sends back the `Outcome`, so
//! the texts, limits and checks a model meets are the same everywhere (plan.md T106).

mod args;
mod call;
pub mod decls;
pub mod host;
pub mod settings;

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Value as Json, json};
use weft_catalog::{
    CatalogError, CatalogSource, KindSource, ProjectOptions, core_catalog, load_project,
};
use weft_core::{Catalog, Diagnostic, Mode, compile_data_schema, has_errors, to_compact};

use crate::boundary::BindingError;

pub use host::{Context, Outcome, RenderRequest};
pub use settings::{ContextPolicy, Limits, Settings};

/// A host's context and settings, fixed when the host starts; calls never change them.
#[derive(Clone, Debug)]
pub struct ToolHost {
    context: Context,
    settings: Settings,
    /// The core catalog's name and version, which `weft_capabilities` always lists first.
    core: (String, String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireContext {
    catalog: Catalog,
    catalogs: Option<Vec<CatalogSource>>,
    kinds: Option<IndexMap<String, KindSource>>,
    tokens: Option<Vec<(String, String)>>,
    actions: Option<Vec<String>>,
    data: Option<Json>,
}

/// Why a host could not start.
#[derive(Debug, thiserror::Error)]
pub enum ToolHostError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// The host's project has errors; the diagnostics say where.
    #[error("the project has errors")]
    Project(Vec<Diagnostic>),
    #[error("the project's tool settings do not read: {0}")]
    Settings(serde_json::Error),
}

impl ToolHost {
    pub fn new(context: Context, settings: Settings) -> Result<ToolHost, CatalogError> {
        let core = core_catalog()?;
        Ok(ToolHost {
            context,
            settings,
            core: (core.name, core.version),
        })
    }

    /// A host with the core catalog only.
    pub fn core(settings: Settings) -> Result<ToolHost, CatalogError> {
        ToolHost::new(Context::of_catalog(core_catalog()?), settings)
    }

    /// A host started with a project (`weft-mcp --project`, a cox `[plugins.weft]` table): its
    /// resources, and the `validate.mode`, `mcp.limits` and `mcp.context` settings (SPEC §10.6).
    /// Members hold their files' JSON content, as in the tools' `project` argument. Returns the
    /// project's warnings beside the host.
    pub fn from_project(project: &Json) -> Result<(ToolHost, Vec<Diagnostic>), ToolHostError> {
        let loaded = load_project(
            project,
            &ProjectOptions {
                read: None,
                prefix: "#",
                mode: Mode::Lenient,
            },
        )?;
        if has_errors(&loaded.diagnostics) {
            return Err(ToolHostError::Project(loaded.diagnostics));
        }
        let settings = Settings::from_json(&Json::Object(loaded.project.settings.clone()))
            .map_err(ToolHostError::Settings)?;
        let host = ToolHost::new(Context::of_project(loaded.project), settings)?;
        Ok((host, loaded.diagnostics))
    }

    /// A host from a JavaScript server's context and settings, as JSON text: the context is
    /// `{catalog, catalogs?, kinds?, tokens?: [[path, type], …], actions?, data?: JSON Schema}`
    /// (entries for the tokens, so the order of the `Map` survives), the settings the `weft.json`
    /// tool sections.
    pub fn from_wire(context: &str, settings: &str) -> crate::Result<ToolHost> {
        let wire = |what| move |source| BindingError::Wire { what, source };
        let c: WireContext = serde_json::from_str(context).map_err(wire("tool host context"))?;
        let settings: Json = serde_json::from_str(settings).map_err(wire("tool settings"))?;
        let settings = Settings::from_json(&settings).map_err(wire("tool settings"))?;
        let context = Context {
            catalog: c.catalog,
            catalogs: c.catalogs,
            kinds: c.kinds,
            tokens: c.tokens.map(|t| t.into_iter().collect()),
            token_values: None,
            token_modifiers: Vec::new(),
            actions: c.actions,
            data: c.data.as_ref().map(|d| compile_data_schema(d).0),
        };
        ToolHost::new(context, settings)
            .map_err(|CatalogError::Embedded(e)| BindingError::Catalog(e))
    }

    /// What the TypeScript server exports beside the tools: `{limits, listed, contextNotice,
    /// primer}`, so it keeps no copy of them.
    pub fn constants(&self) -> String {
        to_compact(&json!({
            "limits": self.settings.limits,
            "listed": decls::LISTED,
            "contextNotice": decls::CONTEXT_NOTICE,
            "primer": decls::primer(&self.settings),
        }))
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Every tool's declaration, in listing order.
    pub fn decls(&self) -> Vec<Json> {
        decls::decls(&self.settings)
    }

    /// Runs one tool on its arguments' JSON text. Never fails: a bad call is a failure result
    /// the model can read and fix.
    pub fn call(&self, name: &str, args: &str) -> Outcome {
        call::call(self, name, args)
    }

    /// `call` for a JavaScript host, whose `JSON.stringify` gives nothing (`None`) for arguments
    /// that nest deeper than its stack, as the rest of the boundary treats such input.
    pub fn call_wire(&self, name: &str, args: Option<&str>) -> Outcome {
        match args {
            Some(args) => self.call(name, args),
            None => Outcome::failure(args::too_deep()),
        }
    }
}

#[cfg(test)]
mod tests;
