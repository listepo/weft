//! Claims about the catalog version diff (SPEC §8): removing or tightening is major, adding or
//! loosening is minor, wording is none, and what the classifier does not know is major.

#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_catalog::{CatalogDiff, ChangeLevel, diff_catalogs};

use ChangeLevel::{Major, Minor, None as Nothing};

fn base() -> Json {
    json!({
        "weft": "0.1", "name": "t", "version": "1.0.0",
        "components": {
            "card": {
                "description": "Card.", "role": "group", "content": "nodes",
                "allowedChildren": ["text", "button"], "allowedParents": ["screen"],
                "requiresLabel": false,
                "props": {
                    "tone": {"description": "Tone.", "type": "enum", "values": ["a", "b"], "default": "a"},
                    "count": {"description": "Count.", "type": "number", "min": 0, "max": 10},
                    "gap": {"description": "Gap.", "type": "token", "tokenType": "dimension"}
                },
                "slots": {"footer": {"description": "Footer.", "allowedChildren": ["button"], "required": false}},
                "states": ["idle", "busy"],
                "events": ["press"]
            },
            "text": {"description": "Text.", "role": "none", "content": "text"}
        }
    })
}

/// The diff from the base to the base after `edit`, and the one back.
fn edit(f: impl FnOnce(&mut Json)) -> (CatalogDiff, CatalogDiff) {
    let before = base();
    let mut after = base();
    f(&mut after);
    (
        diff_catalogs(&before, &after),
        diff_catalogs(&after, &before),
    )
}

fn card(json: &mut Json) -> &mut Json {
    &mut json["components"]["card"]
}

fn prop<'a>(json: &'a mut Json, name: &str) -> &'a mut Json {
    &mut json["components"]["card"]["props"][name]
}

fn unset(value: &mut Json, key: &str) {
    value.as_object_mut().unwrap().remove(key);
}

fn summary(diff: &CatalogDiff) -> Vec<(ChangeLevel, &str, &str)> {
    diff.changes
        .iter()
        .map(|c| (c.level, c.path.as_str(), c.message.as_str()))
        .collect()
}

// ---- the whole ---------------------------------------------------------------------------

#[test]
fn a_catalog_against_itself_has_no_changes() {
    let d = diff_catalogs(&base(), &base());
    assert_eq!(d.level, Nothing);
    assert!(d.changes.is_empty());
}

#[test]
fn the_level_is_the_highest_of_the_changes_and_none_when_there_are_none() {
    let (d, _) = edit(|c| {
        card(c)["description"] = json!("Other.");
        c["components"]["extra"] = json!({"description": "x", "role": "none", "content": "none"});
    });
    assert_eq!(d.level, Minor);
    let (d, _) = edit(|c| {
        card(c)["description"] = json!("Other.");
    });
    assert_eq!((d.level, d.changes.len()), (Nothing, 1));
    let (d, _) = edit(|c| {
        card(c)["role"] = json!("region");
        c["components"]["extra"] = json!({"description": "x", "role": "none", "content": "none"});
    });
    assert_eq!(d.level, Major);
}

#[test]
fn a_diff_serializes_with_lowercase_levels() {
    let (d, _) = edit(|c| {
        card(c)["role"] = json!("region");
    });
    assert_eq!(
        serde_json::to_value(&d).unwrap(),
        json!({"level": "major", "changes": [{
            "path": "components.card.role", "level": "major",
            "message": "role changed from \"group\" to \"region\"."
        }]})
    );
    assert!(ChangeLevel::None < ChangeLevel::Minor && ChangeLevel::Minor < ChangeLevel::Major);
}

#[test]
fn shapes_that_are_not_catalogs_have_no_changes_and_do_not_panic() {
    let shapes = [
        json!(null),
        json!(1),
        json!("x"),
        json!([]),
        json!({}),
        json!({"components": null}),
        json!({"components": []}),
        json!({"components": "x"}),
    ];
    for a in &shapes {
        for b in &shapes {
            let d = diff_catalogs(a, b);
            assert_eq!((d.level, d.changes.len()), (Nothing, 0), "{a} vs {b}");
        }
    }
    // A catalog compared with nothing at all loses every component.
    assert_eq!(diff_catalogs(&base(), &json!({})).level, Major);
    assert_eq!(diff_catalogs(&json!({}), &base()).level, Minor);
}

#[test]
fn members_that_should_be_objects_but_are_not_do_not_panic() {
    let weird = json!({"components": {
        "card": {"props": 1, "slots": [], "states": "x", "events": {"a": 1}, "allowedChildren": 3,
                 "content": 5, "role": null, "description": {}},
        "other": 7, "third": null
    }});
    for (a, b) in [(&weird, &base()), (&base(), &weird), (&weird, &weird)] {
        let _ = diff_catalogs(a, b);
    }
    assert_eq!(diff_catalogs(&weird, &weird).level, Nothing);
}

#[test]
fn inputs_are_not_changed() {
    let (a, b) = (base(), {
        let mut b = base();
        card(&mut b)["role"] = json!("region");
        b
    });
    let (a0, b0) = (a.clone(), b.clone());
    let _ = diff_catalogs(&a, &b);
    assert_eq!((a, b), (a0, b0));
}

// ---- components --------------------------------------------------------------------------

#[test]
fn removing_a_component_is_major_and_adding_one_is_minor_whatever_it_requires() {
    let (removed, added) = edit(|c| {
        c["components"].as_object_mut().unwrap().remove("text");
    });
    assert_eq!(
        summary(&removed),
        [(Major, "components.text", "Component \"text\" was removed.")]
    );
    assert_eq!(
        summary(&added),
        [(Minor, "components.text", "Component \"text\" was added.")]
    );
    let (_, added) = edit(|c| {
        c["components"].as_object_mut().unwrap().remove("card");
    });
    // A component with required props and slots is still only an addition.
    assert_eq!(added.level, Minor);
    assert_eq!(added.changes.len(), 1);
}

#[test]
fn a_changed_description_is_none_at_every_level() {
    let (c, _) = edit(|j| card(j)["description"] = json!("X."));
    assert_eq!(
        summary(&c),
        [(
            Nothing,
            "components.card.description",
            "The description changed."
        )]
    );
    let (p, _) = edit(|j| prop(j, "count")["description"] = json!("X."));
    assert_eq!(p.changes[0].path, "components.card.props.count.description");
    assert_eq!(p.level, Nothing);
    let (s, _) = edit(|j| card(j)["slots"]["footer"]["description"] = json!("X."));
    assert_eq!(
        s.changes[0].path,
        "components.card.slots.footer.description"
    );
    assert_eq!(s.level, Nothing);
    let (gone, _) = edit(|j| unset(prop(j, "count"), "description"));
    assert_eq!((gone.level, gone.changes.len()), (Nothing, 1));
}

#[test]
fn a_changed_role_is_major_in_both_directions() {
    let (a, b) = edit(|j| card(j)["role"] = json!("region"));
    assert_eq!(
        a.changes[0].message,
        "role changed from \"group\" to \"region\"."
    );
    assert_eq!(
        b.changes[0].message,
        "role changed from \"region\" to \"group\"."
    );
    assert_eq!((a.level, b.level), (Major, Major));
    let (a, b) = edit(|j| unset(card(j), "role"));
    assert_eq!((a.level, b.level), (Major, Major));
    assert_eq!(
        a.changes[0].message,
        "role changed from \"group\" to \"undefined\"."
    );
}

#[test]
fn a_content_model_that_accepts_more_is_minor_and_one_that_accepts_less_is_major() {
    let kinds = ["none", "text", "nodes", "mixed"];
    let accepts = |k: &str| match k {
        "text" => vec!["text"],
        "nodes" => vec!["nodes"],
        "mixed" => vec!["text", "nodes"],
        _ => vec![],
    };
    for from in kinds {
        for to in kinds {
            if from == to {
                continue;
            }
            let mut a = base();
            let mut b = base();
            a["components"]["text"]["content"] = json!(from);
            b["components"]["text"]["content"] = json!(to);
            let d = diff_catalogs(&a, &b);
            let wider = accepts(from).iter().all(|k| accepts(to).contains(k));
            assert_eq!(d.level, if wider { Minor } else { Major }, "{from} -> {to}");
            assert_eq!(d.changes.len(), 1);
            assert_eq!(
                d.changes[0].message,
                format!(
                    "content changed from \"{from}\" to \"{to}\", {} the content model.",
                    if wider { "widening" } else { "narrowing" }
                )
            );
            assert_eq!(d.changes[0].path, "components.text.content");
        }
    }
}

#[test]
fn an_unknown_content_word_accepts_nothing() {
    let mut a = base();
    let mut b = base();
    a["components"]["text"]["content"] = json!("none");
    b["components"]["text"]["content"] = json!("weird");
    assert_eq!(diff_catalogs(&a, &b).level, Minor);
    assert_eq!(diff_catalogs(&b, &a).level, Minor);
    assert_eq!(diff_catalogs(&base(), &b).level, Major);
}

#[test]
fn requiring_a_label_is_major_and_relaxing_it_is_minor() {
    let (a, b) = edit(|j| card(j)["requiresLabel"] = json!(true));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.requiresLabel",
            "A label became required."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Minor,
            "components.card.requiresLabel",
            "A label is no longer required."
        )]
    );
    // Absent and null both mean false.
    let (a, b) = edit(|j| unset(card(j), "requiresLabel"));
    assert_eq!((a.changes.len(), b.changes.len()), (0, 0));
    let (a, _) = edit(|j| card(j)["requiresLabel"] = json!(null));
    assert!(a.changes.is_empty());
}

// ---- restrictions ------------------------------------------------------------------------

#[test]
fn adding_a_restriction_that_was_absent_is_major_and_lifting_it_is_minor() {
    let (lifted, restricted) = edit(|j| unset(card(j), "allowedChildren"));
    assert_eq!(
        summary(&lifted),
        [(
            Minor,
            "components.card.allowedChildren",
            "Child kinds restriction was lifted."
        )]
    );
    assert_eq!(
        summary(&restricted),
        [(
            Major,
            "components.card.allowedChildren",
            "Child kinds was restricted to [\"text\",\"button\"]."
        )]
    );
    let (lifted, restricted) = edit(|j| unset(card(j), "allowedParents"));
    assert_eq!((lifted.level, restricted.level), (Minor, Major));
    assert_eq!(
        restricted.changes[0].message,
        "Parent kinds was restricted to [\"screen\"]."
    );
}

#[test]
fn a_restriction_that_loses_a_name_is_major_and_one_that_gains_a_name_is_minor() {
    let (narrowed, widened) = edit(|j| card(j)["allowedChildren"] = json!(["text"]));
    assert_eq!(
        summary(&narrowed),
        [(
            Major,
            "components.card.allowedChildren",
            "Child kinds no longer allows \"button\"."
        )]
    );
    assert_eq!(
        summary(&widened),
        [(
            Minor,
            "components.card.allowedChildren",
            "Child kinds now allows \"button\"."
        )]
    );
}

#[test]
fn a_swapped_restriction_reports_what_left_before_what_arrived() {
    let (d, _) = edit(|j| card(j)["allowedChildren"] = json!(["button", "link"]));
    assert_eq!(
        summary(&d)
            .iter()
            .map(|(l, _, m)| (*l, *m))
            .collect::<Vec<_>>(),
        [
            (Major, "Child kinds no longer allows \"text\"."),
            (Minor, "Child kinds now allows \"link\"."),
        ]
    );
    assert_eq!(d.level, Major);
}

#[test]
fn reordering_a_restriction_changes_nothing() {
    let (d, _) = edit(|j| card(j)["allowedChildren"] = json!(["button", "text"]));
    assert!(d.changes.is_empty());
}

#[test]
fn an_empty_list_is_a_restriction_that_allows_nothing() {
    let (d, e) = edit(|j| card(j)["allowedChildren"] = json!([]));
    assert_eq!(d.changes.len(), 2);
    assert!(d.changes.iter().all(|c| c.level == Major));
    assert!(e.changes.iter().all(|c| c.level == Minor));
}

// ---- props -------------------------------------------------------------------------------

#[test]
fn removing_a_prop_is_major_adding_an_optional_one_is_minor_and_a_required_one_major() {
    let (removed, added) = edit(|j| unset(&mut card(j)["props"], "count"));
    assert_eq!(
        summary(&removed),
        [(
            Major,
            "components.card.props.count",
            "Prop \"count\" was removed."
        )]
    );
    assert_eq!(
        summary(&added),
        [(
            Minor,
            "components.card.props.count",
            "Prop \"count\" was added."
        )]
    );
    let (d, back) = edit(|j| {
        card(j)["props"]["need"] = json!({"description": "N", "type": "string", "required": true});
    });
    assert_eq!(
        summary(&d),
        [(
            Major,
            "components.card.props.need",
            "Prop \"need\" was added."
        )]
    );
    assert_eq!(back.level, Major);
}

#[test]
fn a_changed_prop_type_is_major() {
    let (d, _) = edit(|j| prop(j, "count")["type"] = json!("string"));
    assert_eq!(
        summary(&d),
        [(
            Major,
            "components.card.props.count.type",
            "type changed from number to string."
        )]
    );
}

#[test]
fn enum_values_removed_are_major_and_added_are_minor() {
    let (removed, added) = edit(|j| prop(j, "tone")["values"] = json!(["a"]));
    assert_eq!(
        summary(&removed),
        [(
            Major,
            "components.card.props.tone.values",
            "Enum value \"b\" was removed."
        )]
    );
    assert_eq!(
        summary(&added),
        [(
            Minor,
            "components.card.props.tone.values",
            "Enum value \"b\" was added."
        )]
    );
    let (reordered, _) = edit(|j| prop(j, "tone")["values"] = json!(["b", "a"]));
    assert!(reordered.changes.is_empty());
}

#[test]
fn enum_values_are_compared_by_strict_equality_not_by_text() {
    let (d, _) = edit(|j| prop(j, "tone")["values"] = json!(["a", 1]));
    assert_eq!(d.level, Major);
    assert_eq!(
        d.changes
            .iter()
            .map(|c| c.message.as_str())
            .collect::<Vec<_>>(),
        [
            "Enum value \"b\" was removed.",
            "Enum value \"1\" was added."
        ]
    );
    let mut a = base();
    let mut b = base();
    prop(&mut a, "tone")["values"] = json!([1]);
    prop(&mut b, "tone")["values"] = json!(["1"]);
    let d = diff_catalogs(&a, &b);
    assert_eq!(d.level, Major);
    assert_eq!(d.changes.len(), 2);
}

#[test]
fn a_changed_token_type_is_major() {
    let (d, _) = edit(|j| prop(j, "gap")["tokenType"] = json!("number"));
    assert_eq!(
        summary(&d),
        [(
            Major,
            "components.card.props.gap.tokenType",
            "tokenType changed from \"dimension\" to \"number\"."
        )]
    );
    let (d, _) = edit(|j| unset(prop(j, "gap"), "tokenType"));
    assert_eq!(
        d.changes[0].message,
        "tokenType changed from \"dimension\" to absent."
    );
}

#[test]
fn a_prop_that_becomes_required_is_major_and_one_that_stops_is_minor() {
    let (a, b) = edit(|j| prop(j, "count")["required"] = json!(true));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.props.count.required",
            "The prop became required."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Minor,
            "components.card.props.count.required",
            "The prop is no longer required."
        )]
    );
    // false, null and absent are the same.
    let (a, _) = edit(|j| prop(j, "count")["required"] = json!(false));
    assert!(a.changes.is_empty());
    let (a, _) = edit(|j| prop(j, "count")["required"] = json!(null));
    assert!(a.changes.is_empty());
}

#[test]
fn a_changed_default_is_major_and_defaults_compare_deeply() {
    let (a, _) = edit(|j| prop(j, "tone")["default"] = json!("b"));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.props.tone.default",
            "default changed from \"a\" to \"b\"."
        )]
    );
    let (a, _) = edit(|j| unset(prop(j, "tone"), "default"));
    assert_eq!(
        a.changes[0].message,
        "default changed from \"a\" to absent."
    );
    let mut x = base();
    let mut y = base();
    prop(&mut x, "tone")["default"] = json!({"a": [1, {"b": 2}], "c": null});
    prop(&mut y, "tone")["default"] = json!({"c": null, "a": [1, {"b": 2}]});
    assert!(diff_catalogs(&x, &y).changes.is_empty());
    prop(&mut y, "tone")["default"] = json!({"c": null, "a": [1, {"b": 3}]});
    assert_eq!(diff_catalogs(&x, &y).level, Major);
    // Numbers are numbers whatever their spelling.
    prop(&mut x, "tone")["default"] = json!(1);
    prop(&mut y, "tone")["default"] = json!(1.0);
    assert!(diff_catalogs(&x, &y).changes.is_empty());
}

#[test]
fn bindable_defaults_to_true_and_only_losing_it_is_major() {
    let (a, b) = edit(|j| prop(j, "count")["bindable"] = json!(false));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.props.count.bindable",
            "The prop no longer accepts bindings."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Minor,
            "components.card.props.count.bindable",
            "The prop accepts bindings."
        )]
    );
    let (a, _) = edit(|j| prop(j, "count")["bindable"] = json!(true));
    assert!(a.changes.is_empty());
    let (a, _) = edit(|j| prop(j, "count")["bindable"] = json!(null));
    assert!(a.changes.is_empty());
}

#[test]
fn writable_defaults_to_false_and_only_gaining_it_is_minor() {
    let (a, b) = edit(|j| prop(j, "count")["writable"] = json!(true));
    assert_eq!(
        summary(&a),
        [(
            Minor,
            "components.card.props.count.writable",
            "The prop became a two-way target."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Major,
            "components.card.props.count.writable",
            "The prop is no longer a two-way target."
        )]
    );
    let (a, _) = edit(|j| prop(j, "count")["writable"] = json!(false));
    assert!(a.changes.is_empty());
}

#[test]
fn naming_an_element_kind_is_major_and_dropping_it_is_minor() {
    let (a, b) = edit(|j| prop(j, "count")["references"] = json!("card"));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.props.count.references",
            "references changed from undefined to card."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Minor,
            "components.card.props.count.references",
            "The prop no longer names an element."
        )]
    );
}

#[test]
fn becoming_the_root_is_major_and_giving_it_up_is_minor() {
    let (a, b) = edit(|j| j["components"]["card"]["root"] = json!(true));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.root",
            "The component became the document root."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Minor,
            "components.card.root",
            "The component is no longer the document root."
        )]
    );
    let (a, _) = edit(|j| j["components"]["card"]["root"] = json!(false));
    assert!(a.changes.is_empty());
}

#[test]
fn a_raised_minimum_or_a_lowered_maximum_is_major_and_the_opposite_is_minor() {
    let (a, b) = edit(|j| prop(j, "count")["min"] = json!(2));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.props.count.min",
            "min changed from 0 to 2."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Minor,
            "components.card.props.count.min",
            "min changed from 2 to 0."
        )]
    );
    let (a, b) = edit(|j| prop(j, "count")["max"] = json!(5));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.props.count.max",
            "max changed from 10 to 5."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Minor,
            "components.card.props.count.max",
            "max changed from 5 to 10."
        )]
    );
}

#[test]
fn a_bound_that_appears_narrows_and_one_that_disappears_widens() {
    let (dropped, appeared) = edit(|j| unset(prop(j, "count"), "max"));
    assert_eq!(
        summary(&dropped),
        [(
            Minor,
            "components.card.props.count.max",
            "max changed from 10 to absent."
        )]
    );
    assert_eq!(
        summary(&appeared),
        [(
            Major,
            "components.card.props.count.max",
            "max changed from absent to 10."
        )]
    );
    let (dropped, appeared) = edit(|j| unset(prop(j, "count"), "min"));
    assert_eq!((dropped.level, appeared.level), (Minor, Major));
}

#[test]
fn a_bound_that_is_not_a_number_is_judged_by_whether_it_is_still_there() {
    let (a, b) = edit(|j| prop(j, "count")["min"] = json!("zero"));
    assert_eq!(a.level, Major);
    assert_eq!(a.changes[0].message, "min changed from 0 to \"zero\".");
    assert_eq!(b.level, Major);
}

#[test]
fn bounds_equal_as_numbers_are_unchanged() {
    let (a, _) = edit(|j| prop(j, "count")["max"] = json!(10.0));
    assert!(a.changes.is_empty());
}

#[test]
fn a_field_the_classifier_does_not_know_is_major_whenever_it_differs() {
    let (added, removed) = edit(|j| prop(j, "count")["pattern"] = json!("^a"));
    assert_eq!(
        summary(&added),
        [(
            Major,
            "components.card.props.count.pattern",
            "pattern changed from absent to \"^a\"."
        )]
    );
    assert_eq!(
        summary(&removed),
        [(
            Major,
            "components.card.props.count.pattern",
            "pattern changed from \"^a\" to absent."
        )]
    );
    let mut x = base();
    let mut y = base();
    prop(&mut x, "count")["pattern"] = json!({"a": [1]});
    prop(&mut y, "count")["pattern"] = json!({"a": [1]});
    assert!(diff_catalogs(&x, &y).changes.is_empty());
    prop(&mut y, "count")["pattern"] = json!({"a": [2]});
    assert_eq!(diff_catalogs(&x, &y).level, Major);
}

#[test]
fn integer_is_not_among_the_fields_the_classifier_knows() {
    // Both the TypeScript and the Rust classifier list ten fields, and `integer` is not one of them,
    // so any change of it is reported as an unknown field, even loosening a constraint.
    let (made_integer, made_loose) = edit(|j| prop(j, "count")["integer"] = json!(true));
    assert_eq!(
        summary(&made_integer),
        [(
            Major,
            "components.card.props.count.integer",
            "integer changed from absent to true."
        )]
    );
    assert_eq!(made_loose.level, Major);
}

// ---- slots -------------------------------------------------------------------------------

#[test]
fn removing_a_slot_is_major_adding_one_is_minor_unless_it_is_required() {
    let (removed, added) = edit(|j| unset(&mut card(j)["slots"], "footer"));
    assert_eq!(
        summary(&removed),
        [(
            Major,
            "components.card.slots.footer",
            "Slot \"footer\" was removed."
        )]
    );
    assert_eq!(
        summary(&added),
        [(
            Minor,
            "components.card.slots.footer",
            "Slot \"footer\" was added."
        )]
    );
    let (added_optional, _) = edit(|j| card(j)["slots"]["extra"] = json!({"description": "E"}));
    assert_eq!(added_optional.level, Minor);
    let (added_required, _) =
        edit(|j| card(j)["slots"]["extra"] = json!({"description": "E", "required": true}));
    assert_eq!(
        summary(&added_required),
        [(
            Major,
            "components.card.slots.extra",
            "Slot \"extra\" was added."
        )]
    );
}

#[test]
fn a_slot_that_becomes_required_is_major_and_one_that_stops_is_minor() {
    let (a, b) = edit(|j| card(j)["slots"]["footer"]["required"] = json!(true));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.slots.footer.required",
            "The slot became required."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Minor,
            "components.card.slots.footer.required",
            "The slot is no longer required."
        )]
    );
    let (a, _) = edit(|j| unset(&mut card(j)["slots"]["footer"], "required"));
    assert!(a.changes.is_empty());
}

#[test]
fn slot_content_restrictions_follow_the_same_rules_as_components() {
    let (a, b) = edit(|j| unset(&mut card(j)["slots"]["footer"], "allowedChildren"));
    assert_eq!(
        summary(&a),
        [(
            Minor,
            "components.card.slots.footer.allowedChildren",
            "Slot content restriction was lifted."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Major,
            "components.card.slots.footer.allowedChildren",
            "Slot content was restricted to [\"button\"]."
        )]
    );
    let (a, b) =
        edit(|j| card(j)["slots"]["footer"]["allowedChildren"] = json!(["button", "link"]));
    assert_eq!(
        summary(&a),
        [(
            Minor,
            "components.card.slots.footer.allowedChildren",
            "Slot content now allows \"link\"."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Major,
            "components.card.slots.footer.allowedChildren",
            "Slot content no longer allows \"link\"."
        )]
    );
}

// ---- states and events -------------------------------------------------------------------

#[test]
fn states_and_events_removed_are_major_and_added_are_minor() {
    let (a, b) = edit(|j| card(j)["states"] = json!(["idle"]));
    assert_eq!(
        summary(&a),
        [(
            Major,
            "components.card.states",
            "State \"busy\" was removed."
        )]
    );
    assert_eq!(
        summary(&b),
        [(Minor, "components.card.states", "State \"busy\" was added.")]
    );
    let (a, b) = edit(|j| card(j)["events"] = json!(["press", "hover"]));
    assert_eq!(
        summary(&a),
        [(
            Minor,
            "components.card.events",
            "Event \"hover\" was added."
        )]
    );
    assert_eq!(
        summary(&b),
        [(
            Major,
            "components.card.events",
            "Event \"hover\" was removed."
        )]
    );
    let (a, b) = edit(|j| unset(card(j), "events"));
    assert_eq!((a.level, b.level), (Major, Minor));
}

#[test]
fn reordering_states_changes_nothing() {
    let (a, _) = edit(|j| card(j)["states"] = json!(["busy", "idle"]));
    assert!(a.changes.is_empty());
}

// ---- order -------------------------------------------------------------------------------

#[test]
fn removals_come_before_additions_and_changes_follow_the_new_catalog() {
    let mut next = base();
    unset(&mut next["components"], "text");
    next["components"]["zeta"] = json!({"description": "Z"});
    next["components"]["alpha"] = json!({"description": "A"});
    card(&mut next)["role"] = json!("region");
    let d = diff_catalogs(&base(), &next);
    let paths: Vec<_> = d.changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "components.text",
            "components.card.role",
            "components.zeta",
            "components.alpha"
        ]
    );
}

#[test]
fn integer_like_component_names_come_first_as_in_a_javascript_object() {
    let mut next = base();
    for name in ["b", "10", "a", "2"] {
        next["components"][name] = json!({"description": "x"});
    }
    let d = diff_catalogs(&base(), &next);
    let paths: Vec<_> = d.changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "components.2",
            "components.10",
            "components.b",
            "components.a"
        ]
    );
}

#[test]
fn several_changes_in_one_prop_are_reported_in_the_order_of_the_rules() {
    let (d, _) = edit(|j| {
        let p = prop(j, "count");
        p["description"] = json!("X");
        p["type"] = json!("string");
        p["required"] = json!(true);
        p["bindable"] = json!(false);
        p["writable"] = json!(true);
        p["min"] = json!(1);
        p["max"] = json!(9);
        p["zzz"] = json!(1);
        p["aaa"] = json!(1);
    });
    let fields: Vec<_> = d
        .changes
        .iter()
        .map(|c| c.path.rsplit('.').next().unwrap())
        .collect();
    assert_eq!(
        fields,
        [
            "description",
            "type",
            "required",
            "bindable",
            "writable",
            "min",
            "max",
            "zzz",
            "aaa"
        ]
    );
}
