//! Markup → canonical JSON (SPEC §2–§4): the tokenizer checks syntax, the builder types literals
//! by the catalog and lifts `<slot>` into `slots`, and validation adds the schema and semantic
//! layers.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use crate::canonical::{Parts, append_text, assemble_node, normalize_text};
use crate::context::{misplaced_context, misplaced_entry, read_block};
use crate::diagnostics::{Code, Diagnostic, Mode, Position, has_errors, sort_by_position};
use crate::model::{Catalog, Child, Document, Entry, Node, PropType};
use crate::rules::{CONTEXT, EACH, ENTRY, SLOT, is_name, universal_prop};
use crate::source::{ListSource, NodeSource, Source};
use crate::syntax::{RawChild, RawElement, SyntaxResult, tokenize, tokenize_with};
use crate::validate::{ValidateOptions, validate_document};
use crate::values::read_value;

/// Without a catalog only the syntax layer runs and every literal stays a string.
#[derive(Clone, Copy, Default)]
pub struct ParseOptions<'a> {
    pub catalog: Option<&'a Catalog>,
    pub mode: Mode,
    pub tokens: Option<&'a IndexMap<String, String>>,
    pub actions: Option<&'a [String]>,
}

pub struct ParseResult {
    /// Absent when the markup has syntax errors. Nodes carry their markup positions.
    pub document: Option<Document>,
    pub diagnostics: Vec<Diagnostic>,
    /// Partial mode only: problems that the rest of the stream can still fix, such as an element
    /// that is not closed yet. Empty for a document that is complete.
    pub pending: Vec<Diagnostic>,
}

/// Checks that need more of the document than has arrived, so the cut explains them.
fn waits_for_more(d: &Diagnostic, open: &HashSet<String>) -> bool {
    match d.code {
        // The target may come later in the stream.
        Code::W309 => true,
        // The slot or the repeated element may be the next thing written inside it.
        Code::W208 | Code::W314 => open.contains(&d.path),
        _ => false,
    }
}

pub fn parse(markup: &str, options: &ParseOptions<'_>) -> ParseResult {
    parse_with(markup, options, false)
}

/// `parse` for markup that may stop anywhere, because a model is still writing it (SPEC §6.3):
/// what is finished becomes the document, and what the cut leaves open is reported in `pending`.
pub fn parse_partial(markup: &str, options: &ParseOptions<'_>) -> ParseResult {
    parse_with(markup, options, true)
}

fn parse_with(markup: &str, options: &ParseOptions<'_>, partial: bool) -> ParseResult {
    let mut syntax = tokenize_with(markup, partial);
    let mut pending = std::mem::take(&mut syntax.pending);
    let Some(root) = syntax.root.filter(|_| !has_errors(&syntax.diagnostics)) else {
        return ParseResult {
            document: None,
            diagnostics: syntax.diagnostics,
            pending,
        };
    };
    let (document, mut diagnostics, open) = build_open(&syntax, root, options.catalog);
    if has_errors(&diagnostics) {
        sort_by_position(&mut diagnostics);
        return ParseResult {
            document: None,
            diagnostics,
            pending,
        };
    }
    if options.catalog.is_none() {
        return ParseResult {
            document: Some(document),
            diagnostics: vec![],
            pending,
        };
    }
    let validate = ValidateOptions {
        catalog: options.catalog,
        mode: options.mode,
        tokens: options.tokens,
        actions: options.actions,
    };
    let mut diagnostics = validate_document(&document, &validate);
    if partial {
        let (wait, rest): (Vec<_>, Vec<_>) = diagnostics
            .into_iter()
            .partition(|d| waits_for_more(d, &open));
        diagnostics = rest;
        pending.extend(wait);
        sort_by_position(&mut pending);
    }
    ParseResult {
        document: Some(document),
        diagnostics,
        pending,
    }
}

const FRAGMENT: &str = "x-weft-fragment";

/// Parses markup that has no `<screen>` root (several sibling elements, for patch `insert`) by
/// wrapping it in a throwaway element, so the tokenizer, literal typing and slot handling stay the
/// single implementation. Returns the wrapper: its `children` are the fragment. The wrapper is
/// hidden from diagnostics: its path segment is dropped and line 1 columns are shifted back.
pub fn parse_fragment(markup: &str, catalog: &Catalog) -> (Option<Node>, Vec<Diagnostic>) {
    let open = format!("<{FRAGMENT}>");
    let syntax = tokenize(&format!("{open}{markup}</{FRAGMENT}>"));
    let built = syntax
        .root
        .filter(|_| !has_errors(&syntax.diagnostics))
        .map(|root| build(&syntax, root, Some(catalog)));
    let (wrapper, mut diagnostics) = match built {
        Some((document, diagnostics)) => (Some(document.root), diagnostics),
        None => (None, syntax.diagnostics),
    };
    sort_by_position(&mut diagnostics);
    let prefix = format!("/{FRAGMENT}");
    let shift = u32::try_from(open.encode_utf16().count()).unwrap_or(0);
    for d in &mut diagnostics {
        if let Some(rest) = d.path.strip_prefix(&prefix) {
            d.path = if rest.is_empty() {
                "/".to_owned()
            } else {
                rest.to_owned()
            };
        }
        if d.line == Some(1) {
            d.column = d.column.map(|c| c.saturating_sub(shift));
        }
    }
    if has_errors(&diagnostics) {
        return (None, diagnostics);
    }
    (wrapper, diagnostics)
}

fn prop_type(catalog: Option<&Catalog>, kind: &str, name: &str) -> Option<PropType> {
    // Extension and unknown elements keep every literal a string (SPEC §3).
    if kind == EACH {
        return None;
    }
    let component = catalog?.components.get(kind)?;
    component
        .prop(name)
        .or_else(|| universal_prop(name))
        .map(|d| d.kind)
}

struct Builder<'a> {
    syntax: &'a SyntaxResult,
    catalog: Option<&'a Catalog>,
    diagnostics: Vec<Diagnostic>,
    weft: String,
    /// The root's `<context>` block, lifted out of its content (SPEC §2.3).
    context: Option<Vec<Entry>>,
    /// Paths of the elements the end of a partial input left open.
    open: HashSet<String>,
}

impl Builder<'_> {
    fn element(&mut self, index: usize, path: &str, is_root: bool) -> Node {
        let syntax = self.syntax;
        let raw = &syntax.elements[index];
        if syntax.open.contains(&index) {
            self.open.insert(path.to_owned());
        }
        if raw.name == CONTEXT {
            self.diagnostics.push(
                misplaced_context(
                    raw.pos,
                    path,
                    "A <context> can only be a direct child of the root <screen>.",
                )
                .hint("move the entries into the <context> block under <screen>"),
            );
        } else if raw.name == ENTRY {
            self.diagnostics.push(
                misplaced_entry(raw.pos, path, "An <entry> can only stand inside <context>.")
                    .hint("move the <entry> into the <context> block under <screen>"),
            );
        }
        // Insert markup is parsed under a wrapper root, where a block is as misplaced as anywhere.
        let lifts_context = is_root && raw.name != FRAGMENT && raw.name != CONTEXT;
        let kind = raw.name.clone();
        let mut id = None;
        let mut props = Vec::new();
        let mut on = Vec::new();
        let mut attrs: HashMap<String, Position> = HashMap::new();
        for attr in &raw.attrs {
            attrs.insert(attr.name.clone(), attr.pos);
            if attr.name == "id" {
                id = Some(attr.value.clone());
            } else if let Some(event) = attr.name.strip_prefix("on-") {
                on.push((event.to_owned(), attr.value.clone()));
            } else if is_root && attr.name == "weft" {
                self.weft = attr.value.clone();
            } else {
                match read_value(&attr.value, prop_type(self.catalog, &kind, &attr.name)) {
                    Ok(value) => props.push((attr.name.clone(), value)),
                    Err(bad) => self.diagnostics.push(
                        Diagnostic::new(
                            Code::W116,
                            format!("{path}/@{}", attr.name),
                            bad.message,
                            "{$…}, {!$…}, {token.…} or a literal starting with {{",
                        )
                        .pos(Some(attr.pos))
                        .got(attr.value.clone())
                        .hint(bad.hint),
                    ),
                }
            }
        }

        let mut children = Vec::new();
        let mut child_positions = Vec::new();
        let mut slots: Vec<(String, Vec<Child>, ListSource)> = Vec::new();
        let mut seen_slots: HashSet<String> = HashSet::new();
        for child in &raw.children {
            let child_index = match child {
                RawChild::Text { text, pos } => {
                    if append_text(&mut children, normalize_text(text)) {
                        child_positions.push(Some(*pos));
                    }
                    continue;
                }
                RawChild::Element(i) => *i,
            };
            let child_raw = &syntax.elements[child_index];
            if lifts_context && child_raw.name == CONTEXT {
                let first = self.context.is_none();
                let entries = self.context.get_or_insert_with(Vec::new);
                read_block(
                    syntax,
                    child_raw,
                    path,
                    first,
                    entries,
                    &mut self.diagnostics,
                );
                continue;
            }
            if child_raw.name != SLOT {
                let child_path = format!("{path}/{}", child_raw.segment());
                let pos = child_raw.pos;
                children.push(Child::Node(Box::new(self.element(
                    child_index,
                    &child_path,
                    false,
                ))));
                child_positions.push(Some(pos));
                continue;
            }
            let Some(slot) = self.read_slot(child_raw, raw, path, &mut seen_slots) else {
                continue;
            };
            let mut list = Vec::new();
            let mut positions = Vec::new();
            let slot_path = format!("{path}/slot[{slot}]");
            for grandchild in &child_raw.children {
                match grandchild {
                    RawChild::Text { text, pos } => {
                        if append_text(&mut list, normalize_text(text)) {
                            positions.push(Some(*pos));
                        }
                    }
                    RawChild::Element(g) => {
                        let g_raw = &syntax.elements[*g];
                        if g_raw.name == SLOT {
                            self.misplaced_slot(
                                g_raw.pos,
                                &slot_path,
                                "A <slot> cannot be placed directly inside another <slot>.",
                            );
                        } else {
                            let g_path = format!("{slot_path}/{}", g_raw.segment());
                            let pos = g_raw.pos;
                            list.push(Child::Node(Box::new(self.element(*g, &g_path, false))));
                            positions.push(Some(pos));
                        }
                    }
                }
            }
            slots.push((
                slot,
                list,
                ListSource {
                    pos: child_raw.pos,
                    children: positions,
                },
            ));
        }

        let mut node = assemble_node(Parts {
            kind,
            id,
            props,
            on,
            slots: slots
                .iter()
                .map(|(name, list, _)| (name.clone(), list.clone()))
                .collect(),
            children,
        });
        node.source = Source(Some(Box::new(NodeSource {
            pos: raw.pos,
            attrs,
            children: child_positions,
            slots: slots
                .into_iter()
                .map(|(name, _, source)| (name, source))
                .collect(),
        })));
        node
    }

    fn misplaced_slot(&mut self, pos: Position, path: &str, message: &str) {
        self.diagnostics.push(
            Diagnostic::new(
                Code::W118,
                format!("{path}/slot"),
                message,
                "a <slot> directly inside a component",
            )
            .pos(Some(pos))
            .hint("make the <slot> a direct child of the component that declares it"),
        );
    }

    fn read_slot(
        &mut self,
        raw: &RawElement,
        parent: &RawElement,
        parent_path: &str,
        seen: &mut HashSet<String>,
    ) -> Option<String> {
        if parent.name == EACH {
            self.misplaced_slot(
                raw.pos,
                parent_path,
                "A <slot> cannot be placed directly inside <each>.",
            );
            return None;
        }
        let name = raw.attr("name").map(|a| a.value.clone());
        let extra = raw.attrs.iter().find(|a| a.name != "name");
        let name = match (name, extra) {
            (Some(name), None) if is_name(&name) => name,
            (name, extra) => {
                let message = match extra {
                    None => "A <slot> needs a `name` that matches the name grammar.".to_owned(),
                    Some(e) => format!("A <slot> takes only `name`, not \"{}\".", e.name),
                };
                self.diagnostics.push(
                    Diagnostic::new(
                        Code::W118,
                        format!("{parent_path}/slot"),
                        message,
                        "<slot name=\"…\">",
                    )
                    .pos(Some(extra.map_or(raw.pos, |e| e.pos)))
                    .got_opt(name),
                );
                return None;
            }
        };
        if !seen.insert(name.clone()) {
            self.diagnostics.push(
                Diagnostic::new(
                    Code::W119,
                    format!("{parent_path}/slot[{name}]"),
                    format!("Slot \"{name}\" appears twice under one parent."),
                    "each slot name once per parent",
                )
                .pos(Some(raw.pos))
                .hint(format!(
                    "merge both <slot name=\"{name}\"> elements into one"
                )),
            );
            return None;
        }
        Some(name)
    }
}

fn build(
    syntax: &SyntaxResult,
    root: usize,
    catalog: Option<&Catalog>,
) -> (Document, Vec<Diagnostic>) {
    let (document, diagnostics, _) = build_open(syntax, root, catalog);
    (document, diagnostics)
}

/// `build`, plus the paths of the elements still open at the end of a partial input.
fn build_open(
    syntax: &SyntaxResult,
    root: usize,
    catalog: Option<&Catalog>,
) -> (Document, Vec<Diagnostic>, HashSet<String>) {
    let mut b = Builder {
        syntax,
        catalog,
        diagnostics: vec![],
        weft: String::new(),
        context: None,
        open: HashSet::new(),
    };
    let raw_root = &syntax.elements[root];
    let root = if raw_root.name == SLOT {
        b.misplaced_slot(raw_root.pos, "", "The root element cannot be a <slot>.");
        Node::new(SLOT)
    } else {
        let path = format!("/{}", raw_root.segment());
        b.element(root, &path, true)
    };
    let document = Document {
        weft: b.weft,
        context: b.context.unwrap_or_default(),
        root,
    };
    (document, b.diagnostics, b.open)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::stringify;

    #[test]
    fn markup_without_a_catalog_keeps_literals_as_strings() {
        let r = parse(
            "<screen id=\"s\" weft=\"0.1\" n=\"2\"><slot name=\"a\"><x/></slot> hi </screen>",
            &ParseOptions::default(),
        );
        assert!(r.diagnostics.is_empty());
        let doc = r.document.map(|d| stringify(&d)).unwrap_or_default();
        assert!(doc.contains("\"n\": \"2\""), "{doc}");
        assert!(doc.contains("\"slots\""), "{doc}");
        assert!(doc.contains("\"hi\""), "{doc}");
    }

    #[test]
    fn slot_mistakes_are_reported() {
        let codes = |m: &str| {
            parse(m, &ParseOptions::default())
                .diagnostics
                .iter()
                .map(|d| d.code)
                .collect::<Vec<_>>()
        };
        assert_eq!(codes("<slot/>"), [Code::W118]);
        assert_eq!(
            codes("<a><slot name=\"x\"/><slot name=\"x\"/></a>"),
            [Code::W119]
        );
        assert_eq!(codes("<a><slot name=\"x\" y=\"1\"/></a>"), [Code::W118]);
        assert_eq!(codes("<a b=\"{oops}\"/>"), [Code::W116]);
    }

    #[test]
    fn fragment_diagnostics_hide_the_wrapper() {
        let catalog = Catalog {
            weft: "0.1".into(),
            name: "t".into(),
            version: "1".into(),
            components: Default::default(),
        };
        let (wrapper, diagnostics) = parse_fragment("<a b='1'/>", &catalog);
        assert!(wrapper.is_none());
        let d = &diagnostics[0];
        assert_eq!((d.path.as_str(), d.column), ("/a/@b", Some(6)));
    }
}
