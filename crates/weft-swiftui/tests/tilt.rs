//! A tilt is a chain of `rotation3DEffect`s, z then y then x, with the perspective on the last
//! one only; a dialog keeps its tilt as a marker. All of it reads back as the same attributes.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use weft_swiftui::{GenerateOptions, ImportOptions, generate, import_swiftui};

fn swift(body: &str) -> (String, weft_core::Document) {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let markup = format!("<screen id=\"root\" label=\"Test\" weft=\"0.3\">\n  {body}\n</screen>\n");
    let document = common::parse_screen(&markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
        shared_tokens: false,
        data: None,
        appearance: None,
    };
    (generate(&document, &options).unwrap(), document)
}

fn back(source: &str) -> weft_core::Document {
    let catalog = common::catalog();
    let result = import_swiftui(source, &ImportOptions { catalog: &catalog });
    assert!(result.losses.is_empty(), "{:?}", result.losses);
    result.document
}

#[test]
fn the_chain_runs_z_then_y_then_x_and_the_last_effect_has_the_perspective() {
    let (out, _) = swift(
        "<section id=\"s\" label=\"S\" perspective=\"800\" rotate-x=\"20\" rotate-y=\"25\" rotate-z=\"-6\"/>",
    );
    let at = |needle: &str| {
        out.find(needle)
            .unwrap_or_else(|| panic!("{needle}\n{out}"))
    };
    let z = at(".rotation3DEffect(.degrees(-6), axis: (x: 0, y: 0, z: 1), perspective: 0)");
    let y = at(".rotation3DEffect(.degrees(25), axis: (x: 0, y: 1, z: 0), perspective: 0)");
    let x = at(".rotation3DEffect(.degrees(20), axis: (x: 1, y: 0, z: 0), perspective: 400 / 800)");
    assert!(z < y && y < x, "{out}");
}

#[test]
fn a_tilt_reads_back_as_the_same_attributes() {
    for body in [
        "<section id=\"s\" label=\"S\" perspective=\"800\" rotate-x=\"20\" rotate-y=\"25\" rotate-z=\"-6\"/>",
        "<section id=\"s\" label=\"S\" rotate-y=\"180\"/>",
        "<section id=\"s\" label=\"S\" perspective=\"600.5\"/>",
        "<section id=\"s\" label=\"S\" rotate-z=\"0\"/>",
    ] {
        let (out, document) = swift(body);
        assert_eq!(back(&out), document, "{body}\n{out}");
    }
}

#[test]
fn a_dialog_keeps_its_tilt_as_a_marker() {
    let (out, document) = swift("<dialog id=\"d\" label=\"D\" rotate-x=\"10\"/>");
    assert!(!out.contains("rotation3DEffect"), "{out}");
    assert!(out.contains(".weftProp(\"rotate-x\", 10)"), "{out}");
    assert_eq!(back(&out), document);
}

#[test]
fn an_effect_about_another_axis_is_a_loss() {
    let (out, _) = swift("<section id=\"s\" label=\"S\" rotate-y=\"25\"/>");
    let skewed = out.replace("axis: (x: 0, y: 1, z: 0)", "axis: (x: 1, y: 1, z: 0)");
    let catalog = common::catalog();
    let result = import_swiftui(&skewed, &ImportOptions { catalog: &catalog });
    assert!(
        result.losses.iter().any(|l| l.note.contains("axis")),
        "{:?}",
        result.losses
    );
}

#[test]
fn a_model_shows_the_bundled_usdz_over_the_still_and_reads_back() {
    let (out, document) = swift(
        "<model id=\"m\" fallback=\"assets/gem.png\" label=\"A gem\" src=\"assets/gem.glb\" usdz=\"assets/gem.usdz\"/>",
    );
    assert!(
        out.contains("WeftModel(usdz: \"assets/gem.usdz\", fallback: \"assets/gem.png\")"),
        "{out}"
    );
    // The glTF file is for the web only: SwiftUI cannot read it, so it is kept as a marker.
    assert!(
        out.contains(".weftProp(\"src\", \"assets/gem.glb\")"),
        "{out}"
    );
    assert!(out.contains("WEFT_STILL_MODELS"), "{out}");
    assert_eq!(back(&out), document);
}

#[test]
fn a_screen_without_a_model_carries_no_model_code() {
    let (out, _) = swift("<text id=\"t\">Hi</text>");
    assert!(
        !out.contains("WeftModel") && !out.contains("RealityView"),
        "{out}"
    );
}

#[test]
fn a_model_without_a_usdz_is_only_the_still() {
    let (out, document) = swift(
        "<model id=\"m\" fallback=\"assets/gem.png\" label=\"A gem\" src=\"assets/gem.glb\"/>",
    );
    assert!(
        out.contains("WeftModel(fallback: \"assets/gem.png\")"),
        "{out}"
    );
    assert_eq!(back(&out), document);
}
