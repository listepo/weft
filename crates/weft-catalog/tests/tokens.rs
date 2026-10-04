//! Claims about the Design Tokens loader: what is a token, how types and aliases resolve, and
//! which problem each kind of bad input is reported as. Bad input is never an error.

#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_catalog::{TokenCode, Tokens, load_tokens, token_types};

fn problems(loaded: &Tokens) -> Vec<(TokenCode, &str)> {
    loaded
        .problems
        .iter()
        .map(|p| (p.code, p.path.as_str()))
        .collect()
}

fn names(loaded: &Tokens) -> Vec<&str> {
    loaded.tokens.keys().map(String::as_str).collect()
}

fn kind(loaded: &Tokens, path: &str) -> String {
    loaded.tokens[path].kind.clone()
}

// ---- the file ----------------------------------------------------------------------------

#[test]
fn a_file_that_is_not_an_object_is_t001_at_the_root() {
    for bad in [json!(null), json!([]), json!("x"), json!(1), json!(true)] {
        let loaded = load_tokens(&bad);
        assert_eq!(problems(&loaded), [(TokenCode::T001, "")], "{bad}");
        assert!(loaded.tokens.is_empty());
        assert_eq!(
            loaded.problems[0].message,
            "A tokens file must be a JSON object."
        );
    }
}

#[test]
fn an_empty_file_has_no_tokens_and_no_problems() {
    let loaded = load_tokens(&json!({}));
    assert!(loaded.tokens.is_empty() && loaded.problems.is_empty());
    assert_eq!(loaded, Tokens::default());
}

// ---- tokens and groups -------------------------------------------------------------------

#[test]
fn a_token_is_an_object_with_a_value_and_its_path_joins_group_names_with_dots() {
    let loaded = load_tokens(&json!({
        "space": {"$type": "dimension", "sm": {"$value": "4px"}, "deep": {"x": {"$value": 1}}}
    }));
    assert!(loaded.problems.is_empty());
    assert_eq!(names(&loaded), ["space.sm", "space.deep.x"]);
    assert_eq!(loaded.tokens["space.sm"].value, json!("4px"));
    assert_eq!(loaded.tokens["space.deep.x"].value, json!(1));
}

#[test]
fn a_value_may_be_any_json_including_null_and_composites() {
    let loaded = load_tokens(&json!({
        "$type": "shadow",
        "a": {"$value": null},
        "b": {"$value": {"x": [1, 2], "y": {"z": true}}},
        "c": {"$value": []}
    }));
    assert!(loaded.problems.is_empty());
    assert_eq!(loaded.tokens["a"].value, Json::Null);
    assert_eq!(
        loaded.tokens["b"].value,
        json!({"x": [1, 2], "y": {"z": true}})
    );
    assert_eq!(loaded.tokens["c"].value, json!([]));
}

#[test]
fn an_object_with_a_value_is_a_token_even_if_it_has_members_that_look_like_children() {
    let loaded = load_tokens(&json!({
        "$type": "number",
        "t": {"$value": 1, "child": {"$value": 2}}
    }));
    assert_eq!(names(&loaded), ["t"]);
    assert!(loaded.problems.is_empty());
}

#[test]
fn tokens_come_out_in_document_order_but_integer_like_names_come_first() {
    let loaded = load_tokens(&json!({
        "$type": "number",
        "b": {"$value": 1},
        "10": {"$value": 2},
        "a": {"$value": 3},
        "2": {"$value": 4},
        "01": {"$value": 5}
    }));
    // The order of a JavaScript object's keys: array indices ascending, then the rest as written.
    assert_eq!(names(&loaded), ["2", "10", "b", "a", "01"]);
}

#[test]
fn format_properties_are_ignored_and_root_is_a_token_called_dollar_root() {
    let loaded = load_tokens(&json!({
        "$schema": "x", "$description": "d", "$extensions": {"a": 1}, "$extends": "{z}",
        "color": {
            "$type": "color", "$description": "d",
            "$root": {"$value": "#fff"},
            "light": {"$value": "#eee", "$description": "x", "$extensions": {}}
        }
    }));
    assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
    assert_eq!(names(&loaded), ["color.$root", "color.light"]);
    assert_eq!(kind(&loaded, "color.$root"), "color");
}

#[test]
fn a_dollar_root_group_is_walked_like_any_other_group() {
    let loaded = load_tokens(&json!({"g": {"$type": "number", "$root": {"x": {"$value": 1}}}}));
    assert_eq!(names(&loaded), ["g.$root.x"]);
}

// ---- types -------------------------------------------------------------------------------

#[test]
fn the_type_is_the_nearest_one_on_the_token_or_an_ancestor_group() {
    let loaded = load_tokens(&json!({
        "$type": "color",
        "a": {"$value": 1},
        "g": {
            "$type": "dimension",
            "b": {"$value": 2},
            "c": {"$value": 3, "$type": "number"},
            "inner": {"d": {"$value": 4}, "deeper": {"$type": "duration", "e": {"$value": 5}}}
        }
    }));
    assert!(loaded.problems.is_empty());
    assert_eq!(kind(&loaded, "a"), "color");
    assert_eq!(kind(&loaded, "g.b"), "dimension");
    assert_eq!(kind(&loaded, "g.c"), "number");
    assert_eq!(kind(&loaded, "g.inner.d"), "dimension");
    assert_eq!(kind(&loaded, "g.inner.deeper.e"), "duration");
}

#[test]
fn a_token_without_any_type_is_t003_and_is_left_out() {
    let loaded =
        load_tokens(&json!({"a": {"$value": 1}, "g": {"$type": "number", "b": {"$value": 1}}}));
    assert_eq!(problems(&loaded), [(TokenCode::T003, "a")]);
    assert_eq!(
        loaded.problems[0].message,
        "Token has no `$type` and none is inherited from a group."
    );
    assert_eq!(names(&loaded), ["g.b"]);
}

#[test]
fn a_type_that_is_not_a_string_on_a_group_is_t006_and_the_inherited_type_stays() {
    let loaded = load_tokens(&json!({"$type": "number", "g": {"$type": 5, "a": {"$value": 1}}}));
    assert_eq!(problems(&loaded), [(TokenCode::T006, "g")]);
    assert_eq!(loaded.problems[0].message, "`$type` must be a string.");
    assert_eq!(kind(&loaded, "g.a"), "number");
}

#[test]
fn a_type_that_is_not_a_string_on_a_token_itself_is_ignored_without_a_problem() {
    // Only groups are checked for it; the token falls back to the type it would have inherited.
    let loaded = load_tokens(&json!({"$type": "number", "a": {"$value": 1, "$type": 5}}));
    assert!(loaded.problems.is_empty());
    assert_eq!(kind(&loaded, "a"), "number");
    let loaded = load_tokens(&json!({"a": {"$value": 1, "$type": 5}}));
    assert_eq!(problems(&loaded), [(TokenCode::T003, "a")]);
}

#[test]
fn a_type_that_is_not_a_string_on_the_file_is_t006_at_the_root() {
    let loaded = load_tokens(&json!({"$type": ["x"], "a": {"$value": 1}}));
    assert_eq!(
        problems(&loaded),
        [(TokenCode::T006, ""), (TokenCode::T003, "a")]
    );
}

// ---- names and members -------------------------------------------------------------------

#[test]
fn a_name_with_a_dot_or_a_brace_is_t002_and_everything_under_it_is_skipped() {
    let loaded = load_tokens(&json!({
        "$type": "number",
        "a.b": {"$value": 1},
        "c{": {"$value": 1},
        "d}": {"x": {"$value": 1}},
        "ok": {"$value": 1},
        "g": {"in.valid": {"$value": 1}, "fine": {"$value": 2}}
    }));
    assert_eq!(
        problems(&loaded),
        [
            (TokenCode::T002, "a.b"),
            (TokenCode::T002, "c{"),
            (TokenCode::T002, "d}"),
            (TokenCode::T002, "g.in.valid"),
        ]
    );
    assert_eq!(names(&loaded), ["ok", "g.fine"]);
    assert_eq!(
        loaded.problems[0].message,
        "Name \"a.b\" must not contain \"{\", \"}\" or \".\"."
    );
}

#[test]
fn a_member_that_is_not_an_object_is_t006() {
    let loaded = load_tokens(&json!({
        "$type": "number", "a": 5, "b": null, "c": [], "d": "x", "e": true, "ok": {"$value": 1}
    }));
    assert_eq!(
        problems(&loaded),
        [
            (TokenCode::T006, "a"),
            (TokenCode::T006, "b"),
            (TokenCode::T006, "c"),
            (TokenCode::T006, "d"),
            (TokenCode::T006, "e"),
        ]
    );
    assert_eq!(
        loaded.problems[0].message,
        "A token or group must be a JSON object."
    );
    assert_eq!(names(&loaded), ["ok"]);
}

#[test]
fn an_empty_group_is_fine() {
    let loaded = load_tokens(&json!({"g": {}, "h": {"i": {}}}));
    assert!(loaded.problems.is_empty() && loaded.tokens.is_empty());
}

// ---- aliases -----------------------------------------------------------------------------

#[test]
fn an_alias_takes_the_value_and_an_untyped_alias_takes_the_type_of_its_target() {
    let loaded = load_tokens(&json!({
        "base": {"$type": "dimension", "$value": "4px"},
        "same": {"$value": "{base}"},
        "other": {"$type": "number", "$value": "{base}"}
    }));
    assert!(loaded.problems.is_empty());
    assert_eq!(loaded.tokens["same"].value, json!("4px"));
    assert_eq!(kind(&loaded, "same"), "dimension");
    // Its own type wins; the value is still the target's.
    assert_eq!(kind(&loaded, "other"), "number");
    assert_eq!(loaded.tokens["other"].value, json!("4px"));
}

#[test]
fn an_alias_may_come_before_its_target_and_chains_resolve_through_every_link() {
    let loaded = load_tokens(&json!({
        "$type": "number",
        "c": {"$value": "{b}"},
        "b": {"$value": "{a}"},
        "a": {"$value": 7},
        "d": {"$value": "{c}", "$type": "string"}
    }));
    assert!(loaded.problems.is_empty());
    assert_eq!(names(&loaded), ["c", "b", "a", "d"]);
    for path in ["c", "b", "a"] {
        assert_eq!(loaded.tokens[path].value, json!(7), "{path}");
    }
    assert_eq!(kind(&loaded, "d"), "string");
    assert_eq!(loaded.tokens["d"].value, json!(7));
}

#[test]
fn an_alias_reaches_into_groups_by_its_dotted_path() {
    let loaded = load_tokens(&json!({
        "$type": "color",
        "brand": {"primary": {"$value": "#00f"}},
        "button": {"bg": {"$value": "{brand.primary}"}}
    }));
    assert!(loaded.problems.is_empty());
    assert_eq!(loaded.tokens["button.bg"].value, json!("#00f"));
}

#[test]
fn only_a_whole_string_value_in_braces_is_an_alias() {
    for literal in [
        "{a} and more",
        "x{a}",
        "{}",
        "{a{b}",
        "{a}}",
        "{{a}}",
        "{a.b}c",
        " {a}",
    ] {
        let loaded = load_tokens(&json!({
            "$type": "string", "a": {"$value": "A"}, "t": {"$value": literal}
        }));
        assert!(
            loaded.problems.is_empty(),
            "{literal:?}: {:?}",
            loaded.problems
        );
        assert_eq!(loaded.tokens["t"].value, json!(literal), "{literal:?}");
    }
    // A value that is not a string is never an alias.
    let loaded =
        load_tokens(&json!({"$type": "string", "a": {"$value": 1}, "t": {"$value": ["{a}"]}}));
    assert_eq!(loaded.tokens["t"].value, json!(["{a}"]));
}

#[test]
fn an_alias_to_a_missing_token_is_t004_and_the_aliases_that_lead_to_it_are_casualties() {
    let loaded = load_tokens(&json!({
        "$type": "number",
        "a": {"$value": "{b}"},
        "b": {"$value": "{nope}"},
        "ok": {"$value": 1}
    }));
    assert_eq!(
        problems(&loaded),
        [(TokenCode::T004, "b"), (TokenCode::T004, "a")]
    );
    assert_eq!(
        loaded.problems[0].message,
        "Alias {nope} points at a token that does not exist."
    );
    assert_eq!(loaded.problems[1].message, "Alias {b} cannot be resolved.");
    assert_eq!(names(&loaded), ["ok"]);
}

#[test]
fn an_alias_to_a_group_is_an_alias_to_nothing() {
    let loaded = load_tokens(&json!({
        "$type": "number", "g": {"x": {"$value": 1}}, "t": {"$value": "{g}"}
    }));
    assert_eq!(problems(&loaded), [(TokenCode::T004, "t")]);
}

#[test]
fn an_alias_to_an_untyped_token_is_a_casualty_of_that_token() {
    let loaded = load_tokens(&json!({
        "bare": {"$value": 1}, "a": {"$value": "{bare}", "$type": "number"}
    }));
    assert_eq!(
        problems(&loaded),
        [(TokenCode::T003, "bare"), (TokenCode::T004, "a")]
    );
    assert!(loaded.tokens.is_empty());
}

#[test]
fn a_cycle_is_t005_with_the_whole_loop_in_the_message() {
    let loaded = load_tokens(&json!({
        "$type": "number",
        "a": {"$value": "{b}"},
        "b": {"$value": "{c}"},
        "c": {"$value": "{a}"},
        "ok": {"$value": 1}
    }));
    assert_eq!(problems(&loaded)[0], (TokenCode::T005, "a"));
    assert_eq!(loaded.problems[0].message, "Alias cycle: a -> b -> c -> a.");
    assert_eq!(names(&loaded), ["ok"]);
    // Every member of the loop is accounted for exactly once.
    let paths: Vec<_> = loaded.problems.iter().map(|p| p.path.as_str()).collect();
    for member in ["a", "b", "c"] {
        assert_eq!(
            paths.iter().filter(|p| **p == member).count(),
            1,
            "{member}"
        );
    }
}

#[test]
fn a_token_that_aliases_itself_is_a_cycle_of_one() {
    let loaded = load_tokens(&json!({"$type": "number", "a": {"$value": "{a}"}}));
    assert_eq!(problems(&loaded), [(TokenCode::T005, "a")]);
    assert_eq!(loaded.problems[0].message, "Alias cycle: a -> a.");
    assert!(loaded.tokens.is_empty());
}

#[test]
fn a_token_that_leads_into_a_cycle_is_a_casualty_not_a_second_cycle() {
    let loaded = load_tokens(&json!({
        "$type": "number",
        "in": {"$value": "{x}"},
        "x": {"$value": "{y}"},
        "y": {"$value": "{x}"}
    }));
    let codes: Vec<_> = loaded.problems.iter().map(|p| p.code).collect();
    assert_eq!(codes.iter().filter(|c| **c == TokenCode::T005).count(), 1);
    assert!(loaded.problems.iter().any(|p| p.path == "in"));
    assert!(loaded.tokens.is_empty());
}

#[test]
fn a_long_alias_chain_resolves_without_exhausting_the_stack() {
    let mut file = serde_json::Map::new();
    file.insert("$type".into(), json!("number"));
    let length = 20_000;
    for i in 0..length {
        let target = format!("{{t{}}}", i + 1);
        file.insert(format!("t{i}"), json!({"$value": target}));
    }
    file.insert(format!("t{length}"), json!({"$value": 42}));
    let loaded = load_tokens(&Json::Object(file));
    assert!(loaded.problems.is_empty());
    assert_eq!(loaded.tokens.len(), length + 1);
    assert_eq!(loaded.tokens["t0"].value, json!(42));
}

#[test]
fn a_long_cycle_is_reported_once_without_exhausting_the_stack() {
    let mut file = serde_json::Map::new();
    file.insert("$type".into(), json!("number"));
    let length = 20_000;
    for i in 0..length {
        let target = format!("{{t{}}}", (i + 1) % length);
        file.insert(format!("t{i}"), json!({"$value": target}));
    }
    let loaded = load_tokens(&Json::Object(file));
    let cycles = loaded
        .problems
        .iter()
        .filter(|p| p.code == TokenCode::T005)
        .count();
    assert_eq!(cycles, 1);
    assert!(loaded.tokens.is_empty());
}

// ---- the type table ----------------------------------------------------------------------

#[test]
fn token_types_maps_each_path_to_its_type_in_order() {
    let loaded = load_tokens(&json!({
        "space": {"$type": "dimension", "sm": {"$value": 1}, "md": {"$value": "{space.sm}"}},
        "color": {"$type": "color", "accent": {"$value": "#f00"}}
    }));
    let table = token_types(&loaded.tokens);
    let pairs: Vec<_> = table
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("space.sm", "dimension"),
            ("space.md", "dimension"),
            ("color.accent", "color")
        ]
    );
    assert!(token_types(&Tokens::default().tokens).is_empty());
}

// ---- hostile input -----------------------------------------------------------------------

#[test]
fn a_very_deep_group_nest_is_loaded_without_a_crash() {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(|| {
            let mut node = json!({"$type": "number", "leaf": {"$value": 1}});
            for _ in 0..2_000 {
                let mut m = serde_json::Map::new();
                m.insert("g".into(), node);
                node = Json::Object(m);
            }
            let loaded = load_tokens(&node);
            assert_eq!(loaded.tokens.len(), 1);
            assert!(loaded.tokens.keys().next().unwrap().ends_with(".leaf"));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn loading_does_not_change_its_input() {
    let input =
        json!({"b": {"$type": "number", "$value": 1}, "1": {"$type": "number", "$value": 2}});
    let before = input.clone();
    let _ = load_tokens(&input);
    assert_eq!(input, before);
    assert_eq!(
        serde_json::to_string(&input).unwrap(),
        serde_json::to_string(&before).unwrap()
    );
}
