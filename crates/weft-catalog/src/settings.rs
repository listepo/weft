//! The tool sections of the project file (SPEC §10.6): what `weft.json` configures beyond the
//! shared resources. One table describes every key; the loader checks a file against it and keeps
//! only what is valid, and the published JSON Schema (`schemas/weft.schema.json`) is generated
//! from the same table, so the two cannot drift. A tool applies its own defaults to what is absent,
//! and an explicit argument overrides both.

use serde_json::{Map as Object, Value as Json, json};
use weft_core::{Code, Diagnostic, did_you_mean, one_of};

use crate::project::{MAX_TOKEN_FILES, escape_pointer, is_project_file_name, quote};

/// The kind of value a setting holds.
pub(crate) enum Kind {
    /// Named settings; any other key is `W702`.
    Section(&'static [Setting]),
    /// One of these strings.
    OneOf(&'static [&'static str]),
    Bool,
    /// A string that is not empty.
    Text,
    /// A file or directory name relative to the project file (SPEC §10.2).
    File,
    /// Token files: an array of such names, at most `MAX_TOKEN_FILES`, or one resolver document's
    /// name (SPEC §10.3).
    TokenFiles,
    /// A whole number from 1 to `MAX_COUNT`.
    Count,
    /// An object of objects, one entry per plugin. A plugin the table lists is checked as a
    /// section; the content of any other is not.
    Plugins(&'static [Setting]),
}

pub(crate) struct Setting {
    pub name: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    /// The default as JSON text, for the schema; `None` when absence means "not used".
    pub default: Option<&'static str>,
}

const fn setting(name: &'static str, description: &'static str, kind: Kind) -> Setting {
    Setting {
        name,
        description,
        kind,
        default: None,
    }
}

const fn with_default(
    name: &'static str,
    description: &'static str,
    kind: Kind,
    default: &'static str,
) -> Setting {
    Setting {
        name,
        description,
        kind,
        default: Some(default),
    }
}

/// Large enough for any host, small enough that no limit can overflow arithmetic downstream.
pub const MAX_COUNT: u64 = 1 << 40;

const OUT_DIR: &str = "Directory, relative to the project file, that output files go to. Default: next to the input file.";

const SOURCE: &str = "Keep the canonical screen in a leading comment, so the importer gives it back exactly instead of reading the code by convention.";
const TYPESCRIPT: &str = "Write TSX with typed props instead of JSX.";

const HTML_EXPORT: &[Setting] = &[
    setting("outDir", OUT_DIR, Kind::File),
    with_default("source", SOURCE, Kind::Bool, "false"),
    setting(
        "data",
        "Sample data (a JSON file, relative to the project file) the page shows: bindings read it and `<each>` repeats over it. Default: none, the page is a template for a host to fill.",
        Kind::File,
    ),
];
const SHARED_TOKENS: &str = "Screens read the tokens from one shared `WeftTokens.swift` (`weft swiftui-tokens`) instead of each carrying the tokens it uses.";
const SWIFTUI_EXPORT: &[Setting] = &[
    setting("outDir", OUT_DIR, Kind::File),
    setting(
        "data",
        "Sample data (a JSON file, relative to the project file) the generated model is built from in a `sample` property that `#Preview` shows. Default: none, the preview shows the model's defaults.",
        Kind::File,
    ),
    with_default("sharedTokens", SHARED_TOKENS, Kind::Bool, "true"),
];
const JSX_EXPORT: &[Setting] = &[
    setting("outDir", OUT_DIR, Kind::File),
    with_default("typescript", TYPESCRIPT, Kind::Bool, "false"),
    with_default("source", SOURCE, Kind::Bool, "false"),
];
const OUT_ONLY: &[Setting] = &[setting("outDir", OUT_DIR, Kind::File)];
const CEM_IMPORT: &[Setting] = &[
    setting("outDir", OUT_DIR, Kind::File),
    setting(
        "name",
        "The name of the imported catalog. Default: the manifest's file name without its extension.",
        Kind::Text,
    ),
    with_default(
        "version",
        "The version of the imported catalog.",
        Kind::Text,
        "\"0.0.0\"",
    ),
];

/// One section per target (SPEC §10.6 lists the names reserved for targets in progress).
const EXPORT: &[Setting] = &[
    setting(
        "html",
        "Static HTML pages with CSS and no script (`weft html`).",
        Kind::Section(HTML_EXPORT),
    ),
    setting(
        "react",
        "React components (`weft react`, @weft/to-jsx).",
        Kind::Section(JSX_EXPORT),
    ),
    setting(
        "solid",
        "SolidJS components (`weft solid`).",
        Kind::Section(JSX_EXPORT),
    ),
    setting(
        "lit",
        "Lit web components, JavaScript only (`weft lit`).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "swiftui",
        "SwiftUI views for iOS 17 and macOS 14 (`weft swiftui`).",
        Kind::Section(SWIFTUI_EXPORT),
    ),
    setting(
        "css",
        "The stylesheets React and SolidJS components need: `weft-tokens.css`, the `var(--weft-…)` values they read (`weft css-tokens`), and `weft-base.css`, the rules they share with the static page (`weft css-base`). Default folder: standard output.",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "a2ui",
        "A2UI v0.9 messages, a JSON array of `createSurface` and `updateComponents` (`weft a2ui`).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "slint",
        "Slint components for Slint 1.x (`weft slint`).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "schema",
        "The JSON Schema of the canonical documents the project's catalog admits, `document.schema.json` (`weft schema`). Default folder: standard output.",
        Kind::Section(OUT_ONLY),
    ),
];
const IMPORT: &[Setting] = &[
    setting(
        "html",
        "HTML pages (`weft import-html`, @weft/from-aria).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "react",
        "React components, .jsx or .tsx (`weft import-react`).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "solid",
        "SolidJS components, .jsx or .tsx (`weft import-solid`).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "swiftui",
        "SwiftUI source files (`weft import-swiftui`).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "a2ui",
        "A2UI v0.9 messages, a JSON array, one object or JSON Lines (`weft import-a2ui`).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "slint",
        "Slint source files (`weft import-slint`).",
        Kind::Section(OUT_ONLY),
    ),
    setting(
        "cem",
        "Catalogs from a Custom Elements Manifest (`weft import-cem`).",
        Kind::Section(CEM_IMPORT),
    ),
    setting(
        "figma",
        "Frames of a Figma file, read through the REST API (the plugins' `figma-pull` script).",
        Kind::Section(OUT_ONLY),
    ),
];

const LIMITS: &[Setting] = &[
    with_default(
        "markupChars",
        "UTF-16 code units of one markup argument.",
        Kind::Count,
        "200000",
    ),
    with_default(
        "dataChars",
        "UTF-16 code units of the JSON of weft_render's data argument.",
        Kind::Count,
        "200000",
    ),
    with_default(
        "patches",
        "Patches in one weft_patch call.",
        Kind::Count,
        "100",
    ),
    with_default(
        "patchesChars",
        "UTF-16 code units of the JSON of one patch list.",
        Kind::Count,
        "200000",
    ),
    with_default(
        "projectChars",
        "UTF-16 code units of the JSON of a project argument.",
        Kind::Count,
        "500000",
    ),
    with_default(
        "diagnostics",
        "Diagnostics listed in one result; the rest are counted.",
        Kind::Count,
        "40",
    ),
    with_default(
        "inputElements",
        "JSON values in one call's arguments.",
        Kind::Count,
        "20000",
    ),
];

/// Plugins Weft knows: their settings are checked like any other section.
const PLUGINS: &[Setting] = &[setting(
    "open-design",
    "The Open Design plugin.",
    Kind::Section(&[setting(
        "tokensDir",
        "Folder, relative to the project file, that the plugin's design-md script writes mapped tokens to. Default: next to the design system.",
        Kind::File,
    )]),
)];

/// Every tool section, in the order SPEC §10.6 lists them.
pub(crate) const SECTIONS: &[Setting] = &[
    setting(
        "validate",
        "How screens are checked.",
        Kind::Section(&[with_default(
            "mode",
            "\"strict\" reports unknown elements and attributes as errors, \"lenient\" as warnings (SPEC §8). Used by `weft validate` and as the default of weft_validate's strict argument.",
            Kind::OneOf(&["strict", "lenient"]),
            "\"lenient\"",
        )]),
    ),
    setting(
        "format",
        "How `weft fmt` writes the canonical form. The form itself has no options (SPEC §3).",
        Kind::Section(&[with_default(
            "write",
            "Rewrite the file in place instead of printing it.",
            Kind::Bool,
            "false",
        )]),
    ),
    setting(
        "render",
        "Static HTML pages from screens (`write-page`, the Claude Code plugin's render).",
        Kind::Section(&[
            setting(
                "data",
                "Sample data file the bindings read, relative to the project file. Default: no data.",
                Kind::File,
            ),
            setting(
                "tokens",
                "DTCG token files to render with, in layer order, or one DTCG resolver document; they replace the project's tokens. Default: the project's tokens.",
                Kind::TokenFiles,
            ),
            setting("outDir", OUT_DIR, Kind::File),
            setting(
                "appearance",
                "Which context of the resolver's light and dark modifier (SPEC §10.3) the page is rendered with, and the page's color-scheme. Default: the resolver's default context.",
                Kind::OneOf(&["light", "dark"]),
            ),
        ]),
    ),
    setting(
        "export",
        "Code generated from screens, one section per target.",
        Kind::Section(EXPORT),
    ),
    setting(
        "import",
        "Screens imported from other formats, and catalogs from a Custom Elements Manifest, one section per source.",
        Kind::Section(IMPORT),
    ),
    setting(
        "mcp",
        "The MCP server, when the host starts it with this project file. A project passed as a tool argument never changes these.",
        Kind::Section(&[setting(
            "limits",
            "Bounds on what one tool call may ask of the server.",
            Kind::Section(LIMITS),
        )]),
    ),
    setting(
        "plugins",
        "Settings of plugins, one object per plugin name. The plugins listed here are checked; the content of any other is not.",
        Kind::Plugins(PLUGINS),
    ),
];

/// Checks one section against the table: what is valid is kept, everything else is reported.
pub(crate) fn sanitize(
    setting: &Setting,
    value: &Json,
    pointer: &str,
    report: &mut dyn FnMut(Diagnostic),
) -> Option<Json> {
    let dotted = pointer_name(pointer);
    let wrong =
        |message: String, expected: &str| Diagnostic::new(Code::W701, pointer, message, expected);
    match &setting.kind {
        Kind::Section(children) => {
            let Some(members) = value.as_object() else {
                report(wrong(
                    format!("{} must be an object.", quote(&dotted)),
                    "a JSON object",
                ));
                return None;
            };
            let names = children.iter().map(|c| c.name);
            let mut kept = Object::new();
            for (name, child) in members {
                let at = format!("{pointer}/{}", escape_pointer(name));
                match children.iter().find(|c| c.name == name) {
                    Some(known) => {
                        if let Some(clean) = sanitize(known, child, &at, report) {
                            kept.insert(name.clone(), clean);
                        }
                    }
                    None => report(
                        Diagnostic::new(
                            Code::W702,
                            at,
                            format!("Unknown key {} in {}.", quote(name), quote(&dotted)),
                            one_of(names.clone()),
                        )
                        .got(name)
                        .hint_opt(did_you_mean(name, names.clone())),
                    ),
                }
            }
            Some(Json::Object(kept))
        }
        Kind::OneOf(values) => match value.as_str() {
            Some(v) if values.contains(&v) => Some(value.clone()),
            _ => {
                let quoted: Vec<String> = values.iter().map(|v| quote(v)).collect();
                let choices = quoted.join(" or ");
                report(wrong(
                    format!("{} must be {choices}.", quote(&dotted)),
                    &choices,
                ));
                None
            }
        },
        Kind::Bool => {
            if value.is_boolean() {
                Some(value.clone())
            } else {
                report(wrong(
                    format!("{} must be true or false.", quote(&dotted)),
                    "a boolean",
                ));
                None
            }
        }
        Kind::Text => match value.as_str() {
            Some(text) if !text.is_empty() => Some(value.clone()),
            _ => {
                report(wrong(
                    format!("{} must be a non-empty string.", quote(&dotted)),
                    "a non-empty string",
                ));
                None
            }
        },
        Kind::Count => match value.as_u64() {
            Some(n) if (1..=MAX_COUNT).contains(&n) => Some(value.clone()),
            _ => {
                report(wrong(
                    format!(
                        "{} must be a whole number from 1 to {MAX_COUNT}.",
                        quote(&dotted)
                    ),
                    "a positive whole number",
                ));
                None
            }
        },
        Kind::File => file_name(value, pointer, &dotted, report),
        Kind::TokenFiles if value.is_string() => file_name(value, pointer, &dotted, report),
        Kind::TokenFiles => {
            let Some(entries) = value.as_array().filter(|e| e.len() <= MAX_TOKEN_FILES) else {
                report(wrong(
                    format!(
                        "{} must be an array of at most {MAX_TOKEN_FILES} file names, or a resolver file name.",
                        quote(&dotted)
                    ),
                    "an array of file names or a resolver file name",
                ));
                return None;
            };
            let kept = entries
                .iter()
                .enumerate()
                .filter_map(|(i, entry)| {
                    file_name(entry, &format!("{pointer}/{i}"), &dotted, report)
                })
                .collect();
            Some(Json::Array(kept))
        }
        Kind::Plugins(known) => {
            let Some(members) = value.as_object() else {
                report(wrong(
                    format!("{} must be an object.", quote(&dotted)),
                    "a JSON object with one object per plugin",
                ));
                return None;
            };
            let mut kept = Object::new();
            for (name, entry) in members {
                let at = format!("{pointer}/{}", escape_pointer(name));
                if !entry.is_object() {
                    report(Diagnostic::new(
                        Code::W701,
                        at,
                        format!("The settings of plugin {} must be an object.", quote(name)),
                        "a JSON object",
                    ));
                } else if let Some(plugin) = known.iter().find(|p| p.name == name) {
                    kept.extend(sanitize(plugin, entry, &at, report).map(|c| (name.clone(), c)));
                } else {
                    kept.insert(name.clone(), entry.clone());
                }
            }
            Some(Json::Object(kept))
        }
    }
}

fn file_name(
    value: &Json,
    pointer: &str,
    dotted: &str,
    report: &mut dyn FnMut(Diagnostic),
) -> Option<Json> {
    match value.as_str() {
        None => {
            report(Diagnostic::new(
                Code::W701,
                pointer,
                format!("{} must hold file names.", quote(dotted)),
                "a file name relative to the project",
            ));
            None
        }
        Some(name) if !is_project_file_name(name) => {
            report(
                Diagnostic::new(
                    Code::W703,
                    pointer,
                    format!(
                        "The file name {} is absolute, leaves the project directory or is malformed.",
                        quote(name)
                    ),
                    "a relative path inside the project directory, separated by \"/\"",
                )
                .got(name),
            );
            None
        }
        Some(_) => Some(value.clone()),
    }
}

/// `#/render/outDir` → `render.outDir`, the name messages use; array indexes are left out.
fn pointer_name(pointer: &str) -> String {
    let path = pointer.split_once("#/").map_or("", |(_, rest)| rest);
    let path = path.strip_prefix("project/").unwrap_or(path);
    path.split('/')
        .filter(|s| s.parse::<usize>().is_err())
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>()
        .join(".")
}

fn schema_of(setting: &Setting) -> Json {
    let mut schema = match &setting.kind {
        Kind::Section(children) => section_schema(children),
        Kind::OneOf(values) => json!({ "enum": values }),
        Kind::Bool => json!({ "type": "boolean" }),
        Kind::Text => json!({ "type": "string", "minLength": 1 }),
        Kind::File => json!({ "$ref": "#/$defs/fileName" }),
        Kind::TokenFiles => json!({
            "oneOf": [
                {
                    "type": "array",
                    "maxItems": MAX_TOKEN_FILES,
                    "items": { "$ref": "#/$defs/fileName" },
                },
                { "$ref": "#/$defs/fileName" },
            ],
        }),
        Kind::Count => json!({ "type": "integer", "minimum": 1, "maximum": MAX_COUNT }),
        Kind::Plugins(known) => {
            let properties: Object<String, Json> = known
                .iter()
                .map(|p| (p.name.to_owned(), schema_of(p)))
                .collect();
            json!({
                "type": "object",
                "properties": properties,
                "additionalProperties": { "type": "object" },
            })
        }
    };
    if let Json::Object(map) = &mut schema {
        map.insert("description".to_owned(), json!(setting.description));
        if let Some(default) = setting.default {
            map.insert(
                "default".to_owned(),
                serde_json::from_str(default).unwrap_or(Json::Null),
            );
        }
    }
    schema
}

fn section_schema(children: &[Setting]) -> Json {
    let properties: Object<String, Json> = children
        .iter()
        .map(|c| (c.name.to_owned(), schema_of(c)))
        .collect();
    json!({ "type": "object", "properties": properties, "additionalProperties": false })
}

/// The JSON Schema (2020-12) of `weft.json` as a file on disk, published as
/// `schemas/weft.schema.json`. `additionalProperties: false` lets editors flag unknown keys; the
/// loader itself only warns about them (`W702`).
pub fn project_file_schema() -> Json {
    let mut properties = Object::new();
    properties.insert(
        "$schema".to_owned(),
        json!({
            "type": "string",
            "description": "This schema, for editors. Ignored by Weft.",
        }),
    );
    properties.insert("tokens".to_owned(), json!({
        "oneOf": [
            {
                "type": "array",
                "maxItems": MAX_TOKEN_FILES,
                "items": { "$ref": "#/$defs/fileName" },
            },
            { "$ref": "#/$defs/fileName" },
        ],
        "description": "DTCG token files, in layer order: a later file overrides an earlier one; or one DTCG resolver document (`*.resolver.json`) whose modifiers give contexts such as light and dark (SPEC §10.3).",
    }));
    properties.insert("catalog".to_owned(), json!({
        "$ref": "#/$defs/fileName",
        "description": "A catalog that extends the core catalog with the project's kinds, props and variants (SPEC §10.4).",
    }));
    properties.insert(
        "actions".to_owned(),
        json!({
            "type": "array",
            "items": { "type": "string", "pattern": "^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)*$" },
            "description": "The host's action names; on-* values are checked against them.",
        }),
    );
    properties.insert("data".to_owned(), json!({
        "$ref": "#/$defs/fileName",
        "description": "A JSON Schema of the host data model; bindings are checked against it (SPEC §10.5).",
    }));
    for setting in SECTIONS {
        properties.insert(setting.name.to_owned(), schema_of(setting));
    }
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Weft project file (weft.json)",
        "description": "Shared resources and tool settings of a Weft project (SPEC §10). Every member is optional; an argument given to a tool overrides the file, and the file overrides the defaults.",
        "type": "object",
        "properties": properties,
        "additionalProperties": false,
        "$defs": {
            "fileName": {
                "type": "string",
                "minLength": 1,
                "pattern": "^(?!/)(?!.*//)(?!.*/$)(?!(?:.*/)?\\.\\.(?:/|$))[^\\\\:\\u0000]+$",
                "description": "Relative to the project file, separated by \"/\", inside the project directory.",
            },
        },
    })
}
