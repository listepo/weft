//! The editor's document core over the corpus and small scratch projects: open, save, state and
//! the render input, and each way an open, a render or a save is refused.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use weft_core::{ParseOptions, has_errors, parse, parse_json, serialize};
use weft_slint::GenerateError;
use weft_studio::{Editor, EditorError, MAX_FILE_BYTES};

const LOGIN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/login/screen.weft"
);

/// A directory of its own per test, removed afterwards.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("weft-studio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut screens: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path().join("screen.weft"))
        .filter(|path| path.is_file())
        .collect();
    screens.sort();
    assert!(!screens.is_empty());
    screens
}

#[test]
fn every_corpus_screen_opens_renders_and_saves_byte_identical() {
    let scratch = Scratch::new("corpus");
    for screen in corpus() {
        let name = screen.parent().unwrap().file_name().unwrap();
        let mut editor = Editor::open(&screen).unwrap();
        let state = editor.state();
        assert_eq!(state.title, "screen.weft");
        assert!(state.project.is_none(), "{screen:?}");
        assert!(!has_errors(&state.diagnostics), "{screen:?}");
        let input = editor.render_input().unwrap();
        assert!(input.source.contains("export component"), "{screen:?}");
        assert!(input.data.is_none());
        let copy = scratch.0.join(name).with_extension("weft");
        editor.save(Some(&copy)).unwrap();
        assert_eq!(
            std::fs::read(&copy).unwrap(),
            std::fs::read(&screen).unwrap()
        );
        assert_eq!(editor.state().path, copy);
    }
}

#[test]
fn a_non_canonical_file_saves_as_weft_fmt_prints_it() {
    let scratch = Scratch::new("fmt");
    let text = r#"<screen  weft="0.1" label="Hi"   id="hi"><heading level="1" id="t">Hi</heading></screen>"#;
    let file = scratch.write("screen.weft", text);
    let mut editor = Editor::open(&file).unwrap();
    editor.save(None).unwrap();
    // `weft fmt` parses without a catalog and prints the canonical markup.
    let formatted = serialize(&parse(text, &ParseOptions::default()).document.unwrap());
    assert_ne!(formatted, text);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), formatted);
}

#[test]
fn a_syntax_error_refuses_the_open_with_its_diagnostics() {
    let scratch = Scratch::new("syntax");
    let file = scratch.write("screen.weft", r#"<screen id="s" weft="0.1"><heading"#);
    match Editor::open(&file) {
        Err(EditorError::Syntax { path, diagnostics }) => {
            assert_eq!(path, file);
            assert!(has_errors(&diagnostics));
        }
        other => panic!("{:?}", other.map(|e| e.state())),
    }
}

#[test]
fn a_strictly_invalid_document_opens_and_its_render_is_refused() {
    let scratch = Scratch::new("strict");
    let file = scratch.write(
        "screen.weft",
        r#"<screen id="s" weft="0.1"><heading id="t" level="1" shade="dark">Hi</heading></screen>"#,
    );
    let editor = Editor::open(&file).unwrap();
    assert!(!editor.state().diagnostics.is_empty());
    let error = editor.render_input().unwrap_err();
    let EditorError::Generate(GenerateError::Invalid(diagnostics)) = &error else {
        panic!("{error:?}");
    };
    assert!(has_errors(diagnostics));
    assert_eq!(
        error.to_string(),
        format!("the document has {} diagnostics", diagnostics.len())
    );
}

#[test]
fn render_data_feeds_the_sample_data() {
    let scratch = Scratch::new("data");
    scratch.write("weft.json", r#"{ "render": { "data": "sample.json" } }"#);
    scratch.write("sample.json", r#"{ "email": "ada@example.com" }"#);
    let screen = scratch.write(
        "screens/login.weft",
        &std::fs::read_to_string(LOGIN).unwrap(),
    );
    let editor = Editor::open(&screen).unwrap();
    assert_eq!(editor.state().project, Some(scratch.0.join("weft.json")));
    let input = editor.render_input().unwrap();
    assert_eq!(
        input.data,
        Some(parse_json(r#"{ "email": "ada@example.com" }"#).unwrap())
    );
}

#[test]
fn sample_data_that_is_not_json_is_refused() {
    let scratch = Scratch::new("bad-data");
    scratch.write("weft.json", r#"{ "render": { "data": "sample.json" } }"#);
    scratch.write("sample.json", "{ not json");
    let screen = scratch.write("login.weft", &std::fs::read_to_string(LOGIN).unwrap());
    let error = Editor::open(&screen).unwrap().render_input().unwrap_err();
    assert!(matches!(&error, EditorError::SampleData(name) if name == "sample.json"));
}

#[test]
fn a_project_with_errors_still_opens_and_reports_them() {
    let scratch = Scratch::new("project");
    scratch.write("weft.json", r#"{ "catalog": "missing.catalog.json" }"#);
    let screen = scratch.write("login.weft", &std::fs::read_to_string(LOGIN).unwrap());
    let editor = Editor::open(&screen).unwrap();
    let state = editor.state();
    assert_eq!(state.project, Some(scratch.0.join("weft.json")));
    assert!(has_errors(&state.project_diagnostics));
    assert!(editor.render_input().is_ok());
}

#[test]
fn a_file_over_the_limit_is_refused() {
    let scratch = Scratch::new("large");
    let file = scratch.write("screen.weft", &" ".repeat(MAX_FILE_BYTES + 1));
    assert!(matches!(Editor::open(&file), Err(EditorError::TooLarge(path)) if path == file));
}

#[test]
fn a_missing_file_is_a_read_error() {
    let scratch = Scratch::new("missing");
    let file = scratch.0.join("absent.weft");
    assert!(matches!(Editor::open(&file), Err(EditorError::Read(path, _)) if path == file));
}

#[test]
fn a_failed_save_leaves_the_original_untouched_and_no_temporary_file() {
    let scratch = Scratch::new("failed-save");
    let screen = scratch.write("login.weft", &std::fs::read_to_string(LOGIN).unwrap());
    // A directory in the way: the rename over it fails on every platform.
    let kept = scratch.write("taken.weft/keep.txt", "kept");
    let mut editor = Editor::open(&screen).unwrap();
    let error = editor
        .save(Some(&scratch.0.join("taken.weft")))
        .unwrap_err();
    assert!(matches!(error, EditorError::Write(_)), "{error:?}");
    assert_eq!(std::fs::read_to_string(kept).unwrap(), "kept");
    assert_eq!(scratch.names(), ["login.weft", "taken.weft"]);
    assert_eq!(editor.state().path, screen);
}
