//! WinUI → Weft for XAML `import_winui` rejects: a generated page without its source comment reads
//! back with only the losses its golden lists, a hand-written page reports each loss at its path,
//! and hostile or oversized input is refused with `W601` or cut with `W602`. The results always
//! validate in lenient mode. `WEFT_UPDATE_FIXTURES=1` rewrites the `.read.*` goldens.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{covered, here, setup};
use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::{
    Catalog, Child, Code, Mode, Node, Severity, ValidateOptions, serialize, validate_document,
};
use weft_winui::{
    ImportOptions, ImportResult, LossKind, MAX_SOURCE_LENGTH, MAX_XML_DEPTH, read_winui,
};

fn read(source: &str, catalog: &Catalog, tokens: &IndexMap<String, Token>) -> ImportResult {
    read_winui(source, &ImportOptions { catalog, tokens })
}

/// The page without its `weft:source` comment, as a hand edit that drops it leaves it.
fn bare(xaml: &str) -> String {
    let start = xaml.find("<!-- weft:source").unwrap();
    let end = xaml.find("-->\n<Page").unwrap() + "-->\n".len();
    format!("{}{}", &xaml[..start], &xaml[end..])
}

fn losses(result: &ImportResult) -> String {
    result
        .losses
        .iter()
        .map(|l| format!("{:?} {}: {}\n", l.kind, l.path, l.note))
        .collect()
}

fn lenient(result: &ImportResult, catalog: &Catalog) {
    let diagnostics = validate_document(
        &result.document,
        &ValidateOptions {
            catalog: Some(catalog),
            mode: Mode::Lenient,
            tokens: None,
            actions: None,
        },
    );
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

/// Compares `actual` with the golden file `name`, or rewrites it under `WEFT_UPDATE_FIXTURES`.
fn golden(name: &str, actual: &str) {
    let path = here(&format!("tests/fixtures/{name}"));
    if std::env::var_os("WEFT_UPDATE_FIXTURES").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(actual, expected, "{name}");
}

/// Kind and id of every element, slots included: the structure a page keeps without bindings.
fn shape(node: &Node, out: &mut Vec<String>) {
    out.push(format!(
        "{}#{}",
        node.kind,
        node.id.as_deref().unwrap_or("")
    ));
    let slots = node
        .slots
        .iter()
        .flat_map(|(name, list)| std::iter::once(Err(name.clone())).chain(list.iter().map(Ok)));
    for child in node.children.iter().map(Ok).chain(slots) {
        match child {
            Ok(Child::Node(n)) => shape(n, out),
            Ok(Child::Text(_)) => {}
            Err(slot) => out.push(format!("slot {slot}")),
        }
    }
    out.push("end".into());
}

#[test]
fn generated_pages_read_back_without_their_comment() {
    let (catalog, tokens) = setup();
    for (name, doc, generated) in covered() {
        let result = read(&bare(&generated.xaml), &catalog, &tokens);
        assert!(
            result.diagnostics.is_empty(),
            "{name}: {:?}",
            result.diagnostics
        );
        lenient(&result, &catalog);
        let (mut want, mut got) = (vec![], vec![]);
        shape(&weft_core::expand(&doc, &catalog).document.root, &mut want);
        shape(&result.document.root, &mut got);
        assert_eq!(got, want, "{name}");
        // Until T105.9 reads them, bindings, events and token references are the only losses,
        // with the empty label and the required value that stand in for bound ones.
        for loss in &result.losses {
            let listed = [
                LossKind::Bindings,
                LossKind::Actions,
                LossKind::Tokens,
                LossKind::Names,
            ];
            let stand_in = loss.kind == LossKind::Values
                && result
                    .losses
                    .iter()
                    .any(|l| l.kind == LossKind::Bindings && l.path.starts_with(&loss.path));
            assert!(listed.contains(&loss.kind) || stand_in, "{name}: {loss:?}");
        }
        if name == "controls" {
            golden("controls.read.weft", &serialize(&result.document));
            golden("controls.read.losses", &losses(&result));
        }
    }
}

#[test]
fn a_hand_written_page_reports_each_loss_at_its_path() {
    let (catalog, tokens) = setup();
    let source = std::fs::read_to_string(here("tests/fixtures/handwritten.xaml")).unwrap();
    let result = read(&source, &catalog, &tokens);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    lenient(&result, &catalog);
    golden("handwritten.read.weft", &serialize(&result.document));
    golden("handwritten.read.losses", &losses(&result));
    for kind in [
        LossKind::Ids,
        LossKind::Bindings,
        LossKind::Actions,
        LossKind::Tokens,
        LossKind::Layout,
        LossKind::Repetition,
        LossKind::Slots,
        LossKind::Props,
        LossKind::Values,
        LossKind::Kinds,
        LossKind::Text,
        LossKind::Structure,
    ] {
        assert!(result.losses.iter().any(|l| l.kind == kind), "{kind:?}");
    }
}

#[test]
fn ids_come_from_the_page_and_never_collide() {
    let (catalog, tokens) = setup();
    let page = |body: &str| {
        format!(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"><StackPanel AutomationProperties.AutomationId="s">{body}</StackPanel></Page>"#
        )
    };
    // A generated id avoids an id a later element states, which keeps it.
    let result = read(
        &page(r#"<Button Content="Go" /><Button x:Name="button-go" Content="Go" />"#),
        &catalog,
        &tokens,
    );
    let ids: Vec<_> = result
        .document
        .root
        .children
        .iter()
        .map(|c| match c {
            Child::Node(n) => n.id.clone().unwrap(),
            Child::Text(_) => panic!("text"),
        })
        .collect();
    assert_eq!(ids, ["button-go-2", "button-go"]);
    assert_eq!(result.document.root.id.as_deref(), Some("s"));
    let generated: Vec<_> = result
        .losses
        .iter()
        .map(|l| (l.kind, &l.path[..]))
        .collect();
    assert_eq!(generated, [(LossKind::Ids, "/screen#s/button#button-go-2")]);
    // A loose panel is the screen; a horizontal one sits in a generated screen.
    let loose = r#"<StackPanel xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Orientation="Horizontal" />"#;
    let result = read(loose, &catalog, &tokens);
    assert_eq!(
        serialize(&result.document),
        "<screen id=\"screen-1\" weft=\"0.3\">\n  <stack id=\"stack-1\" direction=\"row\"/>\n</screen>\n"
    );
}

fn diagnostic(result: &ImportResult) -> Code {
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    result.diagnostics[0].code
}

#[test]
fn unreadable_input_is_w601() {
    let (catalog, tokens) = setup();
    let ns = r#"xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation""#;
    let dtd = format!(
        "<!DOCTYPE Page [<!ENTITY lol \"lol\">]>\n<Page {ns}><TextBlock Text=\"&lol;\" /></Page>"
    );
    for source in [
        dtd.as_str(),
        "<Page",
        "not xml",
        "<html><body>Hi</body></html>",
    ] {
        let result = read(source, &catalog, &tokens);
        assert_eq!(diagnostic(&result), Code::W601, "{source}");
        assert_eq!(result.document.root.children, vec![]);
    }
}

#[test]
fn oversized_input_is_cut_with_w602() {
    let (catalog, tokens) = setup();
    let page = |body: String| {
        format!(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><StackPanel>{body}</StackPanel></Page>"#
        )
    };
    // Too long, or too many XML nodes: nothing is read.
    let long = page(" ".repeat(MAX_SOURCE_LENGTH));
    let result = read(&long, &catalog, &tokens);
    assert_eq!(diagnostic(&result), Code::W602);
    assert_eq!(result.document.root.children, vec![]);
    let nodes = page("<!---->".repeat(weft_import::MAX_NODES * 8));
    assert!(nodes.len() < MAX_SOURCE_LENGTH);
    assert_eq!(diagnostic(&read(&nodes, &catalog, &tokens)), Code::W602);
    // Too many elements, or nested too deep: what fits is read.
    let many = page("<TextBlock/>".repeat(weft_import::MAX_NODES + 5));
    let result = read(&many, &catalog, &tokens);
    assert_eq!(diagnostic(&result), Code::W602);
    assert_eq!(result.document.root.children.len(), weft_import::MAX_NODES);
}

#[test]
fn deep_input_is_cut_with_w602() {
    let (catalog, tokens) = setup();
    let page = |body: String| {
        format!(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><StackPanel>{body}</StackPanel></Page>"#
        )
    };
    // `Page` and the outer panel, then as deep as the parser is given: read, and cut.
    let depth = MAX_XML_DEPTH - 2;
    let deep = page(format!(
        "{}{}",
        "<StackPanel>".repeat(depth),
        "</StackPanel>".repeat(depth)
    ));
    let result = read(&deep, &catalog, &tokens);
    assert_eq!(diagnostic(&result), Code::W602);
    assert_eq!(result.document.root.children.len(), 1);
    // Validating 200 levels takes more than a debug test thread's stack.
    weft_import::on_reader_stack("lenient", || lenient(&result, &catalog)).unwrap();
    // Deeper than the parser is given: refused before parsing, wherever the nesting hides.
    for (open, close) in [
        ("<StackPanel>", "</StackPanel>"),
        (
            "<a:b xmlns:a='x' c='>'><!-- <d> --><![CDATA[<e>]]><f/>",
            "</a:b>",
        ),
    ] {
        let deep = page(format!(
            "{}{}",
            open.repeat(MAX_XML_DEPTH),
            close.repeat(MAX_XML_DEPTH)
        ));
        let result = read(&deep, &catalog, &tokens);
        assert_eq!(diagnostic(&result), Code::W602);
        assert_eq!(result.document.root.children, vec![]);
    }
}
