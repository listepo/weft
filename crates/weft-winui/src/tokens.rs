//! `WeftTokens.xaml`, the resource dictionary a project's pages share, merged into `App.xaml`.
//! It holds the dimension tokens, the only tokens the pages reference so far (`Spacing`); the
//! colours, materials and theme dictionaries follow (T105.5).

use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::{escape_attribute, js_number};

/// The file name `generate_tokens` is written to.
pub const TOKENS_FILE: &str = "WeftTokens.xaml";

/// Effective pixels per `rem`, as the other targets count it.
const PX_PER_REM: f64 = 16.0;

/// The dictionary, and the tokens it leaves out with the reason, as `<path>: <why>`.
pub fn generate_tokens(tokens: &IndexMap<String, Token>) -> (String, Vec<String>) {
    let mut xaml = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ResourceDictionary\n    xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\"\n    xmlns:x=\"http://schemas.microsoft.com/winfx/2006/xaml\">\n",
    );
    let mut skipped = vec![];
    for (path, token) in tokens {
        if token.kind != "dimension" {
            skipped.push(format!(
                "{path}: only dimension tokens are written, not {}",
                token.kind
            ));
            continue;
        }
        let Some(px) = pixels(token) else {
            skipped.push(format!("{path}: not a dimension in px or rem"));
            continue;
        };
        // Token paths come from the project's files: the key is escaped as any attribute value.
        xaml.push_str(&format!(
            "    <x:Double x:Key=\"Weft.{}\">{}</x:Double>\n",
            escape_attribute(path),
            js_number(px)
        ));
    }
    xaml.push_str("</ResourceDictionary>\n");
    (xaml, skipped)
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
