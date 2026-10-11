//! React or SolidJS source → Weft (SPEC §9, "From source code"). A module that `to_jsx` wrote
//! with its `weft:source` comment comes back exactly when regenerating from the comment yields the
//! same program; any other component is read by the corpus conventions: its JSX is lowered to the
//! page it renders and read like an imported page, with what Weft cannot hold listed as losses.
//!
//! The source is untrusted: it is parsed by oxc, never run. Its length is bounded, nesting is
//! estimated before parsing (`scan`), and evaluation and output are bounded in steps and nodes.

mod eval;
mod lower;
mod scan;

use std::rc::Rc;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionBody, BindingPattern, Declaration, ExportDefaultDeclarationKind, Expression,
    FormalParameters, Program, Statement,
};
use oxc_parser::{ParseOptions as OxcOptions, Parser};
use oxc_span::{ContentEq, SourceType};
use weft_core::{Code, Diagnostic};
use weft_import::{ImportResult, Loss, empty_result, js_length, limit_reached};

use crate::dom::{Conventions, read_dom};
use crate::import::ImportOptions;
use crate::jsx::{Framework, JsxOptions, to_jsx};
use crate::provenance;
use crate::tree::DOCUMENT;
use eval::{Env, Eval, Sv, bind_pattern};

/// Longest source read, in UTF-16 code units, as JavaScript counts string length.
pub const MAX_JSX_LENGTH: usize = 2_000_000;

/// The stack the parser and the reader get on native targets. The nesting estimate keeps a
/// parse within 1 MiB in a release build; debug builds and deep evaluation need more.
#[cfg(not(target_arch = "wasm32"))]
const STACK: usize = 64 << 20;

/// Imports a React or SolidJS component (`.jsx`, or `.tsx` when `typescript`). Never fails: what
/// cannot be read is reported in the result.
pub fn import_jsx(source: &str, typescript: bool, options: &ImportOptions<'_>) -> ImportResult {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ran = std::thread::scope(|s| {
            std::thread::Builder::new()
                .name("weft-import-jsx".into())
                .stack_size(STACK)
                .spawn_scoped(s, || read(source, typescript, options))
                .map(|h| h.join())
        });
        match ran {
            Ok(Ok(result)) => return result,
            Ok(Err(_)) => {
                return empty_result(vec![Diagnostic::new(
                    Code::W601,
                    "#",
                    "The JSX reader stopped unexpectedly.",
                    "a React or SolidJS component",
                )]);
            }
            // No thread to be had: read on this one, within the same nesting estimate.
            Err(_) => {}
        }
    }
    read(source, typescript, options)
}

fn unreadable(mut diagnostics: Vec<Diagnostic>, message: String) -> ImportResult {
    diagnostics.push(Diagnostic::new(
        Code::W601,
        "#",
        message,
        "a module with a React or SolidJS component that returns JSX",
    ));
    empty_result(diagnostics)
}

fn source_type(typescript: bool) -> SourceType {
    if typescript {
        SourceType::tsx()
    } else {
        SourceType::jsx()
    }
}

fn parse<'a>(
    allocator: &'a Allocator,
    text: &'a str,
    typescript: bool,
) -> Result<Program<'a>, String> {
    let ret = Parser::new(allocator, text, source_type(typescript))
        .with_options(OxcOptions {
            preserve_parens: false,
            ..OxcOptions::default()
        })
        .parse();
    match ret.diagnostics.first() {
        Some(error) => Err(error.message.to_string()),
        None if ret.fatal_error => Err("the parser gave up".into()),
        None => Ok(ret.program),
    }
}

fn read(source: &str, typescript: bool, options: &ImportOptions<'_>) -> ImportResult {
    let mut diagnostics = Vec::new();
    if js_length(source) > MAX_JSX_LENGTH {
        limit_reached(
            &mut diagnostics,
            "#",
            &format!("is longer than {MAX_JSX_LENGTH} characters"),
        );
        return empty_result(diagnostics);
    }
    if !scan::within_budget(source) {
        limit_reached(&mut diagnostics, "#", "nests deeper than the parser allows");
        return empty_result(diagnostics);
    }
    let allocator = Allocator::default();
    // A `.jsx` file with type annotations, or the other way round, still reads.
    let program = match parse(&allocator, source, typescript) {
        Ok(p) => p,
        Err(first) => match parse(&allocator, source, !typescript) {
            Ok(p) => p,
            Err(_) => {
                return unreadable(diagnostics, format!("The source is not valid JSX: {first}"));
            }
        },
    };
    if let Some(result) = provenance_match(source, &program, options) {
        return result;
    }
    let Some((params, body)) = find_component(&program) else {
        return unreadable(diagnostics, "The source declares no component.".into());
    };
    let root_env = Rc::new(Env::default());
    let mut vars = Vec::new();
    bind_pattern(
        params.items.first().map(|p| &p.pattern),
        Sv::Props,
        &mut vars,
    );
    let env = Env::child(&root_env, vars);
    let mut eval = Eval { steps: 0 };
    let value = match body {
        Body::Statements(s) => eval.statements(s, &env),
        Body::Arrow(b) => eval.body(b, &env),
    };
    let Sv::Jsx(root, scope) = value else {
        return unreadable(diagnostics, "The component returns no JSX.".into());
    };
    let mut lowered = lower::lower(source, root, &scope);
    if lowered.truncated {
        limit_reached(
            &mut diagnostics,
            "#",
            "is larger or deeper than the import limit",
        );
    }
    let top = lowered.notes.remove(&DOCUMENT).unwrap_or_default();
    let conventions = Conventions::new(options.tokens);
    let built = read_dom(
        &lowered.dom,
        options.catalog,
        Some(&conventions),
        lowered.notes,
        diagnostics,
    );
    let mut result = built.result;
    result.losses.extend(top.into_iter().map(|n| Loss {
        kind: n.kind,
        path: built.root_path.clone(),
        note: n.note,
    }));
    result
}

/// The document the source's `weft:source` comment claims, when regenerating from it gives the
/// same program up to formatting and comments.
fn provenance_match(
    source: &str,
    program: &Program<'_>,
    options: &ImportOptions<'_>,
) -> Option<ImportResult> {
    let (head, markup) = provenance::find(source, "/*", "*/")?;
    let mut framework = None;
    let mut name = None;
    let mut typescript = false;
    for word in head.split_whitespace() {
        match word {
            "react" => framework = Some(Framework::React),
            "solid" => framework = Some(Framework::Solid),
            "typescript" => typescript = true,
            _ => name = word.strip_prefix("name=").or(name),
        }
    }
    let (document, diagnostics) = provenance::claimed(&markup, options.catalog)?;
    let json = serde_json::to_value(&document).ok()?;
    let again = to_jsx(
        &json,
        &JsxOptions {
            catalog: options.catalog,
            component_name: name,
            framework: framework?,
            typescript,
            source: true,
        },
    )
    .ok()?;
    let allocator = Allocator::default();
    let regenerated = parse(&allocator, &again, typescript).ok()?;
    let same = program.directives.content_eq(&regenerated.directives)
        && program.body.content_eq(&regenerated.body);
    same.then(|| ImportResult {
        document,
        losses: Vec::new(),
        diagnostics,
    })
}

enum Body<'a> {
    Statements(&'a [Statement<'a>]),
    Arrow(&'a ArrowFunctionBody<'a>),
}

/// A function's parameters and body, when the expression is one.
fn function_of<'a>(e: &'a Expression<'a>) -> Option<(&'a FormalParameters<'a>, Body<'a>)> {
    match e.get_inner_expression() {
        Expression::ArrowFunctionExpression(f) => Some((&f.params, Body::Arrow(&f.body))),
        Expression::FunctionExpression(f) => {
            let body = f.body.as_ref()?;
            Some((&f.params, Body::Statements(&body.statements)))
        }
        // `memo(Component)`, `observer(() => …)`: the wrapped component.
        Expression::CallExpression(c) => c
            .arguments
            .first()
            .and_then(|a| a.as_expression())
            .and_then(function_of),
        _ => None,
    }
}

/// Top-level functions by name, in source order, exported or not.
fn named_functions<'a>(
    program: &'a Program<'a>,
) -> Vec<(&'a str, &'a FormalParameters<'a>, Body<'a>)> {
    let mut out = Vec::new();
    let add_declaration = |d: &'a Declaration<'a>, out: &mut Vec<_>| match d {
        Declaration::FunctionDeclaration(f) => {
            if let (Some(id), Some(body)) = (&f.id, &f.body) {
                out.push((
                    id.name.as_str(),
                    &*f.params,
                    Body::Statements(&body.statements),
                ));
            }
        }
        Declaration::VariableDeclaration(v) => {
            for decl in &v.declarations {
                if let (BindingPattern::BindingIdentifier(id), Some(init)) = (&decl.id, &decl.init)
                    && let Some((params, body)) = function_of(init)
                {
                    out.push((id.name.as_str(), params, body));
                }
            }
        }
        _ => {}
    };
    for s in &program.body {
        match s {
            Statement::ExportDeclaration(e) => add_declaration(&e.declaration, &mut out),
            Statement::ExportDefaultDeclaration(e) => {
                if let ExportDefaultDeclarationKind::FunctionDeclaration(f) = &e.declaration
                    && let (Some(id), Some(body)) = (&f.id, &f.body)
                {
                    out.push((
                        id.name.as_str(),
                        &*f.params,
                        Body::Statements(&body.statements),
                    ));
                }
            }
            other => {
                if let Some(d) = other.as_declaration() {
                    add_declaration(d, &mut out);
                }
            }
        }
    }
    out
}

/// The component the module is about: its default export, else its first function whose name
/// starts with a capital letter.
fn find_component<'a>(program: &'a Program<'a>) -> Option<(&'a FormalParameters<'a>, Body<'a>)> {
    let named = named_functions(program);
    for s in &program.body {
        let Statement::ExportDefaultDeclaration(e) = s else {
            continue;
        };
        match &e.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                let body = f.body.as_ref()?;
                return Some((&f.params, Body::Statements(&body.statements)));
            }
            other => {
                let x = other.as_expression()?;
                if let Some(found) = function_of(x) {
                    return Some(found);
                }
                if let Expression::Identifier(id) = x.get_inner_expression() {
                    let mut named = named;
                    if let Some(i) = named.iter().position(|(n, ..)| *n == id.name.as_str()) {
                        let (_, params, body) = named.swap_remove(i);
                        return Some((params, body));
                    }
                }
                return None;
            }
        }
    }
    named
        .into_iter()
        .find(|(n, ..)| n.starts_with(|c: char| c.is_ascii_uppercase()))
        .map(|(_, params, body)| (params, body))
}
