//! A fragment checked on its own (SPEC §10.7): its declarations, and its body as a screen body
//! in which a parameter read counts as a value of the parameter's declared type.

use super::{At, Owner, Validator, diag, node_at, source};
use crate::diagnostics::{Code, did_you_mean, one_of};
use crate::fragment::{self, FRAGMENT, PARAM, ParamKind};
use crate::model::{Content, Node, PropDef, PropType, Value};
use crate::source::path_segment;

impl Validator<'_> {
    pub(super) fn visit_fragment(&mut self, root: &Node, path: &str) {
        for problem in fragment::problems(root) {
            let param = problem
                .param
                .and_then(|i| Some((i, root.children[i].as_node()?)));
            let (node, path) = match param {
                Some((i, node)) => (
                    node,
                    format!("{path}/{}", path_segment(PARAM, None, Some(i))),
                ),
                None => (root, path.to_owned()),
            };
            let at = node_at(node, &path, problem.attr.as_deref());
            self.report(diag(Code::W803, &at, problem.message, problem.expected));
        }
        self.in_fragment = true;
        self.params = fragment::params(root);
        let scope: Vec<String> = self.params.keys().cloned().collect();
        // The body stands wherever a use places it, so `submit` and a top-level `grow` are
        // judged there.
        let owner = Owner {
            in_form: true,
            ..Owner::new(Some(FRAGMENT.to_owned()), Content::Nodes)
        };
        let positions = source(root).map(|s| s.children.as_slice());
        self.visit_list(&root.children, path, &owner, &scope, positions);
    }

    /// A `<param>` or `<outlet>` below the top of a document.
    pub(super) fn visit_structural(&mut self, node: &Node, path: &str) {
        let at = node_at(node, path, None);
        let extra = node.id.is_some()
            || !(node.on.is_empty() && node.slots.is_empty() && node.children.is_empty())
            || node.props.keys().any(|k| k != "name");
        let (code, message, expected) = if node.kind == PARAM {
            let message = "<param> stands only at the top of a fragment.";
            (Code::W803, message, "<param> before the body")
        } else if !self.in_fragment {
            let message = "<outlet> stands only in a fragment.";
            (Code::W804, message, "a <slot> of a <use>")
        } else if extra {
            let message = "<outlet> takes only a name.";
            (Code::W804, message, "<outlet name=\"…\"/>")
        } else {
            return self.check_outlet(node, path);
        };
        self.report(diag(code, &at, message, expected));
    }

    fn check_outlet(&mut self, node: &Node, path: &str) {
        let slots: Vec<&String> = self
            .params
            .iter()
            .filter(|(_, kind)| matches!(kind, ParamKind::Slot(_)))
            .map(|(name, _)| name)
            .collect();
        let at = node_at(node, path, Some("name"));
        let name = match node.props.get("name") {
            Some(Value::String(name)) => name.clone(),
            _ => String::new(),
        };
        let message = if !slots.contains(&&name) {
            "<outlet> names no slot parameter."
        } else if self.outlets.contains(&name) {
            "This slot parameter already has an outlet."
        } else {
            return self.outlets.push(name);
        };
        let hint = did_you_mean(&name, slots.iter().copied());
        let d = diag(Code::W804, &at, message, one_of(slots.iter().copied()));
        self.report(d.got(name).hint_opt(hint));
    }

    /// A read of a parameter; false when the binding checks should stop.
    pub(super) fn check_param_read(
        &mut self,
        path: &str,
        name: &str,
        not: bool,
        def: Option<&PropDef>,
        at: &At,
    ) -> bool {
        let w807 = |message: String| (Code::W807, message, "a value parameter, read whole".into());
        let (code, message, expected) = match &self.params[name] {
            _ if path.len() != name.len() + 1 => w807(format!("\"{name}\" is read whole.")),
            ParamKind::Action => w807(format!("Action \"{name}\" goes in an on-* attribute.")),
            ParamKind::Slot(_) => w807(format!("Slot \"{name}\" goes in an <outlet>.")),
            ParamKind::Value(param) if not && param.kind != PropType::Boolean => {
                let message = "Only a boolean parameter takes a negated read.".into();
                (Code::W218, message, "a plain read".into())
            }
            ParamKind::Value(param) => match def.and_then(|d| fragment::mismatch(name, param, d)) {
                Some(problem) => problem,
                None => return true,
            },
        };
        let got = format!("{{{}{path}}}", if not { "!" } else { "" });
        self.report(diag(code, at, message, expected).got(got));
        false
    }

    /// `on-press="{$back}"` in a fragment forwards an action parameter: true when the value has
    /// that braced form there, and so is no action name.
    pub(super) fn check_action_read(&mut self, action: &str, at: &At) -> bool {
        let read = action.strip_prefix("{$").and_then(|a| a.strip_suffix('}'));
        let Some(name) = read.filter(|_| self.in_fragment) else {
            return false;
        };
        let actions: Vec<&String> = self
            .params
            .iter()
            .filter(|(_, kind)| matches!(kind, ParamKind::Action))
            .map(|(name, _)| name)
            .collect();
        let expected = one_of(actions.iter().map(|a| format!("{{${a}}}")));
        let d = match self.params.get(name) {
            Some(ParamKind::Action) => return true,
            Some(_) => {
                let message = format!("Parameter \"{name}\" is not an action.");
                diag(Code::W807, at, message, expected)
            }
            None => {
                let message = format!("No parameter \"{name}\" in this fragment.");
                let hint = did_you_mean(name, actions.iter().copied());
                diag(Code::W305, at, message, expected).hint_opt(hint)
            }
        };
        self.report(d.got(action));
        true
    }
}
