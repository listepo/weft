//! `weft validate`, `weft fmt`, `weft explain`, and the generators and importers of SPEC §9:
//! SwiftUI (`weft swiftui`, `weft import-swiftui`, in `swiftui.rs`), Slint (`weft slint`,
//! `weft import-slint`, in `slint.rs`) and the web targets (`weft
//! html|react|solid`, `weft import-html|import-react|import-solid`, in `web.rs`), and `weft
//! import-cem`, a catalog from a Custom Elements Manifest (in `cem.rs`), and `weft schema`, the
//! JSON Schema of the documents a catalog admits (in `schema.rs`). The project file (`weft.json`, SPEC §10) found
//! above the document, or given with `--project`, supplies the catalog, tokens, actions and data
//! schema, and the settings of §10.6 (`validate.mode`, `format.write`); a flag overrides the
//! project, which overrides the default. `--catalog` replaces the project's catalog. Without
//! either, `validate` checks the syntax and shape layers. Exit codes: 0 ok, 1 diagnostics with errors,
//! 2 usage or I/O failure.

mod a2ui;
mod cem;
mod convert;
mod schema;
mod slint;
mod swiftui;
mod version_check;
mod web;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use weft_catalog::{
    CORE_CATALOG_JSON, PROJECT_FILE, Project, ProjectOptions, core_catalog, load_project_text,
    merge_catalogs, split_package_read, token_types,
};
use weft_core::{
    Catalog, DataCheckOptions, Diagnostic, Document, Mode, ParseOptions, ValidateOptions,
    check_data, check_data_json, explain, explain_changes, explain_with_context, has_errors, parse,
    parse_json, serialize, validate,
};

#[derive(Parser)]
#[command(
    name = "weft",
    version,
    about = "Validate, format and explain Weft documents, and convert them to and from SwiftUI, HTML, React, SolidJS, Slint and A2UI"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Which project file applies (SPEC §10.1).
#[derive(Args)]
struct ProjectArgs {
    /// Project file; without it, the nearest `weft.json` above the document is used.
    #[arg(long, conflicts_with = "no_project")]
    project: Option<PathBuf>,
    /// Ignore any project file.
    #[arg(long)]
    no_project: bool,
}

/// A generator of a web target.
#[derive(Args)]
struct WebExport {
    file: PathBuf,
    /// Catalog JSON, repeatable; the files replace the project's catalogs, in order (default: the
    /// core catalog).
    #[arg(long)]
    catalog: Vec<PathBuf>,
    #[command(flatten)]
    project: ProjectArgs,
    /// Write `<file stem>.<extension>` here instead of printing (default: the project's
    /// `export.<target>.outDir`, else print).
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Keep the canonical screen in a leading comment, so the importer gives it back exactly
    /// (default: the project's `export.<target>.source`, else off).
    #[arg(long, conflicts_with = "no_source")]
    source: bool,
    /// Leave the source comment out, whatever the project says.
    #[arg(long)]
    no_source: bool,
    /// Whether the source comment carries the screen's context (default: the project's
    /// `export.<target>.context`, else `strip` for HTML and `keep` for the others).
    #[arg(long, value_enum)]
    context: Option<convert::ExportContext>,
    /// Overwrite the output file when it already exists.
    #[arg(long)]
    force: bool,
}

/// Sample data a generator shows (SPEC §9).
#[derive(Args)]
struct SampleData {
    /// JSON the bindings read, shown in the output (default: the project's
    /// `export.<target>.data`, else none).
    #[arg(long)]
    data: Option<PathBuf>,
}

/// What the React and SolidJS generators add.
#[derive(Args)]
struct JsxExport {
    #[command(flatten)]
    common: WebExport,
    /// TSX with typed props (default: the project's `export.<target>.typescript`, else JSX).
    #[arg(long, conflicts_with = "javascript")]
    typescript: bool,
    /// JSX, whatever the project says.
    #[arg(long)]
    javascript: bool,
}

/// An importer of a web target.
#[derive(Args)]
struct WebImport {
    file: PathBuf,
    /// Catalog JSON, repeatable; the files replace the project's catalogs, in order (default: the
    /// core catalog).
    #[arg(long)]
    catalog: Vec<PathBuf>,
    /// Token JSON or a DTCG resolver the page's custom properties are matched against; replaces the project's tokens
    /// (default: the default tokens).
    #[arg(long)]
    tokens: Option<PathBuf>,
    #[command(flatten)]
    project: ProjectArgs,
    /// Write `<file stem>.weft` here instead of printing (default: the project's
    /// `import.<target>.outDir`, else print).
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Whether context read back from a generated file enters the document (default: the
    /// project's `import.<target>.context`, else `keep`).
    #[arg(long, value_enum)]
    context: Option<convert::ImportContext>,
    /// Overwrite the output file when it already exists.
    #[arg(long)]
    force: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Check a document: markup, or canonical JSON when the file name ends in `.json`.
    Validate {
        file: PathBuf,
        /// Catalog JSON, repeatable; the files replace the project's catalogs, in order.
        #[arg(long)]
        catalog: Vec<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Report unknown content as errors instead of warnings (default: the project's
        /// `validate.mode`, else lenient).
        #[arg(long, conflicts_with = "lenient")]
        strict: bool,
        /// Report unknown content as warnings, whatever the project says.
        #[arg(long)]
        lenient: bool,
    },
    /// Print the canonical markup of a document.
    Fmt {
        file: PathBuf,
        /// Rewrite the file instead of printing; untouched when already canonical (default: the
        /// project's `format.write`, else print).
        #[arg(long, conflicts_with = "print")]
        write: bool,
        /// Print, whatever the project says.
        #[arg(long)]
        print: bool,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Read back what each binding, token, event and loop of a markup document means, one per
    /// line, so the meaning can be compared with the instruction behind an edit.
    Explain {
        file: PathBuf,
        /// An older version of the document: print only what was added, removed or changed.
        #[arg(long)]
        against: Option<PathBuf>,
        /// Catalog JSON, repeatable; the files replace the project's catalogs, in order. Without
        /// either, boolean and writable props read as plain reads.
        #[arg(long)]
        catalog: Vec<PathBuf>,
        /// Also list each element's context entries after its readbacks (default: the project's
        /// `explain.context`, else off). `--against` always lists context changes.
        #[arg(long)]
        context: bool,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Compare two versions of one screen, fragment or library catalog and say how far its
    /// version must be raised.
    VersionCheck {
        old: PathBuf,
        new: PathBuf,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Generate a SwiftUI view (iOS 17, macOS 14) from a markup document.
    Swiftui {
        file: PathBuf,
        /// Catalog JSON, repeatable; the files replace the project's catalogs, in order (default: the
        /// core catalog).
        #[arg(long)]
        catalog: Vec<PathBuf>,
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.swift` here instead of printing (default: the project's
        /// `export.swiftui.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[command(flatten)]
        data: SampleData,
        /// Read the tokens from the shared `WeftTokens` that `weft swiftui-tokens` writes
        /// (default: the project's `export.swiftui.sharedTokens`, which is true; without a project,
        /// the screen carries its own tokens).
        #[arg(long, conflicts_with = "no_shared_tokens")]
        shared_tokens: bool,
        /// Put the tokens the screen uses in the screen file, so it builds on its own.
        #[arg(long)]
        no_shared_tokens: bool,
        /// Whether the source comment carries the screen's context (default: the project's
        /// `export.swiftui.context`, else `keep`).
        #[arg(long, value_enum)]
        context: Option<convert::ExportContext>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Generate `WeftTokens.swift`, the design tokens every SwiftUI screen of a project shares.
    SwiftuiTokens {
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        /// Project file; without it, the nearest `weft.json` in or above the working directory.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Write `WeftTokens.swift` here instead of printing (default: the project's
        /// `export.swiftui.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Read a SwiftUI view back into markup; what Weft cannot hold is listed on stderr as losses.
    ImportSwiftui {
        file: PathBuf,
        /// Catalog JSON, repeatable; the files replace the project's catalogs, in order (default: the
        /// core catalog).
        #[arg(long)]
        catalog: Vec<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.weft` here instead of printing (default: the project's
        /// `import.swiftui.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Whether context read back from the source comment enters the document (default: the
        /// project's `import.swiftui.context`, else `keep`).
        #[arg(long, value_enum)]
        context: Option<convert::ImportContext>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Generate a Slint component (Slint 1.x, `std-widgets.slint`) from a markup document.
    Slint {
        file: PathBuf,
        /// Catalog JSON, repeatable; the files replace the project's catalogs, in order (default: the
        /// core catalog).
        #[arg(long)]
        catalog: Vec<PathBuf>,
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        /// Component name (`<Name>Screen`); the screen id when absent.
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.slint` here instead of printing (default: the project's
        /// `export.slint.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Whether the source comment carries the screen's context (default: the project's
        /// `export.slint.context`, else `keep`).
        #[arg(long, value_enum)]
        context: Option<convert::ExportContext>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Read a generated Slint component back into markup.
    ImportSlint {
        file: PathBuf,
        /// Catalog JSON, repeatable; the files replace the project's catalogs, in order (default: the
        /// core catalog).
        #[arg(long)]
        catalog: Vec<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.weft` here instead of printing (default: the project's
        /// `import.slint.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Whether context read back from the source comment enters the document (default: the
        /// project's `import.slint.context`, else `keep`).
        #[arg(long, value_enum)]
        context: Option<convert::ImportContext>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Generate a static HTML page with CSS and no script from a markup document.
    Html {
        #[command(flatten)]
        common: WebExport,
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        #[command(flatten)]
        data: SampleData,
    },
    /// Generate a React component (JSX or TSX) from a markup document.
    React(JsxExport),
    /// Generate a SolidJS component (JSX or TSX) from a markup document.
    Solid(JsxExport),
    /// Generate a Lit web component (JavaScript) from a markup document.
    Lit(WebExport),
    /// Generate `weft-tokens.css`, the design tokens React, SolidJS and Lit components read as CSS
    /// custom properties.
    ///
    /// With light and dark themes (a DTCG resolver), the dark values apply under
    /// `prefers-color-scheme: dark`.
    CssTokens {
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        /// Project file; without it, the nearest `weft.json` in or above the working directory.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Write `weft-tokens.css` here instead of printing (default: the project's
        /// `export.css.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Generate `weft-base.css`, the base stylesheet React, SolidJS and Lit components share with the
    /// static page.
    ///
    /// Its rules read the custom properties of `weft css-tokens` and fall back to the default
    /// tokens' values, so link it after `weft-tokens.css` (or alone).
    CssBase {
        /// Project file; without it, the nearest `weft.json` in or above the working directory.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Write `weft-base.css` here instead of printing (default: the project's
        /// `export.css.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Read an HTML page back into markup; what Weft cannot hold is listed on stderr as losses.
    ImportHtml(WebImport),
    /// Read a React component (.jsx, or .tsx as TypeScript) back into markup; losses go to stderr.
    ImportReact(WebImport),
    /// Read a SolidJS component (.jsx, or .tsx as TypeScript) back into markup; losses go to
    /// stderr.
    ImportSolid(WebImport),
    /// Write a markup document as A2UI v0.9 messages (basic catalog); what A2UI cannot hold is
    /// listed on stderr as losses.
    A2ui(A2uiArgs),
    /// Read A2UI v0.9 messages (a JSON array, one object or JSON Lines) back into markup; losses
    /// go to stderr.
    ImportA2ui(A2uiArgs),
    /// Turn a Custom Elements Manifest into a catalog that extends the project's; losses and
    /// diagnostics go to stderr.
    ImportCem {
        /// The manifest (`custom-elements.json`, schema 2.x), at most 10 MB.
        file: PathBuf,
        /// The catalog's name (default: the project's `import.cem.name`, else the file stem).
        #[arg(long)]
        name: Option<String>,
        /// The catalog's version (default: the project's `import.cem.version`, else 0.0.0).
        #[arg(long)]
        version: Option<String>,
        /// The kinds the catalog owns, `<prefix>-…`, so it can sit beside a project catalog
        /// (default: the project's `import.cem.prefix`, else none).
        #[arg(long)]
        prefix: Option<String>,
        /// Catalog JSON the result extends, repeatable; the files replace the project's catalogs,
        /// in order (default: the core catalog). Their kinds are left out of the result.
        #[arg(long)]
        catalog: Vec<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.catalog.json` here instead of printing (default: the project's
        /// `import.cem.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Print the JSON Schema (2020-12) of the canonical JSON documents the project's catalog
    /// admits, for a model whose decoder takes a schema (SPEC §3.1).
    Schema {
        /// Catalog JSON, repeatable; the files replace the project's catalogs, in order (default: the
        /// core catalog).
        #[arg(long)]
        catalog: Vec<PathBuf>,
        /// Project file; without it, the nearest `weft.json` in or above the working directory.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Write `document.schema.json` here instead of printing (default: the project's
        /// `export.schema.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
}

/// A conversion to or from A2UI.
#[derive(Args)]
struct A2uiArgs {
    file: PathBuf,
    /// Catalog JSON, repeatable; the files replace the project's catalogs, in order (default: the
    /// core catalog).
    #[arg(long)]
    catalog: Vec<PathBuf>,
    #[command(flatten)]
    project: ProjectArgs,
    /// Write `<file stem>.a2ui.json` (or `.weft` when importing) here instead of printing
    /// (default: the project's `export.a2ui.outDir` or `import.a2ui.outDir`, else print).
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Overwrite the output file when it already exists.
    #[arg(long)]
    force: bool,
}

impl From<A2uiArgs> for a2ui::Args {
    fn from(a: A2uiArgs) -> Self {
        a2ui::Args {
            file: a.file,
            catalog: a.catalog,
            project: a.project,
            out_dir: a.out_dir,
            force: a.force,
        }
    }
}

const USAGE_ERROR: u8 = 2;
/// The core catalog's name; a `--catalog` file with it replaces the core (SPEC §10.2).
const CORE: &str = "weft-core";
const DIAGNOSTICS: u8 = 1;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(if e.use_stderr() { USAGE_ERROR } else { 0 });
        }
    };
    match run(cli.command, &mut std::io::stdout().lock()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("weft: {e:#}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

fn read(file: &Path) -> Result<String> {
    std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))
}

/// Writes `contents` by renaming a sibling temp file into the file `path` names, so a crash cannot
/// leave a half-written target (SPEC §10 tools that rewrite a file). A symbolic link is followed
/// to its target and left in place; the target's permissions are kept.
fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let target = if path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        std::fs::canonicalize(path)
            .with_context(|| format!("cannot resolve {}", path.display()))?
    } else {
        path.to_path_buf()
    };
    let permissions = std::fs::metadata(&target)
        .with_context(|| format!("cannot stat {}", target.display()))?
        .permissions();
    let name = target.file_name().unwrap_or_default();
    let tmp = target.with_file_name(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let written = (|| {
        std::fs::write(&tmp, contents)?;
        std::fs::set_permissions(&tmp, permissions)?;
        std::fs::rename(&tmp, &target)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written.with_context(|| format!("cannot write {}", path.display()))
}

/// SPEC §10.2: after the name is accepted, the resolved path (symbolic links followed) must stay
/// inside `dir`. Anything else is unreadable (`W704`).
pub(crate) fn read_project_member(dir: &Path, name: &str) -> Option<String> {
    let root = std::fs::canonicalize(dir).ok()?;
    let resolved = std::fs::canonicalize(root.join(name)).ok()?;
    resolved
        .ancestors()
        .any(|p| p == root)
        .then(|| std::fs::read_to_string(resolved).ok())
        .flatten()
}

/// SPEC §10.2 packages: the package is `node_modules/<package>` of the project directory or of the
/// nearest parent that has it, as Node resolves one, and only files whose resolved path stays
/// inside that package directory are read. Nothing is run and nothing is fetched.
fn read_package_file(project_dir: &Path, package: &str, file: &str) -> Option<String> {
    let start = if project_dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        project_dir
    };
    let package_dir = std::path::absolute(start)
        .ok()?
        .ancestors()
        .map(|dir| dir.join("node_modules").join(package))
        .find(|dir| dir.join("package.json").is_file())?;
    read_project_member(&package_dir, file)
}

/// A file the project names: one of its own, or one of an npm package the loader asks for.
fn read_member(dir: &Path, name: &str) -> Option<String> {
    match split_package_read(name) {
        Some((package, file)) => read_package_file(dir, package, file),
        None => read_project_member(dir, name),
    }
}

/// SPEC §10.1: the first `weft.json` in the document's directory or one above it.
fn find_project(document: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(document).ok()?;
    absolute
        .ancestors()
        .skip(1)
        .map(|dir| dir.join(PROJECT_FILE))
        .find(|candidate| candidate.is_file())
}

/// The project file that applies to `document`: the one given, else the nearest one.
fn project_file(document: &Path, args: ProjectArgs) -> Option<PathBuf> {
    if args.no_project {
        None
    } else {
        args.project.or_else(|| find_project(document))
    }
}

/// The project that applies to `document`, if any, after printing its diagnostics to `out`.
/// Returns whether those diagnostics hold an error too.
fn project_for(
    document: &Path,
    args: ProjectArgs,
    mode: Mode,
    out: &mut dyn Write,
) -> Result<(Option<Project>, bool)> {
    let Some(file) = project_file(document, args) else {
        return Ok((None, false));
    };
    let (project, diagnostics) = load_project(&file, mode)?;
    print_project(&file, &diagnostics, out)?;
    Ok((Some(project), has_errors(&diagnostics)))
}

/// Project diagnostics. A markup problem in a fragment (SPEC §10.7) has its position in the
/// fragment's file, so it is shown in that file.
fn print_project(file: &Path, diagnostics: &[Diagnostic], out: &mut dyn Write) -> Result<()> {
    let members = std::fs::read_to_string(file)
        .ok()
        .and_then(|text| parse_json(&text).ok());
    let dir = file.parent().unwrap_or(Path::new("."));
    for d in diagnostics {
        let shown = (d.line.and(d.path.strip_prefix("#/fragments/")))
            .and_then(|rest| rest.split('/').next())
            .map(|name| name.replace("~1", "/").replace("~0", "~"))
            .and_then(|name| members.as_ref()?.get("fragments")?.get(&name)?.as_str())
            .map(|name| dir.join(name));
        print(
            shown.as_deref().unwrap_or(file),
            std::slice::from_ref(d),
            out,
        )?;
    }
    Ok(())
}

/// A setting of the project's §10.6 sections, when there is a project and it sets one.
fn setting<'a>(project: Option<&'a Project>, path: &[&str]) -> Option<&'a serde_json::Value> {
    project.and_then(|p| p.setting(path))
}

/// The project and its diagnostics. Files it names are read relative to its directory; the
/// loader has already refused names that leave it.
fn load_project(file: &Path, mode: Mode) -> Result<(Project, Vec<Diagnostic>)> {
    let text = read(file)?;
    let dir = file.parent().unwrap_or(Path::new("."));
    let read_member = |name: &str| read_member(dir, name);
    let loaded = load_project_text(
        &text,
        &ProjectOptions {
            read: Some(&read_member),
            mode,
            ..Default::default()
        },
    )
    .context("the embedded core catalog is broken")?;
    Ok((loaded.project, loaded.diagnostics))
}

/// The catalogs of a repeated `--catalog` (SPEC §10.2), or `None` without one: they replace the
/// project's whole list, in order, each merged over the core, and a file named `weft-core`
/// replaces the core as the base, so a whole catalog still loads alone. Their problems go to
/// stderr, each in its own file; an error stops the command, as an unreadable file does.
fn load_catalogs(files: &[PathBuf]) -> Result<Option<Catalog>> {
    if files.is_empty() {
        return Ok(None);
    }
    let mut base = None;
    let mut listed: Vec<(&Path, serde_json::Value)> = Vec::new();
    for file in files {
        let text = read(file)?;
        let json =
            parse_json(&text).with_context(|| format!("catalog {} is not JSON", file.display()))?;
        if json.get("name").and_then(serde_json::Value::as_str) == Some(CORE) {
            let whole = serde_json::from_value::<Catalog>(json.clone())
                .with_context(|| format!("catalog {} is not a Weft catalog", file.display()))?;
            base = Some((whole, json));
        } else {
            listed.push((file, json));
        }
    }
    let base = match base {
        // Alone, a whole catalog is used as it is, as `--catalog` always did.
        Some((whole, _)) if listed.is_empty() => return Ok(Some(whole)),
        Some((_, json)) => json,
        None => parse_json(CORE_CATALOG_JSON).context("the embedded core catalog is broken")?,
    };
    let contents: Vec<serde_json::Value> = listed.iter().map(|(_, json)| json.clone()).collect();
    let loaded = merge_catalogs(&base, &contents, Mode::Lenient)
        .context("the embedded core catalog is broken")?;
    for d in &loaded.diagnostics {
        // `#/catalog/<i>/…` points into the i-th listed file, as `#/…`.
        let rest = d.path.strip_prefix("#/catalog/").unwrap_or_default();
        let (index, inner) = rest.split_once('/').unwrap_or((rest, ""));
        let mut shown = d.clone();
        let file = match index.parse::<usize>().ok().and_then(|i| listed.get(i)) {
            Some((file, _)) => {
                shown.path = format!("#/{inner}").trim_end_matches('/').to_owned();
                *file
            }
            None => files[0].as_path(),
        };
        print(file, &[shown], &mut std::io::stderr())?;
    }
    if has_errors(&loaded.diagnostics) {
        bail!("the catalogs given with --catalog have errors");
    }
    Ok(Some(loaded.project.catalog))
}

/// I/O and usage failures are errors; documents with diagnostics are an exit code.
fn run(command: Command, out: &mut dyn Write) -> Result<u8> {
    match command {
        Command::Fmt {
            file,
            write,
            print: print_only,
            project,
        } => {
            // Project problems go to stderr: stdout carries the formatted document.
            let (project, _) = project_for(&file, project, Mode::Lenient, &mut std::io::stderr())?;
            let write = write
                || (!print_only
                    && setting(project.as_ref(), &["format", "write"])
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false));
            let text = read(&file)?;
            let result = parse(&text, &ParseOptions::default());
            let Some(document) = result.document else {
                print(&file, &result.diagnostics, out)?;
                return Ok(DIAGNOSTICS);
            };
            let formatted = serialize(&document);
            if !write {
                out.write_all(formatted.as_bytes())?;
            } else if formatted != text {
                write_atomic(&file, &formatted)?;
            }
            Ok(0)
        }
        Command::Validate {
            file,
            catalog,
            project,
            strict,
            lenient,
        } => {
            let text = read(&file)?;
            let flag = if strict {
                Some(Mode::Strict)
            } else if lenient {
                Some(Mode::Lenient)
            } else {
                None
            };
            let (project, project_failed) =
                project_for(&file, project, flag.unwrap_or_default(), out)?;
            let mode = flag.unwrap_or(
                match setting(project.as_ref(), &["validate", "mode"])
                    .and_then(serde_json::Value::as_str)
                {
                    Some("strict") => Mode::Strict,
                    _ => Mode::Lenient,
                },
            );
            let explicit = load_catalogs(&catalog)?;
            let catalog = explicit.or_else(|| project.as_ref().map(|p| p.catalog.clone()));
            if catalog.is_none() {
                eprintln!(
                    "weft: no --catalog and no project; only the syntax and shape layers were checked"
                );
            }
            let tokens = project
                .as_ref()
                .and_then(|p| p.tokens.as_ref())
                .map(token_types);
            let actions = project.as_ref().and_then(|p| p.actions.as_deref());
            let data = project.as_ref().and_then(|p| p.data.as_ref());
            let diagnostics = if file.extension().is_some_and(|e| e == "json") {
                let json = match parse_json(&text) {
                    Ok(json) => json,
                    Err(e) => {
                        eprintln!("{}: {e}", file.display());
                        return Ok(DIAGNOSTICS);
                    }
                };
                match &catalog {
                    // Without a catalog the library still checks the canonical
                    // JSON shape (W200): `--no-project broken.json` must not
                    // exit 0 where the same document through the library, the
                    // MCP server or the markup path gets diagnostics (T63).
                    None => validate(
                        &json,
                        &ValidateOptions {
                            catalog: None,
                            mode,
                            tokens: tokens.as_ref(),
                            actions,
                        },
                    ),
                    Some(catalog) => {
                        let mut found = validate(
                            &json,
                            &ValidateOptions {
                                catalog: Some(catalog),
                                mode,
                                tokens: tokens.as_ref(),
                                actions,
                            },
                        );
                        if let Some(data) = data {
                            found.extend(check_data_json(
                                &json,
                                &DataCheckOptions { catalog, data },
                            ));
                        }
                        found
                    }
                }
            } else {
                let result = parse(
                    &text,
                    &ParseOptions {
                        catalog: catalog.as_ref(),
                        mode,
                        tokens: tokens.as_ref(),
                        actions,
                    },
                );
                let mut found = result.diagnostics;
                if let (Some(document), Some(catalog), Some(data)) =
                    (&result.document, &catalog, data)
                {
                    found.extend(check_data(document, &DataCheckOptions { catalog, data }));
                }
                found
            };
            print(&file, &diagnostics, out)?;
            Ok(if has_errors(&diagnostics) || project_failed {
                DIAGNOSTICS
            } else {
                0
            })
        }
        Command::VersionCheck { old, new, project } => {
            let (loaded, _) = project_for(&new, project, Mode::Lenient, &mut std::io::stderr())?;
            let catalog = match loaded {
                Some(project) => project.catalog,
                None => core_catalog()?,
            };
            version_check::run(&old, &new, &catalog, out)
        }
        Command::Explain {
            file,
            against,
            catalog,
            context,
            project,
        } => {
            // Project problems go to stderr: stdout carries the read-back.
            let (project, _) = project_for(&file, project, Mode::Lenient, &mut std::io::stderr())?;
            let context = context
                || setting(project.as_ref(), &["explain", "context"])
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
            let explicit = load_catalogs(&catalog)?;
            let catalog = explicit.or_else(|| project.map(|p| p.catalog));
            if catalog.is_none() {
                eprintln!(
                    "weft: no --catalog and no project; boolean and writable props read as plain reads"
                );
            }
            let catalog = catalog.as_ref();
            let Some(document) = load_markup(&file, catalog, out)? else {
                return Ok(DIAGNOSTICS);
            };
            let Some(against) = against else {
                let readbacks = if context {
                    explain_with_context(&document, catalog)
                } else {
                    explain(&document, catalog)
                };
                for readback in readbacks {
                    writeln!(out, "{readback}")?;
                }
                return Ok(0);
            };
            let Some(before) = load_markup(&against, catalog, out)? else {
                return Ok(DIAGNOSTICS);
            };
            let changes = explain_changes(&before, &document, catalog);
            if changes.is_empty() {
                eprintln!("weft: no prop, event, loop or context entry changed");
            }
            for change in changes {
                writeln!(out, "{change}")?;
            }
            Ok(0)
        }
        Command::Swiftui {
            file,
            catalog,
            tokens,
            project,
            out_dir,
            data,
            shared_tokens,
            no_shared_tokens,
            context,
            force,
        } => swiftui::export(
            swiftui::ExportArgs {
                file,
                catalog,
                tokens,
                project,
                out_dir,
                data: data.data,
                shared_tokens: (shared_tokens, no_shared_tokens),
                context: context.map(|c| c == convert::ExportContext::Keep),
                force,
            },
            out,
        ),
        Command::SwiftuiTokens {
            tokens,
            project,
            no_project,
            out_dir,
            force,
        } => swiftui::export_tokens(
            swiftui::TokensArgs {
                tokens,
                project: ProjectArgs {
                    project,
                    no_project,
                },
                out_dir,
                force,
            },
            out,
        ),
        Command::CssTokens {
            tokens,
            project,
            no_project,
            out_dir,
            force,
        } => web::export_css(
            swiftui::TokensArgs {
                tokens,
                project: ProjectArgs {
                    project,
                    no_project,
                },
                out_dir,
                force,
            },
            out,
        ),
        Command::CssBase {
            project,
            no_project,
            out_dir,
            force,
        } => web::export_css_base(
            ProjectArgs {
                project,
                no_project,
            },
            out_dir,
            force,
            out,
        ),
        Command::ImportSwiftui {
            file,
            catalog,
            project,
            out_dir,
            context,
            force,
        } => swiftui::import(
            swiftui::ImportArgs {
                file,
                catalog,
                project,
                out_dir,
                context: context.map(|c| c == convert::ImportContext::Keep),
                force,
            },
            out,
        ),
        Command::Slint {
            file,
            catalog,
            tokens,
            name,
            project,
            out_dir,
            context,
            force,
        } => slint::export(
            slint::ExportArgs {
                file,
                catalog,
                tokens,
                name,
                project,
                out_dir,
                context: context.map(|c| c == convert::ExportContext::Keep),
                force,
            },
            out,
        ),
        Command::ImportSlint {
            file,
            catalog,
            project,
            out_dir,
            context,
            force,
        } => slint::import(
            slint::ImportArgs {
                file,
                catalog,
                project,
                out_dir,
                context: context.map(|c| c == convert::ImportContext::Keep),
                force,
            },
            out,
        ),
        Command::Html {
            common,
            tokens,
            data,
        } => web_export(web::Target::Html, common, tokens, data.data, None, out),
        Command::React(args) => {
            let target = web::Target::React;
            web_export(
                target,
                args.common,
                None,
                None,
                Some((args.typescript, args.javascript)),
                out,
            )
        }
        Command::Solid(args) => {
            let target = web::Target::Solid;
            web_export(
                target,
                args.common,
                None,
                None,
                Some((args.typescript, args.javascript)),
                out,
            )
        }
        Command::Lit(common) => web_export(web::Target::Lit, common, None, None, None, out),
        Command::ImportHtml(args) => web_import(web::Target::Html, args, out),
        Command::ImportReact(args) => web_import(web::Target::React, args, out),
        Command::ImportSolid(args) => web_import(web::Target::Solid, args, out),
        Command::A2ui(args) => a2ui::export(args.into(), out),
        Command::ImportA2ui(args) => a2ui::import(args.into(), out),
        Command::ImportCem {
            file,
            name,
            version,
            prefix,
            catalog,
            project,
            out_dir,
            force,
        } => cem::import(
            cem::Args {
                file,
                name,
                version,
                prefix,
                catalog,
                project,
                out_dir,
                force,
            },
            out,
        ),
        Command::Schema {
            catalog,
            project,
            no_project,
            out_dir,
            force,
        } => schema::export(
            schema::Args {
                catalog,
                project: ProjectArgs {
                    project,
                    no_project,
                },
                out_dir,
                force,
            },
            out,
        ),
    }
}

fn web_export(
    target: web::Target,
    common: WebExport,
    tokens: Option<PathBuf>,
    data: Option<PathBuf>,
    jsx: Option<(bool, bool)>,
    out: &mut dyn Write,
) -> Result<u8> {
    let (typescript, javascript) = jsx.unwrap_or_default();
    web::export(
        web::ExportArgs {
            target,
            file: common.file,
            catalog: common.catalog,
            tokens,
            project: common.project,
            out_dir: common.out_dir,
            force: common.force,
            typescript,
            javascript,
            source: common.source,
            no_source: common.no_source,
            context: common.context.map(|c| c == convert::ExportContext::Keep),
            data,
        },
        out,
    )
}

fn web_import(target: web::Target, args: WebImport, out: &mut dyn Write) -> Result<u8> {
    web::import(
        web::ImportArgs {
            target,
            file: args.file,
            catalog: args.catalog,
            tokens: args.tokens,
            project: args.project,
            out_dir: args.out_dir,
            context: args.context.map(|c| c == convert::ImportContext::Keep),
            force: args.force,
        },
        out,
    )
}

/// A document with errors has no reliable meaning to read back, so its diagnostics are printed as
/// `validate` prints them and the caller stops.
fn load_markup(
    file: &Path,
    catalog: Option<&Catalog>,
    out: &mut dyn Write,
) -> Result<Option<Document>> {
    if file.extension().is_some_and(|e| e == "json") {
        bail!(
            "{}: explain reads markup, not canonical JSON",
            file.display()
        );
    }
    let text = read(file)?;
    let result = parse(
        &text,
        &ParseOptions {
            catalog,
            ..Default::default()
        },
    );
    match result.document {
        Some(document) if !has_errors(&result.diagnostics) => Ok(Some(document)),
        _ => {
            print(file, &result.diagnostics, out)?;
            Ok(None)
        }
    }
}

/// One line per diagnostic: `file:line:col code message — hint`; JSON input has a path instead.
pub(crate) fn print(file: &Path, diagnostics: &[Diagnostic], out: &mut dyn Write) -> Result<()> {
    let file = file.display();
    for d in diagnostics {
        let place = match d.line {
            Some(line) => format!("{file}:{line}:{}", d.column.unwrap_or(1)),
            None => format!("{file}:{}", d.path),
        };
        let hint = d
            .hint
            .as_ref()
            .map(|h| format!(" — {h}"))
            .unwrap_or_default();
        writeln!(out, "{place} {} {}{hint}", d.code.as_str(), d.message)?;
    }
    Ok(())
}
