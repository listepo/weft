//! The Weft agent tools (`weft_primer` … `weft_context`) as one Rust API that every host calls:
//! the MCP server of `packages/mcp` and the cox plugin of `plugins/cox`. A host lists `decls`,
//! passes a call's name and argument JSON to `ToolHost::call`, and sends back the `Outcome`, so
//! the texts, limits and checks a model meets are the same everywhere (plan.md T106).

mod args;
mod call;
pub mod decls;
pub mod host;
pub mod settings;

use serde_json::Value as Json;
use weft_catalog::{CatalogError, ProjectOptions, core_catalog, load_project};
use weft_core::{Diagnostic, Mode, has_errors};

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
    pub fn new(context: Context, settings: Settings) -> Result<ToolHost, ToolHostError> {
        let core = core_catalog()?;
        Ok(ToolHost {
            context,
            settings,
            core: (core.name, core.version),
        })
    }

    /// A host with the core catalog only.
    pub fn core(settings: Settings) -> Result<ToolHost, ToolHostError> {
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
}

#[cfg(test)]
mod tests;
