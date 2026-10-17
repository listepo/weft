//! `WeftTokens.xaml`, the resource dictionary a project's pages share, merged into `App.xaml`.
//! It holds the dimension tokens, each also as a `Thickness` for `Padding`, which takes no
//! `x:Double`; and in its theme dictionaries the colours, as a `Color` and a `SolidColorBrush`,
//! and the materials, as an `AcrylicBrush`, so a `{ThemeResource}` follows the system theme.

use indexmap::IndexMap;
use weft_catalog::{Appearance, MATERIAL, Token, material_parts, srgb_channels};
use weft_core::js_number;

use crate::xaml::Element;

/// The file name `generate_tokens` is written to.
pub const TOKENS_FILE: &str = "WeftTokens.xaml";

/// Effective pixels per `rem`, as the other targets count it.
const PX_PER_REM: f64 = 16.0;

/// The keys of the theme dictionaries, in the order they are written.
const THEMES: [&str; 3] = ["Light", "Dark", "HighContrast"];

/// The dictionary, and the tokens it leaves out with the reason, as `<path>: <why>`. With an
/// appearance (SPEC §10.3) the `Dark` dictionary holds the dark values; without one it holds the
/// same values as `Light`.
pub fn generate_tokens(
    tokens: &IndexMap<String, Token>,
    appearance: Option<Appearance<'_>>,
) -> (String, Vec<String>) {
    let mut xaml = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ResourceDictionary\n    xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\"\n    xmlns:x=\"http://schemas.microsoft.com/winfx/2006/xaml\">\n",
    );
    let (entries, skipped) = resources(tokens.iter(), appearance);
    for entry in &entries {
        entry.write(1, &mut xaml);
    }
    xaml.push_str("</ResourceDictionary>\n");
    let skipped = skipped
        .into_iter()
        .map(|(path, why)| format!("{path}: {why}"));
    (xaml, skipped.collect())
}

/// The entries of a dictionary holding `tokens`, `WeftTokens.xaml` or a page's own (SPEC §9,
/// To WinUI), and the tokens left out, each with why.
pub(crate) fn resources<'t>(
    tokens: impl Iterator<Item = (&'t String, &'t Token)>,
    appearance: Option<Appearance<'_>>,
) -> (Vec<Element>, Vec<(String, String)>) {
    let mut skipped = vec![];
    let mut dimensions = vec![];
    let mut themes: [Vec<Element>; 3] = Default::default();
    for (path, token) in tokens {
        match token.kind.as_str() {
            "dimension" => {
                let Some(px) = pixels(token) else {
                    skipped.push((path.clone(), "not a dimension in px or rem".to_owned()));
                    continue;
                };
                let px = js_number(px);
                dimensions.push(entry("x:Double", format!("Weft.{path}"), &px));
                dimensions.push(entry("Thickness", format!("Weft.{path}.thickness"), &px));
            }
            "color" | MATERIAL => {
                let light = appearance.and_then(|a| a.light.get(path)).unwrap_or(token);
                let dark = appearance.and_then(|a| a.dark.get(path)).unwrap_or(token);
                let (Some(light), Some(dark)) = (colour(light), colour(dark)) else {
                    let what = if token.kind == MATERIAL {
                        "a material whose tint"
                    } else {
                        "a colour that"
                    };
                    let why = format!("{what} has neither sRGB components nor a hex value");
                    skipped.push((path.clone(), why));
                    continue;
                };
                let material = token.kind == MATERIAL;
                // High contrast keeps the light colours; a material there is a solid brush of
                // its fallback, since the system turns translucency off in that theme.
                for (out, (argb, high)) in
                    themes
                        .iter_mut()
                        .zip([(&light, false), (&dark, false), (&light, true)])
                {
                    out.extend(resource(path, argb, material, high));
                }
            }
            other => skipped.push((
                path.clone(),
                format!("only dimension, colour and material tokens are written, not {other}"),
            )),
        }
    }
    let mut entries = vec![];
    if themes.iter().any(|t| !t.is_empty()) {
        let mut holder = Element::new("ResourceDictionary.ThemeDictionaries");
        for (name, body) in THEMES.iter().zip(themes) {
            let mut theme = Element::new("ResourceDictionary");
            theme.attr("x:Key", *name);
            theme.children = body;
            holder.children.push(theme);
        }
        entries.push(holder);
    }
    entries.extend(dimensions);
    (entries, skipped)
}

/// A resource whose value is its content.
fn entry(name: &'static str, key: String, value: &str) -> Element {
    let mut e = Element::new(name);
    e.attr("x:Key", key);
    e.text(value);
    e
}

/// A colour, or a material's tint, as XAML `#AARRGGBB`, and the opacity apart.
struct Argb {
    opaque: String,
    alpha: f64,
    full: String,
}

fn colour(token: &Token) -> Option<Argb> {
    let value = match material_parts(token) {
        Some((tint, _)) => tint,
        None => &token.value,
    };
    let (rgb, alpha) = srgb_channels(value)?;
    let byte = |c: f64| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    let rgb = format!(
        "{:02X}{:02X}{:02X}",
        byte(rgb[0]),
        byte(rgb[1]),
        byte(rgb[2])
    );
    Some(Argb {
        opaque: format!("#FF{rgb}"),
        alpha,
        full: format!("#{:02X}{rgb}", byte(alpha)),
    })
}

/// One theme's resources for a colour (`Weft.<path>` the brush, `Weft.<path>.color` the colour)
/// or a material (`Weft.<path>`, an `AcrylicBrush`; a solid brush in high contrast). A blur
/// radius has no `AcrylicBrush` form: the system's acrylic blur is fixed.
fn resource(path: &str, c: &Argb, material: bool, high_contrast: bool) -> Vec<Element> {
    let full = &c.full;
    let brush = |name: &'static str| {
        let mut e = Element::new(name);
        e.attr("x:Key", format!("Weft.{path}"));
        e
    };
    match (material, high_contrast) {
        (false, _) | (true, true) => {
            let mut solid = brush("SolidColorBrush");
            solid.attr("Color", full);
            if material {
                return vec![solid];
            }
            vec![entry("Color", format!("Weft.{path}.color"), full), solid]
        }
        (true, false) => {
            let mut acrylic = brush("AcrylicBrush");
            acrylic.attr("TintColor", &c.opaque);
            acrylic.attr(
                "TintOpacity",
                js_number((c.alpha * 1000.0).round() / 1000.0),
            );
            acrylic.attr("FallbackColor", full);
            vec![acrylic]
        }
    }
}

fn pixels(token: &Token) -> Option<f64> {
    let value = &token.value;
    let n = value.get("value")?.as_f64()?;
    match value.get("unit")?.as_str()? {
        "px" => Some(n),
        "rem" => Some(n * PX_PER_REM),
        _ => None,
    }
}
