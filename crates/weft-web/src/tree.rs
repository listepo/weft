//! HTML text → a flat tree of elements and text, built by html5ever's WHATWG tree builder so a page
//! is read the way a browser reads it. Comments, doctypes and processing instructions are dropped;
//! a `<template>` holds its contents as children, since the importers read templates as markup.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{Attribute, LocalName, Namespace, ParseOpts, QualName, parse_document};

/// One node of a parsed page. Index 0 is the document; its children are the top-level nodes.
#[derive(Clone, Debug, PartialEq)]
pub enum HNode {
    Text(String),
    Element {
        /// Lower-cased, as every HTML tag name compares.
        name: String,
        /// In source order; a repeated attribute keeps its first value, as the standard says.
        attrs: Vec<(String, String)>,
        children: Vec<usize>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Dom {
    pub nodes: Vec<HNode>,
}

pub const DOCUMENT: usize = 0;

impl Dom {
    pub fn name(&self, i: usize) -> Option<&str> {
        match self.nodes.get(i) {
            Some(HNode::Element { name, .. }) => Some(name),
            _ => None,
        }
    }

    pub fn attr(&self, i: usize, key: &str) -> Option<&str> {
        match self.nodes.get(i) {
            Some(HNode::Element { attrs, .. }) => attrs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str()),
            _ => None,
        }
    }

    pub fn children(&self, i: usize) -> &[usize] {
        match self.nodes.get(i) {
            Some(HNode::Element { children, .. }) => children,
            _ => &[],
        }
    }

    pub fn elements(&self, i: usize) -> impl DoubleEndedIterator<Item = usize> + '_ {
        self.children(i)
            .iter()
            .copied()
            .filter(|&c| matches!(self.nodes.get(c), Some(HNode::Element { .. })))
    }

    pub fn text(&self, i: usize) -> Option<&str> {
        match self.nodes.get(i) {
            Some(HNode::Text(t)) => Some(t),
            _ => None,
        }
    }
}

pub fn parse_html(html: &str) -> Dom {
    let sink = Sink::default();
    let document = parse_document(sink, ParseOpts::default()).one(html);
    flatten(&document)
}

enum Data {
    Document,
    Element {
        attrs: RefCell<Vec<Attribute>>,
        template: Option<Handle>,
    },
    Text(RefCell<StrTendril>),
    Other,
}

struct Node {
    /// Empty for anything but an element; the tree builder asks only elements for names.
    name: QualName,
    data: Data,
    parent: Cell<Option<Weak<Node>>>,
    children: RefCell<Vec<Handle>>,
}

type Handle = Rc<Node>;

impl Node {
    fn new(name: QualName, data: Data) -> Handle {
        Rc::new(Node {
            name,
            data,
            parent: Cell::new(None),
            children: RefCell::new(Vec::new()),
        })
    }

    fn parent(&self) -> Option<Handle> {
        let weak = self.parent.take();
        let parent = weak.as_ref().and_then(Weak::upgrade);
        self.parent.set(weak);
        parent
    }
}

// Dropping a deeply nested page recursively would overflow the stack, so subtrees are released
// from an explicit stack instead.
impl Drop for Node {
    fn drop(&mut self) {
        let mut stack = std::mem::take(self.children.get_mut());
        if let Data::Element { template, .. } = &mut self.data
            && let Some(t) = template.take()
        {
            stack.push(t);
        }
        while let Some(node) = stack.pop() {
            if let Ok(mut node) = Rc::try_unwrap(node) {
                stack.append(node.children.get_mut());
                if let Data::Element { template, .. } = &mut node.data
                    && let Some(t) = template.take()
                {
                    stack.push(t);
                }
            }
        }
    }
}

fn empty_name() -> QualName {
    QualName::new(None, Namespace::from(""), LocalName::from(""))
}

struct Sink {
    document: Handle,
}

impl Default for Sink {
    fn default() -> Self {
        Sink {
            document: Node::new(empty_name(), Data::Document),
        }
    }
}

fn text_node(text: StrTendril) -> Handle {
    Node::new(empty_name(), Data::Text(RefCell::new(text)))
}

fn detach(target: &Handle) {
    if let Some(parent) = target.parent() {
        parent
            .children
            .borrow_mut()
            .retain(|c| !Rc::ptr_eq(c, target));
    }
    target.parent.set(None);
}

impl TreeSink for Sink {
    type Handle = Handle;
    type Output = Handle;
    type ElemName<'a> = &'a QualName;

    fn finish(self) -> Handle {
        self.document
    }

    // A page is imported whatever its errors; the standard says how each one is repaired.
    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> Handle {
        self.document.clone()
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> &'a QualName {
        &target.name
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        let template = flags
            .template
            .then(|| Node::new(empty_name(), Data::Document));
        Node::new(
            name,
            Data::Element {
                attrs: RefCell::new(attrs),
                template,
            },
        )
    }

    fn create_comment(&self, _text: StrTendril) -> Handle {
        Node::new(empty_name(), Data::Other)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Handle {
        Node::new(empty_name(), Data::Other)
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        let mut children = parent.children.borrow_mut();
        match child {
            NodeOrText::AppendText(text) => {
                if let Some(last) = children.last()
                    && let Data::Text(existing) = &last.data
                {
                    existing.borrow_mut().push_tendril(&text);
                    return;
                }
                let node = text_node(text);
                node.parent.set(Some(Rc::downgrade(parent)));
                children.push(node);
            }
            NodeOrText::AppendNode(node) => {
                node.parent.set(Some(Rc::downgrade(parent)));
                children.push(node);
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        if element.parent().is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}

    fn get_template_contents(&self, target: &Handle) -> Handle {
        match &target.data {
            Data::Element {
                template: Some(t), ..
            } => t.clone(),
            // The tree builder asks only templates; anything else gets a detached fragment, so
            // what it appends there is simply not part of the page.
            _ => Node::new(empty_name(), Data::Document),
        }
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, new_node: NodeOrText<Handle>) {
        let Some(parent) = sibling.parent() else {
            return;
        };
        if let NodeOrText::AppendNode(node) = &new_node {
            detach(node);
        }
        let mut children = parent.children.borrow_mut();
        let Some(at) = children.iter().position(|c| Rc::ptr_eq(c, sibling)) else {
            return;
        };
        let node = match new_node {
            NodeOrText::AppendText(text) => {
                if at > 0
                    && let Data::Text(existing) = &children[at - 1].data
                {
                    existing.borrow_mut().push_tendril(&text);
                    return;
                }
                text_node(text)
            }
            NodeOrText::AppendNode(node) => node,
        };
        node.parent.set(Some(Rc::downgrade(&parent)));
        children.insert(at, node);
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        if let Data::Element {
            attrs: existing, ..
        } = &target.data
        {
            let mut existing = existing.borrow_mut();
            for attr in attrs {
                if !existing.iter().any(|a| a.name == attr.name) {
                    existing.push(attr);
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &Handle) {
        detach(target);
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let moved = std::mem::take(&mut *node.children.borrow_mut());
        let mut children = new_parent.children.borrow_mut();
        for child in moved {
            child.parent.set(Some(Rc::downgrade(new_parent)));
            children.push(child);
        }
    }
}

fn attr_name(name: &QualName) -> String {
    let local = name.local.to_ascii_lowercase().to_string();
    match &name.prefix {
        Some(prefix) => format!("{prefix}:{local}"),
        None => local,
    }
}

/// The handles' tree as an index arena, without recursion: a page may nest arbitrarily deep.
fn flatten(document: &Handle) -> Dom {
    let mut dom = Dom {
        nodes: vec![HNode::Element {
            name: String::new(),
            attrs: Vec::new(),
            children: Vec::new(),
        }],
    };
    let mut stack = vec![(document.clone(), DOCUMENT)];
    while let Some((handle, at)) = stack.pop() {
        let source = match &handle.data {
            Data::Element {
                template: Some(t), ..
            } => t.clone(),
            _ => handle.clone(),
        };
        let mut ids = Vec::new();
        for child in source.children.borrow().iter() {
            let node = match &child.data {
                Data::Text(t) => HNode::Text(t.borrow().to_string()),
                Data::Element { attrs, .. } => HNode::Element {
                    name: child.name.local.to_ascii_lowercase().to_string(),
                    attrs: attrs
                        .borrow()
                        .iter()
                        .map(|a| (attr_name(&a.name), a.value.to_string()))
                        .collect(),
                    children: Vec::new(),
                },
                Data::Document | Data::Other => continue,
            };
            let id = dom.nodes.len();
            if matches!(node, HNode::Element { .. }) {
                stack.push((child.clone(), id));
            }
            dom.nodes.push(node);
            ids.push(id);
        }
        if let Some(HNode::Element { children, .. }) = dom.nodes.get_mut(at) {
            *children = ids;
        }
    }
    dom
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(dom: &Dom, i: usize) -> Vec<String> {
        dom.children(i)
            .iter()
            .map(|&c| match &dom.nodes[c] {
                HNode::Text(t) => format!("#{t}"),
                HNode::Element { name, .. } => name.clone(),
            })
            .collect()
    }

    #[test]
    fn pages_get_the_tree_a_browser_builds() {
        let dom = parse_html("<p>a<!-- c -->b<div>x</div><TEMPLATE><i>t</i></template>");
        let html = dom.elements(DOCUMENT).next().unwrap();
        assert_eq!(dom.name(html), Some("html"));
        let body = dom.elements(html).nth(1).unwrap();
        // The div closes the paragraph; the comment is dropped and leaves two text nodes.
        assert_eq!(names(&dom, body), ["p", "div", "template"]);
        let p = dom.children(body)[0];
        assert_eq!(names(&dom, p), ["#a", "#b"]);
        let template = dom.children(body)[2];
        assert_eq!(names(&dom, template), ["i"]);
    }

    #[test]
    fn the_first_of_repeated_attributes_wins() {
        let dom = parse_html(r#"<a href="1" HREF="2" data-X="y">"#);
        let body = dom
            .elements(dom.elements(DOCUMENT).next().unwrap())
            .nth(1)
            .unwrap();
        let a = dom.children(body)[0];
        assert_eq!(dom.attr(a, "href"), Some("1"));
        assert_eq!(dom.attr(a, "data-x"), Some("y"));
    }

    #[test]
    fn deep_pages_parse_and_drop() {
        let html = format!("{}x{}", "<div>".repeat(5_000), "</div>".repeat(5_000));
        let dom = parse_html(&html);
        assert!(dom.nodes.len() > 5_000);
    }
}
