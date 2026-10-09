//! Context entries in `weft explain` (SPEC §2.3): after the element they are about, and in every
//! comparison, so a reviewer sees each note an edit added, changed or removed.

#![allow(clippy::unwrap_used)]

mod common;

use common::{catalog, document, parse_lenient};
use weft_core::{explain, explain_changes, explain_with_context};

fn screen(entries: &str) -> String {
    format!(
        "<screen id=\"login\" weft=\"0.2\">\n  <context>\n{entries}  </context>\n  \
         <form id=\"f\" on-submit=\"auth.submit\">\n    \
         <button id=\"go\" submit=\"true\">Sign in</button>\n    \
         <slot name=\"footer\">\n      \
         <link id=\"reset\" on-press=\"nav.reset\">Forgot password?</link>\n    \
         </slot>\n  </form>\n</screen>\n"
    )
}

const BEFORE: &str = "    <entry id=\"why\" by=\"human\" kind=\"intent\" name=\"Ivan\">Returning users sign in.</entry>\n    \
<entry id=\"go-why\" by=\"agent\" for=\"go\" kind=\"decision\" name=\"m\">Primary, because it is the one action.</entry>\n    \
<entry id=\"where\" by=\"agent\" for=\"reset\" kind=\"question\" name=\"m\" status=\"open\">Dialog or screen?</entry>\n";

const AFTER: &str = "    <entry id=\"go-why\" by=\"agent\" for=\"go\" kind=\"decision\" name=\"m\">Primary: the one action.</entry>\n    \
<entry id=\"where\" by=\"agent\" for=\"reset\" kind=\"question\" name=\"m\" status=\"resolved\">Dialog or screen?</entry>\n    \
<entry id=\"n\" by=\"agent\" for=\"go\" kind=\"todo\" name=\"m\" status=\"open\">Check the copy.</entry>\n";

fn lines<T: ToString>(items: &[T]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

#[test]
fn entries_follow_the_readbacks_of_the_element_they_are_about() {
    let catalog = catalog();
    let doc = document(&screen(BEFORE));
    let with = explain_with_context(&doc, Some(&catalog));
    assert_eq!(
        lines(&with),
        [
            "screen#login context why: intent by human Ivan: Returning users sign in.",
            "form#f on-submit: runs action auth.submit",
            "button#go context go-why: decision by agent m: Primary, because it is the one action.",
            "link#reset on-press: runs action nav.reset",
            "link#reset context where: question (open) by agent m: Dialog or screen?",
        ]
    );
    assert_eq!(with[0].path, "/screen#login/context/entry#why");
    assert_eq!(
        lines(&explain(&doc, Some(&catalog))),
        [
            "form#f on-submit: runs action auth.submit",
            "link#reset on-press: runs action nav.reset",
        ]
    );
}

#[test]
fn comparisons_list_every_context_change() {
    let catalog = catalog();
    let before = document(&screen(BEFORE));
    let after = document(&screen(AFTER));
    assert_eq!(
        lines(&explain_changes(&before, &after, Some(&catalog))),
        [
            "button#go context go-why changed: was decision by agent m: Primary, because it is the one action.; now decision by agent m: Primary: the one action.",
            "button#go context n added: todo (open) by agent m: Check the copy.",
            "link#reset context where changed: was question (open) by agent m: Dialog or screen?; now question (resolved) by agent m: Dialog or screen?",
            "screen#login context why removed: was intent by human Ivan: Returning users sign in.",
        ]
    );
    assert!(explain_changes(&before, &before, Some(&catalog)).is_empty());
}

#[test]
fn an_entry_moved_to_another_element_is_one_change() {
    let catalog = catalog();
    let before = document(&screen(BEFORE));
    let moved = document(&screen(&BEFORE.replace("for=\"reset\"", "for=\"go\"")));
    assert_eq!(
        lines(&explain_changes(&before, &moved, Some(&catalog))),
        [
            "button#go context where changed: was question (open) by agent m: Dialog or screen?; now question (open) by agent m: Dialog or screen?"
        ]
    );
}

#[test]
fn an_entry_about_no_element_reads_back_against_the_screen_and_long_text_is_cut() {
    let catalog = catalog();
    let long = "a".repeat(600);
    let entries = format!(
        "    <entry id=\"gone\" by=\"agent\" for=\"removed\" kind=\"constraint\" name=\"m\">{long}</entry>\n"
    );
    let result = parse_lenient(&screen(&entries));
    let doc = result.document.unwrap();
    let with = explain_with_context(&doc, Some(&catalog));
    let last = with.last().unwrap().to_string();
    let expected = format!(
        "screen#login context gone: constraint by agent m: {}… (cut at 500 characters)",
        "a".repeat(500)
    );
    assert_eq!(last, expected);
    assert_eq!(
        with.last().unwrap().path,
        "/screen#login/context/entry#gone"
    );
}
