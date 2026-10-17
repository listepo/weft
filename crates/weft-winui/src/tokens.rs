//! `WeftTokens.xaml`, the resource dictionary a project's pages share, merged into `App.xaml`.
//! It holds the dimension tokens, each also as a `Thickness` for `Padding`, which takes no
//! `x:Double`; and in its theme dictionaries the colours, as a `Color` and a `SolidColorBrush`,
//! and the materials, as an `AcrylicBrush`, so a `{ThemeResource}` follows the system theme.

use indexmap::IndexMap;
use weft_catalog::{Appearance, MATERIAL, Token, material_parts, srgb_channels};
use weft_core::{escape_attribute, js_number};

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
    let mut skipped = vec![];
    let mut dimensions = String::new();
    let mut themes: [String; 3] = Default::default();
    for (path, token) in tokens {
        // Token paths come from the project's files: the key is escaped as any attribute value.
        let key = escape_attribute(path);
        match token.kind.as_str() {
            "dimension" => {
                let Some(px) = pixels(token) else {
                    skipped.push(format!("{path}: not a dimension in px or rem"));
                    continue;
                };
                let px = js_number(px);
                dimensions.push_str(&format!(
                    "    <x:Double x:Key=\"Weft.{key}\">{px}</x:Double>\n    <Thickness x:Key=\"Weft.{key}.thickness\">{px}</Thickness>\n"
                ));
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
                    skipped.push(format!(
                        "{path}: {what} has neither sRGB components nor a hex value"
                    ));
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
                    out.push_str(&resource(&key, argb, material, high));
                }
            }
            other => skipped.push(format!(
                "{path}: only dimension, colour and material tokens are written, not {other}"
            )),
        }
    }
    if themes.iter().any(|t| !t.is_empty()) {
        xaml.push_str("    <ResourceDictionary.ThemeDictionaries>\n");
        for (name, body) in THEMES.iter().zip(&themes) {
            xaml.push_str(&format!(
                "        <ResourceDictionary x:Key=\"{name}\">\n{body}        </ResourceDictionary>\n"
            ));
        }
        xaml.push_str("    </ResourceDictionary.ThemeDictionaries>\n");
    }
    xaml.push_str(&dimensions);
    xaml.push_str("</ResourceDictionary>\n");
    (xaml, skipped)
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
fn resource(key: &str, c: &Argb, material: bool, high_contrast: bool) -> String {
    const IN: &str = "            ";
    let full = &c.full;
    match (material, high_contrast) {
        (false, _) => format!(
            "{IN}<Color x:Key=\"Weft.{key}.color\">{full}</Color>\n{IN}<SolidColorBrush x:Key=\"Weft.{key}\" Color=\"{full}\" />\n"
        ),
        (true, true) => format!("{IN}<SolidColorBrush x:Key=\"Weft.{key}\" Color=\"{full}\" />\n"),
        (true, false) => format!(
            "{IN}<AcrylicBrush x:Key=\"Weft.{key}\" TintColor=\"{}\" TintOpacity=\"{}\" FallbackColor=\"{full}\" />\n",
            c.opaque,
            js_number((c.alpha * 1000.0).round() / 1000.0)
        ),
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
