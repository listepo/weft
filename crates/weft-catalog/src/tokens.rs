//! Design Tokens loader for the DTCG 2025.10 format
//! (https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/), ported from
//! packages/catalog/src/tokens.ts with the same subset, problem codes and messages.
//!
//! Supported: groups and tokens (`$value`), `$type` on a token or inherited from the nearest
//! ancestor group, the `$root` token name, and whole-value aliases written `{group.token}` (chains
//! allowed; an untyped token takes the type of the token it aliases). Other `$`-properties are
//! ignored. Not supported: `$extends`, JSON-pointer `$ref`, the resolver module, and aliases nested
//! inside composite values. Bad input is reported as problems, never as an error.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Map as Object, Value as Json};
use weft_core::order_keys;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Token {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TokenCode {
    T001,
    T002,
    T003,
    T004,
    T005,
    T006,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TokenProblem {
    pub code: TokenCode,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tokens {
    pub tokens: IndexMap<String, Token>,
    pub problems: Vec<TokenProblem>,
}

struct Raw {
    kind: Option<String>,
    value: Json,
}

enum State {
    Visiting,
    Failed,
    Done(Token),
}

/// The target of a whole-value alias `{group.token}`.
fn alias_target(value: &Json) -> Option<&str> {
    let inner = value.as_str()?.strip_prefix('{')?.strip_suffix('}')?;
    (!inner.is_empty() && !inner.contains(['{', '}'])).then_some(inner)
}

fn problem(problems: &mut Vec<TokenProblem>, code: TokenCode, path: &str, message: String) {
    problems.push(TokenProblem {
        code,
        path: path.to_owned(),
        message,
    });
}

fn walk(
    node: &Object<String, Json>,
    path: &[&str],
    inherited: Option<&str>,
    raw: &mut IndexMap<String, Raw>,
    problems: &mut Vec<TokenProblem>,
) {
    let own = node.get("$type");
    if own.is_some_and(|t| !t.is_string()) {
        problem(
            problems,
            TokenCode::T006,
            &path.join("."),
            "`$type` must be a string.".to_owned(),
        );
    }
    let kind = own.and_then(Json::as_str).or(inherited);
    for (name, child) in node {
        // `$root` is the only `$` name that is a token; the rest are format properties.
        if name.starts_with('$') && name != "$root" {
            continue;
        }
        let child_path: Vec<&str> = path.iter().copied().chain([name.as_str()]).collect();
        let joined = child_path.join(".");
        if name.contains(['{', '}', '.']) {
            let message = format!("Name \"{name}\" must not contain \"{{\", \"}}\" or \".\".");
            problem(problems, TokenCode::T002, &joined, message);
            continue;
        }
        match child {
            Json::Object(group) if group.contains_key("$value") => {
                let own = group.get("$type").and_then(Json::as_str);
                let value = group.get("$value").cloned().unwrap_or(Json::Null);
                raw.insert(
                    joined,
                    Raw {
                        kind: own.or(kind).map(str::to_owned),
                        value,
                    },
                );
            }
            Json::Object(group) => walk(group, &child_path, kind, raw, problems),
            _ => problem(
                problems,
                TokenCode::T006,
                &joined,
                "A token or group must be a JSON object.".to_owned(),
            ),
        }
    }
}

/// Resolves `start` and every alias it leads through. The TypeScript loader recurses; this walks
/// the chain with an explicit stack, so a long alias chain cannot overflow the native stack, and
/// reports the same problems in the same order.
fn resolve(
    start: &str,
    raw: &IndexMap<String, Raw>,
    state: &mut HashMap<String, State>,
    problems: &mut Vec<TokenProblem>,
) -> Option<Token> {
    let mut stack: Vec<&str> = Vec::new();
    let mut current = start;
    let mut inner = loop {
        match state.get(current) {
            Some(State::Visiting) => {
                let from = stack.iter().position(|p| *p == current).unwrap_or(0);
                let cycle: Vec<&str> = stack[from..].iter().copied().chain([current]).collect();
                let message = format!("Alias cycle: {}.", cycle.join(" -> "));
                problem(problems, TokenCode::T005, current, message);
                break None;
            }
            Some(State::Failed) => break None,
            Some(State::Done(token)) => break Some(token.clone()),
            None => {}
        }
        let Some(token) = raw.get(current) else {
            break None;
        };
        match alias_target(&token.value) {
            None => {
                let result = token.kind.as_ref().map(|kind| Token {
                    kind: kind.clone(),
                    value: token.value.clone(),
                });
                if result.is_none() {
                    let message =
                        "Token has no `$type` and none is inherited from a group.".to_owned();
                    problem(problems, TokenCode::T003, current, message);
                }
                let done = result.clone().map_or(State::Failed, State::Done);
                state.insert(current.to_owned(), done);
                break result;
            }
            Some(target) if !raw.contains_key(target) => {
                let message = format!("Alias {{{target}}} points at a token that does not exist.");
                problem(problems, TokenCode::T004, current, message);
                state.insert(current.to_owned(), State::Failed);
                break None;
            }
            Some(target) => {
                state.insert(current.to_owned(), State::Visiting);
                stack.push(current);
                current = target;
            }
        }
    };
    while let Some(path) = stack.pop() {
        let Some(token) = raw.get(path) else { continue };
        inner = match inner {
            Some(target) => {
                let kind = token.kind.clone().unwrap_or(target.kind);
                Some(Token {
                    kind,
                    value: target.value,
                })
            }
            None => {
                // The root cause was reported where it was found; this token is only a casualty.
                if !problems.iter().any(|p| p.path == path) {
                    let target = alias_target(&token.value).unwrap_or_default();
                    let message = format!("Alias {{{target}}} cannot be resolved.");
                    problem(problems, TokenCode::T004, path, message);
                }
                None
            }
        };
        let done = inner.clone().map_or(State::Failed, State::Done);
        state.insert(path.to_owned(), done);
    }
    inner
}

pub fn load_tokens(json: &Json) -> Tokens {
    let mut problems = Vec::new();
    // Object.entries lists integer-like names first; token names such as `100` depend on it.
    let Json::Object(root) = order_keys(json.clone()) else {
        problem(
            &mut problems,
            TokenCode::T001,
            "",
            "A tokens file must be a JSON object.".to_owned(),
        );
        return Tokens {
            tokens: IndexMap::new(),
            problems,
        };
    };
    let mut raw = IndexMap::new();
    walk(&root, &[], None, &mut raw, &mut problems);
    let mut state = HashMap::new();
    let mut tokens = IndexMap::new();
    for path in raw.keys() {
        if let Some(token) = resolve(path, &raw, &mut state, &mut problems) {
            tokens.insert(path.clone(), token);
        }
    }
    Tokens { tokens, problems }
}

/// Path to `$type`: the `tokens` option of the core's parse and validate.
pub fn token_types(tokens: &IndexMap<String, Token>) -> IndexMap<String, String> {
    tokens
        .iter()
        .map(|(path, token)| (path.clone(), token.kind.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn codes(tokens: &Tokens) -> Vec<(TokenCode, &str)> {
        tokens
            .problems
            .iter()
            .map(|p| (p.code, p.path.as_str()))
            .collect()
    }

    #[test]
    fn group_type_is_inherited_and_nested_groups_override_it() {
        let loaded = load_tokens(&json!({
            "space": { "$type": "dimension", "sm": { "$value": 4 }, "inner": { "$type": "number", "x": { "$value": 1 } } }
        }));
        assert!(loaded.problems.is_empty());
        let types = token_types(&loaded.tokens);
        assert_eq!(types["space.sm"], "dimension");
        assert_eq!(types["space.inner.x"], "number");
    }

    #[test]
    fn aliases_resolve_through_chains_and_take_the_target_type() {
        let loaded = load_tokens(&json!({
            "a": { "$type": "color", "$value": "#fff" },
            "b": { "$value": "{a}" },
            "c": { "$value": "{b}" }
        }));
        assert!(loaded.problems.is_empty());
        assert_eq!(
            loaded.tokens["c"],
            Token {
                kind: "color".into(),
                value: json!("#fff")
            }
        );
    }

    #[test]
    fn root_tokens_are_addressed_with_dollar_root() {
        let loaded =
            load_tokens(&json!({ "blue": { "$type": "color", "$root": { "$value": "#00f" } } }));
        assert!(loaded.tokens.contains_key("blue.$root"));
    }

    #[test]
    fn alias_cycles_report_the_cycle_and_drop_every_token_on_it() {
        let loaded = load_tokens(&json!({ "a": { "$value": "{b}" }, "b": { "$value": "{a}" } }));
        assert!(loaded.tokens.is_empty());
        assert_eq!(loaded.problems[0].message, "Alias cycle: a -> b -> a.");
        assert_eq!(
            codes(&loaded),
            [(TokenCode::T005, "a"), (TokenCode::T004, "b")]
        );
    }

    #[test]
    fn a_long_alias_chain_resolves_without_recursion() {
        let mut tokens = serde_json::Map::new();
        tokens.insert("t0".into(), json!({ "$type": "number", "$value": 1 }));
        for i in 1..200_000 {
            tokens.insert(
                format!("t{i}"),
                json!({ "$value": format!("{{t{}}}", i - 1) }),
            );
        }
        let loaded = load_tokens(&Json::Object(tokens));
        assert!(loaded.problems.is_empty());
        assert_eq!(loaded.tokens.len(), 200_000);
    }

    #[test]
    fn bad_input_is_reported_never_raised() {
        for bad in [Json::Null, json!(1), json!("x"), json!([])] {
            assert_eq!(load_tokens(&bad).problems[0].code, TokenCode::T001);
        }
        let loaded = load_tokens(&json!({
            "untyped": { "$value": 1 },
            "a.b": { "$type": "number", "$value": 1 },
            "junk": 5,
            "$type": 7
        }));
        assert!(loaded.tokens.is_empty());
        let mut got: Vec<TokenCode> = loaded.problems.iter().map(|p| p.code).collect();
        got.sort_by_key(|c| *c as u8);
        assert_eq!(
            got,
            [
                TokenCode::T002,
                TokenCode::T003,
                TokenCode::T006,
                TokenCode::T006
            ]
        );
    }

    #[test]
    fn integer_like_names_come_first_as_in_javascript() {
        let loaded = load_tokens(
            &json!({ "$type": "number", "b": { "$value": 1 }, "10": { "$value": 2 }, "2": { "$value": 3 } }),
        );
        assert_eq!(loaded.tokens.keys().collect::<Vec<_>>(), ["2", "10", "b"]);
    }
}
