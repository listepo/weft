//! Design Tokens loader for the DTCG 2025.10 format
//! (https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/), ported from
//! packages/catalog/src/tokens.ts with the same subset, problem codes and messages.
//!
//! Supported: groups and tokens (`$value`), `$type` on a token or inherited from the nearest
//! ancestor group, the `$root` token name, and whole-value aliases written `{group.token}` (chains
//! allowed; an untyped token takes the type of the token it aliases). A `color` token that carries
//! the `dev.weft.material` extension is a `material` token (see `material`). Other `$`-properties
//! are ignored. Not supported: `$extends`, JSON-pointer `$ref` and the resolver module (the project
//! loader reads resolvers, `resolver.rs`), and aliases nested inside composite values, which
//! `composite_part` resolves for the generators. Bad input is reported as problems, never as an error.

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
    T007,
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
    /// The token's `dev.weft.material` extension, the only extension Weft reads.
    material: Option<Json>,
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
                let material = group
                    .get("$extensions")
                    .and_then(|e| e.get(MATERIAL_EXTENSION))
                    .cloned();
                raw.insert(
                    joined,
                    Raw {
                        kind: own.or(kind).map(str::to_owned),
                        value,
                        material,
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
                let result = match (&token.kind, &token.material) {
                    (Some(kind), Some(extension)) if kind == "color" => {
                        match material(&token.value, extension) {
                            Ok(value) => Some(Token {
                                kind: MATERIAL.to_owned(),
                                value,
                            }),
                            Err(message) => {
                                problem(problems, TokenCode::T007, current, message);
                                state.insert(current.to_owned(), State::Failed);
                                break None;
                            }
                        }
                    }
                    (Some(kind), _) => Some(Token {
                        kind: kind.clone(),
                        value: token.value.clone(),
                    }),
                    (None, _) => None,
                };
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

/// The `$type` Weft gives a colour token that carries `MATERIAL_EXTENSION`.
pub const MATERIAL: &str = "material";

/// The vendor key of the `$extensions` entry that makes a `color` token a material (DTCG 2025.10
/// §5.2.3: vendor keys, reverse domain notation recommended). The DTCG format has no material
/// type and requires `$type` to be one of its own, so the file stays a valid colour file that any
/// tool reads, with the blur radius in an extension every tool must keep.
pub const MATERIAL_EXTENSION: &str = "dev.weft.material";

/// The largest background blur, in px. Figma's blur radius is open ended and a browser accepts any
/// length, so this bound is Weft's: a token file is untrusted, and a radius in the thousands only
/// costs a renderer time.
pub const MAX_BLUR_PX: f64 = 100.0;

/// The value of a material token: `{ "tint": <the colour value>, "blur": <the dimension value> }`.
/// The tint's alpha is the material's opacity, so there is one way to say it; it must be in
/// `0..=1` as the Color Module requires. The blur is a DTCG dimension, `0..=MAX_BLUR_PX` px
/// (`rem` at 16 px, as the generators read it); anything else is an error and the token is left
/// out, never half used.
fn material(color: &Json, extension: &Json) -> Result<Json, String> {
    let bad = |what: &str| format!("A material token {what}.");
    if !(color.is_object() || color.is_string()) {
        return Err(bad("needs a colour as its value"));
    }
    if let Some(alpha) = color.get("alpha")
        && !alpha
            .as_f64()
            .is_some_and(|a| a.is_finite() && (0.0..=1.0).contains(&a))
    {
        return Err(bad("has a tint opacity (`alpha`) outside 0 to 1"));
    }
    let Some(blur) = extension.get("blur") else {
        return Err(bad("needs a `blur` in its `dev.weft.material` extension"));
    };
    let number = blur
        .get("value")
        .and_then(Json::as_f64)
        .filter(|n| n.is_finite());
    let px = match (number, blur.get("unit").and_then(Json::as_str)) {
        (Some(n), Some("px")) => n,
        (Some(n), Some("rem")) => n * 16.0,
        _ => return Err(bad("needs a `blur` that is a dimension in px or rem")),
    };
    if !(0.0..=MAX_BLUR_PX).contains(&px) {
        return Err(bad(&format!(
            "has a blur of {px} px; it must be between 0 and {MAX_BLUR_PX} px"
        )));
    }
    Ok(serde_json::json!({ "tint": color, "blur": blur }))
}

/// The tint colour and the blur radius in px of a loaded `material` token, `None` for anything
/// else. The loader has already bounded both, so a generator never re-checks them.
pub fn material_parts(token: &Token) -> Option<(&Json, f64)> {
    if token.kind != MATERIAL {
        return None;
    }
    let blur = token.value.get("blur")?;
    let n = blur.get("value")?.as_f64()?;
    let px = if blur.get("unit")?.as_str()? == "rem" {
        n * 16.0
    } else {
        n
    };
    Some((token.value.get("tint")?, px))
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

/// A part of a composite token's value, such as the `fontSize` of a typography token, with an
/// alias to a whole token resolved: the loader resolves whole-value aliases only.
pub fn composite_part(value: &Json, key: &str, tokens: &IndexMap<String, Token>) -> Option<Json> {
    let part = value.get(key)?;
    match alias_target(part) {
        Some(path) => tokens.get(path).map(|t| t.value.clone()),
        None => Some(part.clone()),
    }
}

/// A DTCG font weight, a number from 1 to 1000 or one of the names the format lists, as a number.
pub fn font_weight(value: &Json) -> Option<f64> {
    match value {
        Json::String(name) => Some(match name.as_str() {
            "thin" | "hairline" => 100.0,
            "extra-light" | "ultra-light" => 200.0,
            "light" => 300.0,
            "normal" | "regular" | "book" => 400.0,
            "medium" => 500.0,
            "semi-bold" | "demi-bold" => 600.0,
            "bold" => 700.0,
            "extra-bold" | "ultra-bold" => 800.0,
            "black" | "heavy" | "extra-black" | "ultra-black" => 900.0,
            _ => return None,
        }),
        other => other
            .as_f64()
            .filter(|n| n.is_finite() && (1.0..=1000.0).contains(n)),
    }
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

    fn glass(color: Json, blur: Json) -> Json {
        json!({ "g": {
            "$type": "color",
            "$value": color,
            "$extensions": { "dev.weft.material": { "blur": blur } }
        } })
    }

    #[test]
    fn a_color_with_the_material_extension_is_a_material() {
        let tint = json!({ "colorSpace": "srgb", "components": [1, 1, 1], "alpha": 0.28 });
        let loaded = load_tokens(&glass(tint.clone(), json!({ "value": 1, "unit": "rem" })));
        assert!(loaded.problems.is_empty());
        let (got, px) = material_parts(&loaded.tokens["g"]).unwrap();
        assert_eq!((got, px), (&tint, 16.0));
        // A plain colour is not a material, and an alias to a material is one.
        let both = load_tokens(&json!({
            "plain": { "$type": "color", "$value": "#fff" },
            "g": glass(json!("#fff"), json!({ "value": 8, "unit": "px" }))["g"],
            "again": { "$value": "{g}" }
        }));
        assert!(material_parts(&both.tokens["plain"]).is_none());
        assert_eq!(both.tokens["again"].kind, MATERIAL);
    }

    #[test]
    fn a_material_outside_its_bounds_is_dropped_with_t007() {
        let px = |n: f64| json!({ "value": n, "unit": "px" });
        let white =
            |alpha: f64| json!({ "colorSpace": "srgb", "components": [1, 1, 1], "alpha": alpha });
        for (color, blur) in [
            (white(1.0), px(100.1)),
            (white(1.0), px(-1.0)),
            (white(1.5), px(10.0)),
            (white(-0.1), px(10.0)),
            (white(0.5), json!({ "value": 10, "unit": "em" })),
            (white(0.5), json!(10)),
            (white(0.5), json!({ "value": 1e308, "unit": "rem" })),
            (json!(5), px(10.0)),
        ] {
            let loaded = load_tokens(&glass(color, blur));
            assert!(loaded.tokens.is_empty());
            assert_eq!(codes(&loaded), [(TokenCode::T007, "g")]);
        }
        let none = load_tokens(&json!({ "g": {
            "$type": "color", "$value": "#fff", "$extensions": { "dev.weft.material": {} }
        } }));
        assert_eq!(codes(&none), [(TokenCode::T007, "g")]);
        let edge = load_tokens(&glass(white(1.0), px(100.0)));
        assert!(edge.problems.is_empty());
    }
}
