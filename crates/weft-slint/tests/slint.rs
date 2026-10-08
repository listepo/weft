//! Weft → Slint → Weft over corpus screens and a fixture of the other mapped controls: the
//! generated `.slint` matches its reviewed golden file, compiles with the Slint compiler, and
//! reads back to the canonical document, which the Weft validator accepts. `WEFT_UPDATE_FIXTURES=1`
//! rewrites the golden files.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::task::{Context, Poll, Waker};

use weft_catalog::token_types;
use weft_core::{Mode, ValidateOptions, serialize, validate_document};
use weft_slint::{
    GenerateError, GenerateOptions, ImportError, ImportOptions, MAX_SOURCE_LENGTH, generate,
    import_slint,
};

use common::{document, exported_component, here, screens, setup};

/// Error-level diagnostics of the Slint compiler, and whether it built `component`.
fn compile(source: &str, component: &str) -> (Vec<String>, bool) {
    let compiler = slint_interpreter::Compiler::default();
    let mut build =
        std::pin::pin!(compiler.build_from_source(source.into(), "screen.slint".into()));
    let mut cx = Context::from_waker(Waker::noop());
    // Without a file loader the build never waits, so polling once is enough; the loop guards it.
    let result = loop {
        if let Poll::Ready(result) = build.as_mut().poll(&mut cx) {
            break result;
        }
    };
    let errors = result
        .diagnostics()
        .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
        .map(|d| d.message().to_owned())
        .collect();
    (errors, result.component(component).is_some())
}

#[test]
fn generated_slint_matches_the_golden_files() {
    let (catalog, tokens) = setup();
    let update = std::env::var_os("WEFT_UPDATE_FIXTURES").is_some();
    for (name, path) in screens() {
        let doc = document(&std::fs::read_to_string(&path).unwrap(), &catalog, &tokens);
        let options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        };
        let slint = generate(&doc, &options).unwrap();
        let golden = here(&format!("tests/fixtures/{name}.slint"));
        if update {
            std::fs::write(&golden, &slint).unwrap();
        }
        assert_eq!(slint, std::fs::read_to_string(&golden).unwrap(), "{name}");
    }
}

#[test]
fn generated_slint_compiles() {
    let (catalog, tokens) = setup();
    for (name, path) in screens() {
        let doc = document(&std::fs::read_to_string(&path).unwrap(), &catalog, &tokens);
        let options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        };
        let slint = generate(&doc, &options).unwrap();
        let (errors, built) = compile(&slint, exported_component(&slint));
        assert!(errors.is_empty() && built, "{name}: {errors:#?}");
    }
}

#[test]
fn generated_slint_reads_back_to_the_canonical_document() {
    let (catalog, tokens) = setup();
    for (name, path) in screens() {
        let markup = std::fs::read_to_string(&path).unwrap();
        let doc = document(&markup, &catalog, &tokens);
        let gen_options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        };
        let slint = generate(&doc, &gen_options).unwrap();
        let options = ImportOptions {
            catalog: &catalog,
            tokens: &tokens,
        };
        let back = import_slint(&slint, &options).unwrap();
        assert_eq!(serialize(&back), markup, "{name}");
        let types = token_types(&tokens);
        let checked = ValidateOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            tokens: Some(&types),
            actions: None,
        };
        assert!(validate_document(&back, &checked).is_empty(), "{name}");
        // A reindented file is the same file.
        let indented: String = slint.lines().map(|l| format!("  {l}\n")).collect();
        assert_eq!(import_slint(&indented, &options).unwrap(), back, "{name}");
    }
}

#[test]
fn a_named_screen_keeps_its_name_through_the_round_trip() {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: Some("sign-in"),
    };
    let slint = generate(&doc, &options).unwrap();
    assert!(slint.contains("export component SignInScreen inherits Window"));
    let import = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    assert_eq!(serialize(&import_slint(&slint, &import).unwrap()), markup);
}

#[test]
fn a_bound_heading_level_is_refused_with_its_path() {
    let (catalog, tokens) = setup();
    let markup =
        r#"<screen id="s" weft="0.1"><heading id="h" level="{$.n}">Title</heading></screen>"#;
    let doc = document(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let Err(GenerateError::Unsupported(problems)) = generate(&doc, &options) else {
        panic!("a bound heading level must be refused");
    };
    assert_eq!(problems[0].path, "heading#h");
}

#[test]
fn ids_that_slint_reads_as_one_are_refused() {
    let (catalog, tokens) = setup();
    let markup =
        r#"<screen id="s" weft="0.1"><text id="a-b">A</text><text id="a_b">B</text></screen>"#;
    let doc = document(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let Err(GenerateError::Unsupported(problems)) = generate(&doc, &options) else {
        panic!("clashing ids must be refused");
    };
    assert_eq!(problems[0].path, "text#a_b");
}

#[test]
fn document_text_cannot_escape_its_string_literal() {
    let (catalog, tokens) = setup();
    let markup = r#"<screen id="s" weft="0.1"><text id="t">"; } export component X { \{root.y} &#10;</text></screen>"#;
    let doc = document(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let slint = generate(&doc, &options).unwrap();
    let (errors, built) = compile(&slint, "SScreen");
    assert!(errors.is_empty() && built, "{errors:#?}");
    assert_eq!(
        slint.lines().filter(|l| l.starts_with("export ")).count(),
        1
    );
}

#[test]
fn edited_or_foreign_slint_is_not_taken_for_its_comment() {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    let slint = generate(&doc, &options).unwrap();
    let import = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    let edited = slint.replace("text: \"Sign in\";", "text: \"Log in\";");
    assert!(matches!(
        import_slint(&edited, &import),
        Err(ImportError::Edited)
    ));
    let tampered = slint.replace(
        "// <screen id=\"login\" label=\"Sign in\"",
        "// <screen id=\"login\" label=\"Other\"",
    );
    assert!(matches!(
        import_slint(&tampered, &import),
        Err(ImportError::Edited)
    ));
    let foreign = "export component A inherits Window {}\n";
    assert!(matches!(
        import_slint(foreign, &import),
        Err(ImportError::NoSource)
    ));
    let broken = "// weft:source slint\n// <screen id=\"s\">\n";
    assert!(matches!(
        import_slint(broken, &import),
        Err(ImportError::Invalid(_))
    ));
    let long = " ".repeat(MAX_SOURCE_LENGTH + 1);
    assert!(matches!(
        import_slint(&long, &import),
        Err(ImportError::TooLong)
    ));
}
