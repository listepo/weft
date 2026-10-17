//! `weft winui`, `weft winui-tokens` and `weft import-winui` (SPEC §9, §10.6). The catalog and
//! tokens come from the arguments, else the project, else the core catalog and the default
//! tokens; the output directory from `--out-dir`, else the project's `export.winui.outDir` or
//! `import.winui.outDir`, else standard output. `--namespace`, else `export.winui.namespace`, else
//! `WeftScreens`, is the C# namespace; `--name` the page name the generator is given. A page reads
//! the shared `WeftTokens.xaml` unless `--no-shared-tokens`, `export.winui.sharedTokens` `false`
//! or the lack of a project gives it its own. The files written are the generator's whole set, so
//! a file it adds needs no change here.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use weft_catalog::{PROJECT_FILE, Project, appearance};
use weft_winui::{
    GenerateError, GenerateOptions, ImportError, ImportOptions, TOKENS_FILE, generate,
    generate_tokens, helpers, import_winui,
};

use crate::convert::{
    catalog, emit, emit_files, keeps_context, out_dir, project, strict_document, switch_or, tokens,
};
use crate::{DIAGNOSTICS, ProjectArgs, print, read};

/// The namespace of the goldens, which the Windows build compiles (T105.7).
const DEFAULT_NAMESPACE: &str = "WeftScreens";

pub struct ExportArgs {
    pub file: PathBuf,
    pub catalog: Vec<PathBuf>,
    pub tokens: Option<PathBuf>,
    pub name: Option<String>,
    pub namespace: Option<String>,
    /// `--shared-tokens` and `--no-shared-tokens`.
    pub shared_tokens: (bool, bool),
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    /// `--context`: `Some(true)` keeps, `Some(false)` strips.
    pub context: Option<bool>,
    pub force: bool,
}

pub fn export(args: ExportArgs, out: &mut dyn Write) -> Result<u8> {
    let (loaded, project_dir) = project(&args.file, args.project)?;
    let loaded = loaded.as_ref();
    let catalog = catalog(&args.catalog, loaded)?;
    let tokens = tokens(args.tokens.as_deref(), loaded)?;
    let Some(mut document) = strict_document(&args.file, &catalog)? else {
        return Ok(DIAGNOSTICS);
    };
    if !keeps_context(args.context, loaded, "export", "winui", true) {
        document.context.clear();
    }
    let namespace = namespace(args.namespace, loaded);
    // A project's pages share one tokens file; a lone page builds on its own.
    let (on, off) = args.shared_tokens;
    let shared = &["export", "winui", "sharedTokens"];
    let shared_tokens = switch_or(on, off, loaded, shared, loaded.is_some());
    let errors = &mut std::io::stderr();
    let generated = match generate(
        &document,
        &GenerateOptions {
            catalog: &catalog,
            tokens: &tokens.tokens,
            name: args.name.as_deref(),
            namespace: &namespace,
            shared_tokens,
            appearance: appearance(&tokens.modifiers),
        },
    ) {
        Ok(generated) => generated,
        Err(GenerateError::Invalid(diagnostics)) => {
            print(&args.file, &diagnostics, errors)?;
            return Ok(DIAGNOSTICS);
        }
        Err(GenerateError::Unsupported(problems)) => {
            for problem in problems {
                writeln!(errors, "{}:{problem}", args.file.display())?;
            }
            return Ok(DIAGNOSTICS);
        }
        Err(error) => {
            writeln!(errors, "{}: {error}", args.file.display())?;
            return Ok(DIAGNOSTICS);
        }
    };
    let dir = out_dir(
        args.out_dir,
        loaded,
        project_dir.as_deref(),
        "export",
        "winui",
    );
    emit_files(&generated.files(), dir, args.force, out)?;
    Ok(0)
}

/// `--namespace`, else `export.winui.namespace`, else `WeftScreens`.
fn namespace(arg: Option<String>, loaded: Option<&Project>) -> String {
    arg.unwrap_or_else(|| {
        loaded
            .and_then(|p| p.setting(&["export", "winui", "namespace"]))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(DEFAULT_NAMESPACE)
            .to_owned()
    })
}

pub struct TokensArgs {
    pub tokens: Option<PathBuf>,
    pub namespace: Option<String>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    pub force: bool,
}

/// `WeftTokens.xaml` and `Weft.cs`, the files every page of a project shares. Tokens WinUI has
/// no form for are left out with a warning: only a page that uses one fails.
pub fn export_tokens(args: TokensArgs, out: &mut dyn Write) -> Result<u8> {
    // The project is found as for a document in the working directory.
    let (loaded, project_dir) = project(Path::new(PROJECT_FILE), args.project)?;
    let loaded = loaded.as_ref();
    let tokens = tokens(args.tokens.as_deref(), loaded)?;
    let errors = &mut std::io::stderr();
    let helpers = match helpers(&namespace(args.namespace, loaded)) {
        Ok(helpers) => helpers,
        Err(error) => {
            writeln!(errors, "weft: {error}")?;
            return Ok(DIAGNOSTICS);
        }
    };
    let (xaml, skipped) = generate_tokens(&tokens.tokens, appearance(&tokens.modifiers));
    for p in skipped {
        writeln!(errors, "weft: token {p}; it is left out")?;
    }
    let dir = out_dir(
        args.out_dir,
        loaded,
        project_dir.as_deref(),
        "export",
        "winui",
    );
    let files = [(TOKENS_FILE, xaml.as_str()), ("Weft.cs", helpers.as_str())];
    emit_files(&files, dir, args.force, out)?;
    Ok(0)
}

pub struct ImportArgs {
    pub file: PathBuf,
    pub catalog: Vec<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    /// `--context`: `Some(true)` keeps, `Some(false)` drops.
    pub context: Option<bool>,
    pub force: bool,
}

pub fn import(args: ImportArgs, out: &mut dyn Write) -> Result<u8> {
    let (loaded, project_dir) = project(&args.file, args.project)?;
    let loaded = loaded.as_ref();
    let catalog = catalog(&args.catalog, loaded)?;
    // The page is believed only when regenerating it gives the same XAML, so the tokens are the
    // ones generation used: the project's, or the default tokens. Name and namespace come from
    // the page's comment.
    let tokens = tokens(None, loaded)?;
    let text = read(&args.file)?;
    let mut document = match import_winui(
        &text,
        &ImportOptions {
            catalog: &catalog,
            tokens: &tokens.tokens,
        },
    ) {
        Ok(document) => document,
        Err(ImportError::Invalid(diagnostics)) => {
            print(&args.file, &diagnostics, &mut std::io::stderr())?;
            return Ok(DIAGNOSTICS);
        }
        Err(error) => {
            writeln!(std::io::stderr(), "{}: {error}", args.file.display())?;
            return Ok(DIAGNOSTICS);
        }
    };
    if !keeps_context(args.context, loaded, "import", "winui", true) {
        document.context.clear();
    }
    let dir = out_dir(
        args.out_dir,
        loaded,
        project_dir.as_deref(),
        "import",
        "winui",
    );
    emit(
        &weft_core::serialize(&document),
        &args.file,
        dir,
        "weft",
        args.force,
        out,
    )?;
    Ok(0)
}
