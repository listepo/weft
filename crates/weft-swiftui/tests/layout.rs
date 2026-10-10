//! The layout vocabulary (SPEC §5.1) in SwiftUI: a row's `justify` as spacers, `grow` as a
//! full-width frame, `padding` and `max-width` as modifiers with theme tokens, and
//! `min-column-width` as the generated `WeftColumns` layout. Each reads back to the same props from
//! the code alone; what SwiftUI cannot draw stays a marker, and other spacers, paddings and frames
//! are `layout` losses.

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
    let out = generate(&document, &options).unwrap();
    // Without the `weft:source` comment the importer has only the code to go on.
    let (comment, code) = out.split_once("\n\n").unwrap();
    assert!(comment.starts_with("// weft:source"), "{out}");
    (code.to_owned(), document)
}

fn back(source: &str) -> weft_core::Document {
    let catalog = common::catalog();
    let result = import_swiftui(source, &ImportOptions { catalog: &catalog });
    assert!(result.losses.is_empty(), "{:?}\n{source}", result.losses);
    result.document
}

fn layout_losses(source: &str) -> Vec<String> {
    let catalog = common::catalog();
    import_swiftui(source, &ImportOptions { catalog: &catalog })
        .losses
        .into_iter()
        .map(|l| format!("{:?} {} {}", l.kind, l.path, l.note))
        .collect()
}

fn round_trip(body: &str, expect: &[&str], absent: &[&str]) -> String {
    let (out, document) = swift(body);
    for needle in expect {
        assert!(out.contains(needle), "{needle}\n{out}");
    }
    for needle in absent {
        assert!(!out.contains(needle), "{needle}\n{out}");
    }
    assert_eq!(back(&out), document, "{body}\n{out}");
    out
}

const PAIR: &str = "<text id=\"a\">A</text><text id=\"b\">B</text>";

#[test]
fn a_rows_justify_is_drawn_with_spacers() {
    let row = |justify: &str| {
        format!(
            "<stack id=\"row\" direction=\"row\" gap=\"{{token.space.sm}}\" justify=\"{justify}\">{PAIR}</stack>"
        )
    };
    let end = round_trip(
        &row("end"),
        &["Spacer(minLength: 0)"],
        &["weftProp(\"justify\""],
    );
    assert_eq!(end.matches("Spacer(").count(), 1, "{end}");
    let center = round_trip(
        &row("center"),
        &["Spacer(minLength: 0)"],
        &["weftProp(\"justify\""],
    );
    assert_eq!(center.matches("Spacer(").count(), 2, "{center}");
    round_trip(
        &row("space-between"),
        &["HStack(spacing: 0) {", "Spacer(minLength: theme.space.sm)"],
        &["weftProp(\"justify\"", "Spacer(minLength: 0)"],
    );
    round_trip(
        &format!("<stack id=\"row\" direction=\"row\" justify=\"space-between\">{PAIR}</stack>"),
        &["HStack(spacing: 0) {", "Spacer()"],
        &["weftProp(\"justify\""],
    );
}

#[test]
fn what_spacers_cannot_say_stays_a_marker() {
    for body in [
        // `start` is the row's own layout.
        format!("<stack id=\"row\" direction=\"row\" justify=\"start\">{PAIR}</stack>"),
        // A column has free height only inside a stretched row.
        format!("<stack id=\"col\" justify=\"end\">{PAIR}</stack>"),
        // A single child sits at the start.
        "<stack id=\"row\" direction=\"row\" justify=\"space-between\"><text id=\"a\">A</text></stack>".to_owned(),
        // A growing child leaves no free space.
        "<stack id=\"row\" direction=\"row\" justify=\"center\"><text id=\"a\" grow=\"true\">A</text><text id=\"b\">B</text></stack>".to_owned(),
        "<stack id=\"row\" direction=\"row\" justify=\"{$.how}\"><text id=\"a\">A</text></stack>".to_owned(),
    ] {
        round_trip(&body, &[".weftProp(\"justify\""], &["Spacer("]);
    }
}

#[test]
fn space_between_puts_a_spacer_between_the_items_of_a_list() {
    // The list leads the row: its first item has no spacer before it.
    let first = round_trip(
        "<stack id=\"row\" direction=\"row\" gap=\"{token.space.sm}\" justify=\"space-between\"><each id=\"items\" as=\"item\" in=\"{$.items}\"><text id=\"t\" text=\"{$item.name}\"/></each><text id=\"end\">End</text></stack>",
        &["if itemIndex > 0 {"],
        &[],
    );
    assert_eq!(
        first.matches("Spacer(minLength: theme.space.sm)").count(),
        2,
        "{first}"
    );
    // After another child, every item has one.
    let after = round_trip(
        "<stack id=\"row\" direction=\"row\" justify=\"space-between\"><text id=\"start\">Start</text><each id=\"items\" as=\"item\" in=\"{$.items}\"><text id=\"t\" text=\"{$item.name}\"/><text id=\"u\" text=\"{$item.note}\"/></each></stack>",
        &["Spacer()"],
        &["Index > 0"],
    );
    assert_eq!(after.matches("Spacer()").count(), 2, "{after}");
    // Nested lists: the first item of the first group only.
    round_trip(
        "<stack id=\"row\" direction=\"row\" justify=\"space-between\"><each id=\"groups\" as=\"group\" in=\"{$.groups}\"><each id=\"items\" as=\"item\" in=\"{$group.items}\"><text id=\"t\" text=\"{$item.name}\"/></each></each></stack>",
        &["if groupIndex > 0 || itemIndex > 0 {"],
        &[],
    );
}

#[test]
fn grow_is_a_full_width_frame_in_a_row_and_a_marker_in_a_column() {
    round_trip(
        "<stack id=\"row\" direction=\"row\"><field id=\"f\" grow=\"true\" label=\"F\" value=\"{$.q}\"/><each id=\"items\" as=\"item\" in=\"{$.items}\"><text id=\"t\" grow=\"true\" text=\"{$item.name}\"/></each></stack>",
        &[".frame(maxWidth: .infinity)"],
        &["weftProp(\"grow\""],
    );
    round_trip(
        "<stack id=\"col\"><text id=\"t\" grow=\"true\">T</text></stack>",
        &[".weftProp(\"grow\", true)"],
        &[".frame("],
    );
    // Only the row's own children: a section in the row starts its own layout.
    round_trip(
        "<stack id=\"row\" direction=\"row\"><section id=\"s\" label=\"S\"><stack id=\"in\"><text id=\"t\" grow=\"true\">T</text></stack></section></stack>",
        &[".weftProp(\"grow\", true)"],
        &[".frame("],
    );
}

#[test]
fn padding_and_max_width_are_modifiers_inside_the_material() {
    let out = round_trip(
        "<stack id=\"card\" align=\"center\" material=\"{token.material.glass}\" max-width=\"{token.size.md}\" padding=\"{token.space.lg}\"><text id=\"t\">T</text></stack>",
        &[
            ".padding(theme.space.lg)",
            ".frame(maxWidth: .infinity, alignment: .center)",
            ".frame(maxWidth: theme.size.md)",
        ],
        &["weftProp(\"padding\"", "weftProp(\"max-width\""],
    );
    let at = |n: &str| out.find(n).unwrap();
    assert!(
        at(".padding(") < at(".frame(maxWidth: .infinity")
            && at(".frame(maxWidth: theme") < at(".modifier(theme.material")
    );
    round_trip(
        "<grid id=\"g\" columns=\"2\" max-width=\"{token.size.lg}\" padding=\"{token.space.sm}\"><text id=\"t\">T</text></grid>",
        &[
            "LazyVGrid",
            ".padding(theme.space.sm)",
            ".frame(maxWidth: theme.size.lg)",
        ],
        &["WeftColumns"],
    );
}

#[test]
fn a_reflowing_grid_is_the_generated_layout_and_keeps_sections_whole() {
    let out = round_trip(
        "<grid id=\"g\" columns=\"3\" gap=\"{token.space.lg}\" min-column-width=\"{token.size.sm}\"><section id=\"s\" label=\"S\"><text id=\"t\">T</text></section><text id=\"u\">U</text></grid>",
        &[
            "WeftColumns(columns: 3, minWidth: theme.size.sm, spacing: theme.space.lg) {",
            "fileprivate struct WeftColumns: Layout",
        ],
        &["LazyVGrid(", "weftProp(\"min-column-width\""],
    );
    assert_eq!(out.matches("WeftCell {").count(), 1, "{out}");
    round_trip(
        "<grid id=\"g\" columns=\"{$.n}\" min-column-width=\"{token.size.sm}\"><text id=\"t\">T</text></grid>",
        &["WeftColumns(columns: weftColumns(model.n), minWidth: theme.size.sm) {"],
        &[],
    );
    let (plain, _) = swift("<grid id=\"g\" columns=\"2\"><text id=\"t\">T</text></grid>");
    assert!(!plain.contains("WeftColumns"), "{plain}");
}

#[test]
fn other_spacers_paddings_and_frames_are_layout_losses() {
    let lost = |body: &str, from: &str, to: &str, note: &str| {
        let (out, _) = swift(body);
        let edited = out.replace(from, to);
        assert_ne!(edited, out, "{from}");
        let losses = layout_losses(&edited);
        assert!(
            losses
                .iter()
                .any(|l| l.starts_with("Layout") && l.contains(note)),
            "{note}: {losses:?}\n{edited}"
        );
    };
    let card = "<stack id=\"card\" max-width=\"{token.size.md}\" padding=\"{token.space.lg}\"><text id=\"t\">T</text></stack>";
    lost(
        card,
        ".padding(theme.space.lg)",
        ".padding(16)",
        "`.padding`",
    );
    lost(
        card,
        ".frame(maxWidth: theme.size.md)",
        ".frame(maxWidth: 480)",
        "`.frame`",
    );
    let row = format!("<stack id=\"row\" direction=\"row\" justify=\"end\">{PAIR}</stack>");
    lost(
        &row,
        "Spacer(minLength: 0)",
        "Spacer(minLength: 12)",
        "`Spacer`",
    );
    let between = format!(
        "<stack id=\"row\" direction=\"row\" gap=\"{{token.space.sm}}\" justify=\"space-between\">{PAIR}<text id=\"c\">C</text></stack>"
    );
    // Spacers of two lengths are not the generator's.
    let (out, _) = swift(&between);
    let edited = out.replacen(
        "Spacer(minLength: theme.space.sm)",
        "Spacer(minLength: theme.space.md)",
        1,
    );
    assert!(
        layout_losses(&edited)
            .iter()
            .any(|l| l.contains("`Spacer`"))
    );
    let column = "<stack id=\"col\"><text id=\"t\" grow=\"true\">T</text></stack>";
    lost(
        column,
        ".weftProp(\"grow\", true)",
        ".frame(maxWidth: .infinity)",
        "`.frame`",
    );
    let grid = "<grid id=\"g\" columns=\"2\" min-column-width=\"{token.size.sm}\"><text id=\"t\">T</text></grid>";
    lost(
        grid,
        "minWidth: theme.size.sm",
        "minWidth: 200",
        "column width",
    );
}
