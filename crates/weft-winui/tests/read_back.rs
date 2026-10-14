//! WinUI → Weft for pages the generator wrote: every golden reads back to its document through
//! the source comment, also reformatted, and an edited, foreign or hostile page is rejected.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{controls, covered, document, options, screen, setup};
use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::Catalog;
use weft_winui::{
    ImportError, ImportOptions, MAX_SOURCE_LENGTH, MAX_XML_DEPTH, generate, import_winui,
};

fn import<'a>(catalog: &'a Catalog, tokens: &'a IndexMap<String, Token>) -> ImportOptions<'a> {
    ImportOptions { catalog, tokens }
}

#[test]
fn generated_xaml_reads_back_to_the_same_document() {
    let (catalog, tokens) = setup();
    for (name, doc, generated) in covered() {
        let back = import_winui(&generated.xaml, &import(&catalog, &tokens)).unwrap();
        assert_eq!(back, doc, "{name}");
        // Reindented, with its attributes on lines of their own and a comment added, it is the
        // same XML and still reads back.
        let reformatted = generated
            .xaml
            .replace("    ", "\t")
            .replace("\" ", "\"\n  ")
            .replace("<ScrollViewer>", "<ScrollViewer><!-- note -->");
        assert_ne!(reformatted, generated.xaml);
        let back = import_winui(&reformatted, &import(&catalog, &tokens)).unwrap();
        assert_eq!(back, doc, "{name}");
    }
}

#[test]
fn the_name_and_namespace_come_from_the_comment() {
    let (catalog, tokens) = setup();
    let doc = controls(&catalog, &tokens);
    let named = generate(&doc, &options(&catalog, &tokens, Some("sign-in"))).unwrap();
    let back = import_winui(&named.xaml, &import(&catalog, &tokens)).unwrap();
    assert_eq!(back, doc);
}

#[test]
fn hostile_text_reads_back() {
    let (catalog, tokens) = setup();
    let markup =
        screen(r#"<text id="t">{x:Bind ViewModel.secret} -- --> &lt;!-- "q" &amp;</text>"#);
    let doc = document(&markup, &catalog, &tokens);
    let generated = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    let back = import_winui(&generated.xaml, &import(&catalog, &tokens)).unwrap();
    assert_eq!(back, doc);
}

#[test]
fn an_edited_or_foreign_page_is_rejected() {
    let (catalog, tokens) = setup();
    let options = import(&catalog, &tokens);
    let (_, _, generated) = covered()
        .into_iter()
        .find(|(n, ..)| n == "controls")
        .unwrap();
    let xaml = generated.xaml;
    let edited = |from: &str, to: &str| {
        assert!(xaml.contains(from), "{from}");
        import_winui(&xaml.replacen(from, to, 1), &options)
    };
    assert!(matches!(
        edited("Text=\"Three\"", "Text=\"3\""),
        Err(ImportError::Edited)
    ));
    assert!(matches!(
        edited(">Three</heading>", ">3</heading>"),
        Err(ImportError::Edited)
    ));
    assert!(matches!(
        edited("Mode=TwoWay", "Mode=OneWay"),
        Err(ImportError::Edited)
    ));
    assert!(matches!(
        edited("using:WeftScreens", "using:Other"),
        Err(ImportError::Edited)
    ));
    assert!(matches!(
        edited("namespace=WeftScreens", "namespace=Other"),
        Err(ImportError::Edited)
    ));
    assert!(matches!(
        edited("weft:source winui", "weft:source slint"),
        Err(ImportError::NoSource)
    ));
    assert!(matches!(
        edited("<!-- weft:source", "<!-- source"),
        Err(ImportError::NoSource)
    ));
    assert!(matches!(
        edited("level=\"3\"", "level=\"9\""),
        Err(ImportError::Invalid(_))
    ));
    assert!(matches!(
        edited("</Page>", "</Page"),
        Err(ImportError::Xml(_))
    ));
    // A source comment after the root is not the page's.
    let moved = xaml.replacen("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n", "", 1);
    let (comment, rest) = moved.split_at(moved.find("<Page").unwrap());
    let after = format!("{rest}{comment}");
    assert!(matches!(
        import_winui(&after, &options),
        Err(ImportError::NoSource)
    ));
    let dtd = format!("<!DOCTYPE Page [<!ENTITY e \"x\">]>\n{moved}");
    assert!(matches!(
        import_winui(&dtd, &options),
        Err(ImportError::Xml(_))
    ));
    let long = format!("{xaml}{}", " ".repeat(MAX_SOURCE_LENGTH));
    assert!(matches!(
        import_winui(&long, &options),
        Err(ImportError::TooLong)
    ));
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let (catalog, tokens) = setup();
    let options = import(&catalog, &tokens);
    let (_, _, generated) = covered()
        .into_iter()
        .find(|(n, ..)| n == "controls")
        .unwrap();
    let nested = |depth: usize| {
        let grids = format!("{}{}", "<Grid>".repeat(depth), "</Grid>".repeat(depth));
        assert!(generated.xaml.contains("<ScrollViewer>"));
        let xaml = generated
            .xaml
            .replacen("<ScrollViewer>", &format!("<ScrollViewer>{grids}"), 1);
        import_winui(&xaml, &options)
    };
    // Within the bound the page parses, deeper than a test thread's stack allows, and is compared.
    assert!(matches!(
        nested(MAX_XML_DEPTH - 8),
        Err(ImportError::Edited)
    ));
    assert!(matches!(nested(MAX_XML_DEPTH), Err(ImportError::Xml(_))));
}
