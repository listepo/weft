//! Design tokens as Swift (SPEC §9): a struct of nested groups named after the token paths, so
//! `{token.space.md}` reads `theme.space.md`. A project shares one `WeftTokens` struct holding
//! every token it can express (`generate_tokens`); a screen generated without one carries its own
//! `<Screen>Theme` with the tokens it references, so the file stays self-contained.
//!
//! With an appearance (SPEC §10.3), a colour whose dark value differs is built by the struct's
//! `adaptive(light:dark:)`, which follows the system appearance; a typography token with letter
//! spacing or line height is the struct's `Typography` view modifier rather than a `Font`. Both
//! are nested in the struct, so neither the shared tokens nor any number of screen themes clash
//! in one module, and token sets without them print exactly as before. A `material` token is the
//! struct's `Surface` view modifier (Liquid Glass from iOS 26 and macOS 26, a system material
//! before).

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde_json::Value as Json;
use weft_catalog::{Appearance, MATERIAL, Token, composite_part, font_weight, material_parts};

use crate::Unsupported;
use crate::swift::{self, number_literal, string_literal};

/// The type of the shared tokens; screens that use it declare `var theme = WeftTokens()`.
pub const TOKENS_TYPE: &str = "WeftTokens";

/// The file name `generate_tokens` output is written to.
pub const TOKENS_FILE: &str = "WeftTokens.swift";

/// CSS `rem` against the browser default; SwiftUI points match CSS pixels.
const POINTS_PER_REM: f64 = 16.0;

/// A token as a stored property: its Swift type and initial value.
struct Leaf {
    ty: String,
    value: String,
}

/// What the leaves of one struct need declared at its root.
#[derive(Default)]
struct Needs {
    adaptive: bool,
    typography: bool,
    surface: bool,
}

/// The adaptive colour helper, nested in the root struct. UIKit and AppKit resolve a dynamic
/// colour per trait collection or appearance; SwiftUI on iOS 17 and macOS 14 has no `Color`
/// initialiser of its own for that.
const ADAPTIVE: &str = r#"/// A colour that follows the system's light or dark appearance.
static func adaptive(light: Color, dark: Color) -> Color {
    #if canImport(UIKit)
    Color(uiColor: UIColor { traits in
        UIColor(traits.userInterfaceStyle == .dark ? dark : light)
    })
    #else
    Color(nsColor: NSColor(name: nil) { appearance in
        NSColor(appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light)
    })
    #endif
}"#;

/// The typography modifier, nested in the root struct. `tracking(_:)` is the letter spacing.
/// SwiftUI before iOS 26 and macOS 26 has no line height, only `lineSpacing(_:)`, the space added
/// between lines; so the space is the CSS line box (`lineHeight` × size) less the font's own
/// line height, and half of it pads the first and last line as CSS half-leading does.
const TYPOGRAPHY: &str = r#"/// A typography token: apply it with `.modifier(theme.<path>)`.
struct Typography: ViewModifier, Sendable {
    var font: Font
    var size: CGFloat
    /// The font's family, for its line height; `nil` for the system font.
    var family: String?
    /// Letter spacing in points.
    var tracking: CGFloat = 0
    /// Line height as a multiple of `size`; `nil` keeps the font's own.
    var lineHeight: Double?

    func body(content: Content) -> some View {
        let extra = lineHeight.map { max(0, CGFloat($0) * size - naturalLineHeight) } ?? 0
        content
            .font(font)
            .tracking(tracking)
            .lineSpacing(extra)
            .padding(.vertical, extra / 2)
    }

    private var naturalLineHeight: CGFloat {
        #if canImport(UIKit)
        (family.flatMap { UIFont(name: $0, size: size) } ?? .systemFont(ofSize: size)).lineHeight
        #else
        let font = family.flatMap { NSFont(name: $0, size: size) } ?? .systemFont(ofSize: size)
        return font.ascender - font.descender + font.leading
        #endif
    }
}"#;

/// The material modifier, nested in the root struct. Liquid Glass (iOS 26 and macOS 26) has no
/// blur radius to set, so the radius only picks the thickness of the system material that stands
/// in on earlier systems, which has a fixed set; the tint is applied in both. The glass branch is
/// compiled only by a toolchain whose SDK has it (Swift 6.2 shipped with Xcode 26).
const SURFACE: &str = r#"/// A material token: apply it with `.modifier(theme.<path>)`.
struct Surface: ViewModifier, Sendable {
    var tint: Color
    /// The background blur in points.
    var blur: CGFloat

    func body(content: Content) -> some View {
        #if compiler(>=6.2)
        if #available(iOS 26, macOS 26, *) {
            content.glassEffect(.regular.tint(tint), in: Rectangle())
        } else {
            standIn(content)
        }
        #else
        standIn(content)
        #endif
    }

    private func standIn(_ content: Content) -> some View {
        content.background(tint).background(thickness, in: Rectangle())
    }

    private var thickness: Material {
        switch blur {
        case ..<10: .ultraThinMaterial
        case ..<20: .thinMaterial
        case ..<30: .regularMaterial
        case ..<50: .thickMaterial
        default: .ultraThickMaterial
        }
    }
}"#;

#[derive(Default)]
struct Group {
    leaves: BTreeMap<String, Leaf>,
    groups: BTreeMap<String, Group>,
}

/// `theme.space.md` for the token path `space.md`.
pub fn token_expr(path: &str) -> String {
    let mut out = String::from("theme");
    for segment in path.split('.') {
        out.push('.');
        out.push_str(&swift::token_member(segment));
    }
    out
}

/// The whole token set as `WeftTokens.swift`, and the tokens left out because SwiftUI has no
/// form for their type or value.
pub fn generate_tokens(
    tokens: &IndexMap<String, Token>,
    appearance: Option<Appearance<'_>>,
) -> (String, Vec<Unsupported>) {
    let mut skipped = vec![];
    let lines = tokens_struct(
        TOKENS_TYPE,
        tokens.keys().map(String::as_str),
        tokens,
        appearance,
        &mut skipped,
    );
    let mut out = vec![
        "// Generated by `weft swiftui-tokens` from the project's design tokens. Regenerate it"
            .to_owned(),
        "// instead of editing it: every screen of the project reads its values.".to_owned(),
        String::new(),
        "import SwiftUI".to_owned(),
        String::new(),
        "/// The project's design tokens: colours, sizes in points, fonts and timing.".to_owned(),
    ];
    out.extend(lines);
    let mut text = out.join("\n");
    text.push('\n');
    (text, skipped)
}

/// Whether a screen can refer to `path` in the shared tokens: the reason when it cannot.
pub fn check_token(path: &str, tokens: &IndexMap<String, Token>) -> Option<Unsupported> {
    let token = tokens.get(path);
    let problem = match token.map(|t| swift_value(t, tokens, "")) {
        None => "the token is not in the token set".to_owned(),
        Some(Err(message)) => message,
        Some(Ok(_)) => return None,
    };
    Some(Unsupported::new(format!("{{token.{path}}}"), problem))
}

/// The lines of a struct named `name` holding the tokens at `paths`, indented by the caller.
/// A token that cannot be expressed is reported in `problems` and left out.
pub fn tokens_struct<'p>(
    name: &str,
    paths: impl IntoIterator<Item = &'p str>,
    tokens: &IndexMap<String, Token>,
    appearance: Option<Appearance<'_>>,
    problems: &mut Vec<Unsupported>,
) -> Vec<String> {
    let mut root = Group::default();
    let mut needs = Needs::default();
    for path in paths {
        if let Some(problem) = check_token(path, tokens) {
            problems.push(problem);
            continue;
        }
        let Some(Ok(mut leaf)) = tokens.get(path).map(|t| swift_value(t, tokens, name)) else {
            continue;
        };
        needs.typography |= leaf.ty.ends_with(".Typography");
        needs.surface |= leaf.ty.ends_with(".Surface");
        if let Some(value) = appearance.and_then(|a| adaptive(path, a, name)) {
            leaf.value = value;
            needs.adaptive = true;
        }
        let segments: Vec<&str> = path.split('.').collect();
        let Some((last, groups)) = segments.split_last() else {
            continue;
        };
        let mut group = &mut root;
        for segment in groups {
            group = group.groups.entry((*segment).to_owned()).or_default();
        }
        group.leaves.insert((*last).to_owned(), leaf);
    }
    // A member named like a nested type would clash with it.
    for (needed, name, what) in [
        (needs.typography, "Typography", "typography"),
        (needs.surface, "Surface", "material"),
    ] {
        if needed && (root.leaves.remove(name).is_some() || root.groups.remove(name).is_some()) {
            problems.push(Unsupported::new(
                format!("{{token.{name}}}"),
                format!("`{name}` is the Swift name of the {what} type of the tokens"),
            ));
        }
    }
    let mut lines = vec![];
    print_group(name, &root, &mut lines, "", "", problems);
    // The helpers go last in the root struct, so token sets that need neither print as before.
    let mut helpers = vec![];
    if needs.adaptive {
        helpers.push(ADAPTIVE);
    }
    if needs.typography {
        helpers.push(TYPOGRAPHY);
    }
    if needs.surface {
        helpers.push(SURFACE);
    }
    if !helpers.is_empty() {
        let close = lines.pop().unwrap_or_default();
        for helper in helpers {
            lines.push(String::new());
            lines.extend(helper.lines().map(|l| {
                if l.is_empty() {
                    String::new()
                } else {
                    format!("    {l}")
                }
            }));
        }
        lines.push(close);
    }
    lines
}

/// The adaptive colour for `path` when its light and dark values differ.
fn adaptive(path: &str, appearance: Appearance<'_>, root: &str) -> Option<String> {
    let value = |tokens: &IndexMap<String, Token>| {
        let token = tokens.get(path)?;
        match token.kind.as_str() {
            "color" => color(&token.value),
            MATERIAL => material_parts(token).and_then(|(tint, _)| color(tint)),
            _ => None,
        }
    };
    let light = value(appearance.light)?;
    let dark = value(appearance.dark)?;
    if light == dark {
        return None;
    }
    let adaptive = format!("{root}.adaptive(light: {light}, dark: {dark})");
    match appearance.light.get(path).and_then(material_parts) {
        // The blur is one value: a stored property cannot change with the appearance, and the
        // system material that takes it has a fixed set of thicknesses anyway.
        Some((_, blur)) => Some(surface(root, &adaptive, blur)),
        None => Some(adaptive),
    }
}

fn surface(root: &str, tint: &str, blur: f64) -> String {
    format!(
        "{root}.Surface(tint: {tint}, blur: {})",
        number_literal(blur)
    )
}

fn print_group(
    name: &str,
    group: &Group,
    out: &mut Vec<String>,
    indent: &str,
    path: &str,
    problems: &mut Vec<Unsupported>,
) {
    out.push(format!("{indent}struct {name}: Sendable {{"));
    let inner = format!("{indent}    ");
    let mut types: Vec<(String, &str, &Group)> = vec![];
    for (segment, group) in &group.groups {
        if group.leaves.is_empty() && group.groups.is_empty() {
            continue;
        }
        let type_name = group_type_name(segment, types.iter().map(|(t, _, _)| t.as_str()));
        out.push(format!(
            "{inner}var {} = {type_name}()",
            swift::token_member(segment)
        ));
        types.push((type_name, segment, group));
    }
    let full = |segment: &str| {
        if path.is_empty() {
            segment.to_owned()
        } else {
            format!("{path}.{segment}")
        }
    };
    for (segment, leaf) in &group.leaves {
        if group.groups.contains_key(segment) {
            problems.push(Unsupported::new(
                format!("{{token.{}}}", full(segment)),
                "a token path is both a token and a group of the referenced tokens",
            ));
            continue;
        }
        if segment == "_root" && group.leaves.contains_key("$root") {
            problems.push(Unsupported::new(
                format!("{{token.{}}}", full(segment)),
                "`_root` is the Swift name of the group's `$root` token",
            ));
            continue;
        }
        out.push(format!(
            "{inner}var {}: {} = {}",
            swift::token_member(segment),
            leaf.ty,
            leaf.value
        ));
    }
    for (type_name, segment, group) in types {
        out.push(String::new());
        print_group(&type_name, group, out, &inner, &full(segment), problems);
    }
    out.push(format!("{indent}}}"));
}

/// `space` → `SpaceTokens`. The suffix keeps a group named `color` or `font` from shadowing the
/// SwiftUI type its tokens are declared with.
fn group_type_name<'a>(segment: &str, taken: impl Iterator<Item = &'a str> + Clone) -> String {
    let mut chars = segment.chars();
    let stem = match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {
            let candidate: String = first.to_uppercase().chain(chars).collect();
            if swift::is_plain_identifier(&candidate) {
                candidate
            } else {
                "Group".to_owned()
            }
        }
        _ => "Group".to_owned(),
    };
    let base = format!("{stem}Tokens");
    let mut name = base.clone();
    let mut n = 2;
    while taken.clone().any(|t| t == name) {
        name = format!("{base}{n}");
        n += 1;
    }
    name
}

// ---- Values ----

/// The Swift type and value of a token, or why SwiftUI has none.
/// `root` names the struct whose `Typography` a typography token with letter spacing or line
/// height is built with.
fn swift_value(
    token: &Token,
    tokens: &IndexMap<String, Token>,
    root: &str,
) -> Result<Leaf, String> {
    let v = &token.value;
    let leaf = |ty: &str, value: String| Leaf {
        ty: ty.to_owned(),
        value,
    };
    let leaf = match token.kind.as_str() {
        "color" => color(v).map(|value| leaf("Color", value)),
        "dimension" => points(v).map(|n| leaf("CGFloat", number_literal(n))),
        "number" => finite(v).map(|n| leaf("Double", number_literal(n))),
        "fontFamily" => {
            families(v).map(|names| leaf("[String]", format!("[{}]", names.join(", "))))
        }
        "fontWeight" => weight(v).map(|w| leaf("Font.Weight", format!(".{w}"))),
        "duration" => seconds(v).map(|s| leaf("Double", number_literal(s))),
        "cubicBezier" => bezier(v).map(|value| leaf("UnitCurve", value)),
        "typography" => typography(v, tokens, root),
        MATERIAL => material_parts(token).and_then(|(tint, blur)| {
            color(tint).map(|tint| leaf(&format!("{root}.Surface"), surface(root, &tint, blur)))
        }),
        other => return Err(format!("SwiftUI has no form for `{other}` tokens")),
    };
    leaf.ok_or_else(|| {
        format!(
            "the `{}` token's value has no SwiftUI form (README: \"Tokens\")",
            token.kind
        )
    })
}

fn finite(v: &Json) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}

fn points(v: &Json) -> Option<f64> {
    let number = finite(v.get("value")?)?;
    match v.get("unit")?.as_str()? {
        "px" => Some(number),
        "rem" => Some(number * POINTS_PER_REM),
        _ => None,
    }
}

/// Four decimals keep every 8-bit channel exact after rounding.
fn channel(n: f64) -> String {
    number_literal((n * 10_000.0).round() / 10_000.0)
}

/// DTCG 2025.10 colours are objects: components in a colour space SwiftUI has, else the `hex`
/// fallback the format requires for any other space.
fn color(v: &Json) -> Option<String> {
    // The string form of earlier DTCG drafts is still common (the example project uses it).
    if let Some(s) = v.as_str() {
        let (values, alpha) = hex(s)?;
        return Some(rgb(".sRGB", &values, alpha));
    }
    let space = match v.get("colorSpace")?.as_str()? {
        "srgb" => Some(".sRGB"),
        "srgb-linear" => Some(".sRGBLinear"),
        "display-p3" => Some(".displayP3"),
        _ => None,
    };
    let alpha = match v.get("alpha") {
        None => 1.0,
        Some(a) => finite(a).filter(|a| (0.0..=1.0).contains(a))?,
    };
    let components = v.get("components").and_then(Json::as_array);
    if let (Some(space), Some(components)) = (space, components)
        && components.len() == 3
    {
        // `none` is a missing component, which reads as zero.
        let values: Option<Vec<f64>> = components
            .iter()
            .map(|c| if c == "none" { Some(0.0) } else { finite(c) })
            .collect();
        if let Some(values) = values {
            return Some(rgb(space, &values, alpha));
        }
    }
    let (values, hex_alpha) = hex(v.get("hex")?.as_str()?)?;
    let alpha = if v.get("alpha").is_some() {
        alpha
    } else {
        hex_alpha
    };
    Some(rgb(".sRGB", &values, alpha))
}

fn rgb(space: &str, c: &[f64], alpha: f64) -> String {
    format!(
        "Color({space}, red: {}, green: {}, blue: {}, opacity: {})",
        channel(c[0]),
        channel(c[1]),
        channel(c[2]),
        channel(alpha)
    )
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` as channels from 0 to 1, and the alpha.
fn hex(text: &str) -> Option<(Vec<f64>, f64)> {
    let digits = text.strip_prefix('#')?;
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let pairs: Vec<String> = match digits.len() {
        3 | 4 => digits.chars().map(|c| format!("{c}{c}")).collect(),
        6 | 8 => digits
            .as_bytes()
            .chunks(2)
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect(),
        _ => return None,
    };
    let values: Vec<f64> = pairs
        .iter()
        .map(|p| u8::from_str_radix(p, 16).map(|n| f64::from(n) / 255.0))
        .collect::<Result<_, _>>()
        .ok()?;
    let alpha = values.get(3).copied().unwrap_or(1.0);
    Some((values[..3].to_vec(), alpha))
}

/// A family or a list of fallbacks, as Swift string literals.
fn families(v: &Json) -> Option<Vec<String>> {
    match v {
        Json::String(s) if !s.is_empty() => Some(vec![string_literal(s)]),
        Json::Array(list) if !list.is_empty() => list
            .iter()
            .map(|f| f.as_str().filter(|s| !s.is_empty()).map(string_literal))
            .collect(),
        _ => None,
    }
}

/// The `Font.Weight` for a DTCG weight. SwiftUI's nine weights stand for the hundreds, from
/// `ultraLight` (100) to `black` (900).
fn weight(v: &Json) -> Option<&'static str> {
    const WEIGHTS: [&str; 9] = [
        "ultraLight",
        "thin",
        "light",
        "regular",
        "medium",
        "semibold",
        "bold",
        "heavy",
        "black",
    ];
    let step = (font_weight(v)? / 100.0).round().clamp(1.0, 9.0) as usize;
    Some(WEIGHTS[step - 1])
}

fn seconds(v: &Json) -> Option<f64> {
    let number = finite(v.get("value")?)?;
    match v.get("unit")?.as_str()? {
        "ms" => Some(number / 1000.0),
        "s" => Some(number),
        _ => None,
    }
}

fn bezier(v: &Json) -> Option<String> {
    let p: Vec<f64> = v
        .as_array()
        .filter(|a| a.len() == 4)?
        .iter()
        .map(finite)
        .collect::<Option<_>>()?;
    Some(format!(
        ".bezier(startControlPoint: UnitPoint(x: {}, y: {}), endControlPoint: UnitPoint(x: {}, y: {}))",
        number_literal(p[0]),
        number_literal(p[1]),
        number_literal(p[2]),
        number_literal(p[3])
    ))
}

/// The CSS generic families SwiftUI draws with the system font, and the design each one means.
const GENERIC_FAMILIES: &[(&str, Option<&str>)] = &[
    ("system-ui", None),
    ("-apple-system", None),
    ("ui-sans-serif", None),
    ("sans-serif", None),
    ("ui-serif", Some(".serif")),
    ("serif", Some(".serif")),
    ("ui-monospace", Some(".monospaced")),
    ("monospace", Some(".monospaced")),
    ("ui-rounded", Some(".rounded")),
];

/// A typography token: a `Font`, or `<root>.Typography` when it has letter spacing (a dimension)
/// or line height (a multiple of the font size, DTCG 2025.10 §9.7).
fn typography(v: &Json, tokens: &IndexMap<String, Token>, root: &str) -> Option<Leaf> {
    let font = font(v, tokens)?;
    let tracking = match v.get("letterSpacing") {
        Some(_) => Some(points(&composite_part(v, "letterSpacing", tokens)?)?),
        None => None,
    };
    let line_height = match v.get("lineHeight") {
        Some(_) => Some(finite(&composite_part(v, "lineHeight", tokens)?).filter(|n| *n > 0.0)?),
        None => None,
    };
    if tracking.is_none() && line_height.is_none() {
        return Some(Leaf {
            ty: "Font".to_owned(),
            value: font,
        });
    }
    let size = points(&composite_part(v, "fontSize", tokens)?)?;
    let family = match family(&composite_part(v, "fontFamily", tokens)?)? {
        name if GENERIC_FAMILIES.iter().any(|(g, _)| *g == name) => "nil".to_owned(),
        name => string_literal(&name),
    };
    let mut args = vec![
        format!("font: {font}"),
        format!("size: {}", number_literal(size)),
        format!("family: {family}"),
    ];
    if let Some(t) = tracking {
        args.push(format!("tracking: {}", number_literal(t)));
    }
    if let Some(l) = line_height {
        args.push(format!("lineHeight: {}", number_literal(l)));
    }
    Some(Leaf {
        ty: format!("{root}.Typography"),
        value: format!("{root}.Typography({})", args.join(", ")),
    })
}

/// The first family of a `fontFamily` value.
fn family(v: &Json) -> Option<String> {
    let first = match v {
        Json::String(s) => s.clone(),
        Json::Array(list) => list.first()?.as_str()?.to_owned(),
        _ => return None,
    };
    (!first.is_empty()).then_some(first)
}

/// A typography token as a `Font`; letter spacing and line height are `typography`'s.
fn font(v: &Json, tokens: &IndexMap<String, Token>) -> Option<String> {
    let part = |key: &str| composite_part(v, key, tokens);
    let size = number_literal(points(&part("fontSize")?)?);
    let weight = match part("fontWeight") {
        Some(w) => Some(weight(&w)?),
        None => None,
    };
    let first = family(&part("fontFamily")?)?;
    if let Some((_, design)) = GENERIC_FAMILIES.iter().find(|(name, _)| *name == first) {
        let mut args = vec![format!("size: {size}")];
        if let Some(w) = weight {
            args.push(format!("weight: .{w}"));
        }
        if let Some(d) = design {
            args.push(format!("design: {d}"));
        }
        return Some(format!(".system({})", args.join(", ")));
    }
    let custom = format!(".custom({}, size: {size})", string_literal(&first));
    Some(match weight {
        Some(w) => format!("{custom}.weight(.{w})"),
        None => custom,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn value(kind: &str, value: Json) -> Result<String, String> {
        let token = Token {
            kind: kind.to_owned(),
            value,
        };
        swift_value(&token, &IndexMap::new(), "T").map(|l| format!("{}: {}", l.ty, l.value))
    }

    #[test]
    fn colours_use_their_space_or_fall_back_to_hex() {
        assert_eq!(
            value(
                "color",
                json!({ "colorSpace": "display-p3", "components": [1, 0.5, "none"], "alpha": 0.5 })
            ),
            Ok("Color: Color(.displayP3, red: 1, green: 0.5, blue: 0, opacity: 0.5)".to_owned())
        );
        assert_eq!(
            value(
                "color",
                json!({ "colorSpace": "oklch", "components": [0.5, 0.1, 30], "hex": "#ff000080" })
            ),
            Ok("Color: Color(.sRGB, red: 1, green: 0, blue: 0, opacity: 0.502)".to_owned())
        );
        assert_eq!(
            value("color", json!("#d97706")),
            Ok(
                "Color: Color(.sRGB, red: 0.851, green: 0.4667, blue: 0.0235, opacity: 1)"
                    .to_owned()
            )
        );
        assert!(value("color", json!("orange")).is_err());
    }

    #[test]
    fn other_types_map_to_their_swiftui_forms() {
        assert_eq!(
            value("dimension", json!({ "value": 1.5, "unit": "rem" })),
            Ok("CGFloat: 24".to_owned())
        );
        assert_eq!(
            value("duration", json!({ "value": 250, "unit": "ms" })),
            Ok("Double: 0.25".to_owned())
        );
        assert_eq!(
            value("fontWeight", json!("semi-bold")),
            Ok("Font.Weight: .semibold".to_owned())
        );
        assert_eq!(
            value("fontWeight", json!(350)),
            Ok("Font.Weight: .regular".to_owned())
        );
        assert_eq!(
            value("fontFamily", json!(["Inter", "system-ui"])),
            Ok("[String]: [\"Inter\", \"system-ui\"]".to_owned())
        );
        assert_eq!(
            value(
                "typography",
                json!({ "fontFamily": "ui-monospace", "fontSize": { "value": 12, "unit": "px" }, "fontWeight": 700 })
            ),
            Ok("Font: .system(size: 12, weight: .bold, design: .monospaced)".to_owned())
        );
        assert!(value("shadow", json!({})).is_err());
        assert!(value("dimension", json!({ "value": 1, "unit": "em" })).is_err());
    }

    #[test]
    fn letter_spacing_and_line_height_make_a_typography_modifier() {
        assert_eq!(
            value(
                "typography",
                json!({
                    "fontFamily": ["Inter", "sans-serif"],
                    "fontSize": { "value": 1, "unit": "rem" },
                    "letterSpacing": { "value": -0.5, "unit": "px" },
                    "lineHeight": 1.25
                })
            ),
            Ok("T.Typography: T.Typography(font: .custom(\"Inter\", size: 16), size: 16, family: \"Inter\", tracking: -0.5, lineHeight: 1.25)".to_owned())
        );
        assert_eq!(
            value(
                "typography",
                json!({ "fontFamily": "system-ui", "fontSize": { "value": 12, "unit": "px" }, "lineHeight": 2 })
            ),
            Ok("T.Typography: T.Typography(font: .system(size: 12), size: 12, family: nil, lineHeight: 2)".to_owned())
        );
        // A line height that is not positive has no form, as any other bad value.
        assert!(
            value(
                "typography",
                json!({ "fontFamily": "x", "fontSize": { "value": 12, "unit": "px" }, "lineHeight": -1 })
            )
            .is_err()
        );
    }

    fn tokens(pairs: &[(&str, &str)]) -> IndexMap<String, Token> {
        pairs
            .iter()
            .map(|(path, hex)| {
                let token = Token {
                    kind: "color".to_owned(),
                    value: json!(hex),
                };
                ((*path).to_owned(), token)
            })
            .collect()
    }

    #[test]
    fn only_colours_that_differ_in_the_dark_adapt() {
        let light = tokens(&[("ink", "#000000"), ("line", "#cccccc")]);
        let dark = tokens(&[("ink", "#ffffff"), ("line", "#cccccc")]);
        let appearance = Appearance {
            modifier: "theme",
            light_context: "light",
            dark_context: "dark",
            light: &light,
            dark: &dark,
        };
        let mut problems = vec![];
        let lines = tokens_struct(
            "T",
            ["ink", "line"],
            &light,
            Some(appearance),
            &mut problems,
        )
        .join("\n");
        assert!(problems.is_empty());
        assert!(lines.contains("var ink: Color = T.adaptive(light: Color(.sRGB, red: 0, green: 0, blue: 0, opacity: 1), dark: Color(.sRGB, red: 1, green: 1, blue: 1, opacity: 1))"), "{lines}");
        assert!(lines.contains("var line: Color = Color(.sRGB"), "{lines}");
        assert!(lines.contains("static func adaptive(light: Color, dark: Color) -> Color {"));
        // Without an appearance nothing is added, so token files print as before.
        let plain = tokens_struct("T", ["ink"], &light, None, &mut problems).join("\n");
        assert!(!plain.contains("adaptive"), "{plain}");
    }

    #[test]
    fn root_tokens_get_a_swift_name() {
        assert_eq!(token_expr("color.blue.$root"), "theme.color.blue._root");
    }

    #[test]
    fn a_material_is_a_surface_and_follows_the_appearance_by_its_tint() {
        let glass = |hex: &str, blur: u32| Token {
            kind: MATERIAL.to_owned(),
            value: json!({ "tint": hex, "blur": { "value": blur, "unit": "px" } }),
        };
        let one = glass("#ffffff80", 20);
        assert_eq!(
            swift_value(&one, &IndexMap::new(), "T").map(|l| l.value),
            Ok("T.Surface(tint: Color(.sRGB, red: 1, green: 1, blue: 1, opacity: 0.502), blur: 20)"
                .to_owned())
        );
        let light: IndexMap<String, Token> = [("m".to_owned(), glass("#ffffff80", 20))].into();
        let dark: IndexMap<String, Token> = [("m".to_owned(), glass("#00000080", 8))].into();
        let appearance = Appearance {
            modifier: "theme",
            light_context: "light",
            dark_context: "dark",
            light: &light,
            dark: &dark,
        };
        let got = adaptive("m", appearance, "T").unwrap();
        assert!(
            got.starts_with("T.Surface(tint: T.adaptive(light: "),
            "{got}"
        );
        assert!(got.ends_with("blur: 20)"), "{got}");
    }
}
