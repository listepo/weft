//! `weft html`, `weft react`, `weft solid` and their importers `weft import-html`,
//! `weft import-react`, `weft import-solid` (SPEC §9). Catalog and tokens resolve as for SwiftUI;
//! `--typescript` and `--source` fall back to the project's `export.<target>` settings, the output
//! directory to `export.<target>.outDir` or `import.<target>.outDir`, else standard output.
//! `weft html --data` falls back to `export.html.data`; without data the page is a template.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use weft_web::{
    Framework, HtmlOptions, ImportOptions, JsxOptions, import_html, import_jsx, to_html_with_data,
    to_jsx,
};

use crate::convert::{
    catalog, emit, finish_import, out_dir, project, sample_data, strict_document, switch, tokens,
};
use crate::{DIAGNOSTICS, ProjectArgs, print, read};

#[derive(Clone, Copy)]
pub enum Target {
    Html,
    React,
    Solid,
}

impl Target {
    fn name(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::React => "react",
            Self::Solid => "solid",
        }
    }
}

pub struct ExportArgs {
    pub target: Target,
    pub file: PathBuf,
    pub catalog: Option<PathBuf>,
    pub tokens: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    pub typescript: bool,
    pub javascript: bool,
    pub source: bool,
    pub no_source: bool,
    /// Sample data for the static page; the component targets take data at run time.
    pub data: Option<PathBuf>,
}

pub fn export(args: ExportArgs, out: &mut dyn Write) -> Result<u8> {
    let target = args.target.name();
    let (project, project_dir) = project(&args.file, args.project)?;
    let project = project.as_ref();
    let catalog = catalog(args.catalog.as_deref(), project)?;
    let Some(document) = strict_document(&args.file, &catalog)? else {
        return Ok(DIAGNOSTICS);
    };
    let source = switch(
        args.source,
        args.no_source,
        project,
        &["export", target, "source"],
    );
    let (text, extension) = match args.target {
        Target::Html => {
            let tokens = tokens(args.tokens.as_deref(), project)?;
            let data = sample_data(args.data, project, project_dir.as_deref(), target)?;
            let options = HtmlOptions {
                catalog: &catalog,
                tokens: &tokens.tokens,
                source,
            };
            match to_html_with_data(&document, &options, data.as_ref()) {
                Ok(html) => (html, "html"),
                // The tokens can still refuse a screen the catalog alone accepts.
                Err(invalid) => {
                    print(&args.file, &invalid.0, &mut std::io::stderr())?;
                    return Ok(DIAGNOSTICS);
                }
            }
        }
        Target::React | Target::Solid => {
            let typescript = switch(
                args.typescript,
                args.javascript,
                project,
                &["export", target, "typescript"],
            );
            let json = serde_json::to_value(&document)?;
            let options = JsxOptions {
                catalog: &catalog,
                component_name: None,
                framework: if matches!(args.target, Target::Solid) {
                    Framework::Solid
                } else {
                    Framework::React
                },
                typescript,
                source,
            };
            // Only a component name can be refused, and the default one is valid.
            let code = to_jsx(&json, &options)
                .ok()
                .context("the generator refused its default component name")?;
            (code, if typescript { "tsx" } else { "jsx" })
        }
    };
    let dir = out_dir(
        args.out_dir,
        project,
        project_dir.as_deref(),
        "export",
        target,
    );
    emit(&text, &args.file, dir, extension, out)?;
    Ok(0)
}

pub struct ImportArgs {
    pub target: Target,
    pub file: PathBuf,
    pub catalog: Option<PathBuf>,
    pub tokens: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
}

/// TypeScript by extension: `.tsx`, `.ts`, `.mts` and `.cts` hold types the JSX grammar refuses.
fn is_typescript(file: &Path) -> bool {
    file.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "tsx" | "ts" | "mts" | "cts"
        )
    })
}

pub fn import(args: ImportArgs, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let project = project.as_ref();
    let catalog = catalog(args.catalog.as_deref(), project)?;
    let tokens = tokens(args.tokens.as_deref(), project)?;
    let text = read(&args.file)?;
    let options = ImportOptions {
        catalog: &catalog,
        tokens: &tokens.tokens,
    };
    let result = match args.target {
        Target::Html => import_html(&text, &options),
        Target::React | Target::Solid => import_jsx(&text, is_typescript(&args.file), &options),
    };
    let dir = out_dir(
        args.out_dir,
        project,
        project_dir.as_deref(),
        "import",
        args.target.name(),
    );
    finish_import(&args.file, &result, dir, out)
}
