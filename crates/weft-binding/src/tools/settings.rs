//! How a tool host runs: the limits on what a model can make it chew on, the default validation
//! mode and whether patches may change context. They are the `validate.mode`, `mcp.limits` and
//! `mcp.context` sections of `weft.json` (SPEC §10.6), read in that shape so that no host grows
//! an option of its own; a tool argument never sets them, so a model cannot lift its own bounds.

use serde::Deserialize;
use serde_json::Value as Json;
use weft_core::Mode;

/// Default bounds; tool arguments are untrusted input, and the numbers are generous for real
/// screens (the largest corpus screen is under 3,000 characters). Lengths count UTF-16 code
/// units, as the TypeScript server's argument checks did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Limits {
    /// One markup argument.
    pub markup_chars: usize,
    /// The JSON text of `weft_render`'s `data` argument.
    pub data_chars: usize,
    /// Patches in one `weft_patch` call.
    pub patches: usize,
    /// The JSON text of the whole patch list.
    pub patches_chars: usize,
    /// The JSON text of the `project` argument (SPEC §10).
    pub project_chars: usize,
    /// Diagnostics returned in one result; the rest are counted, not listed.
    pub diagnostics: usize,
    /// JSON values in one call's arguments; enforced by the MCP SDK, which reads it from here.
    pub input_elements: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            markup_chars: 200_000,
            data_chars: 200_000,
            patches: 100,
            patches_chars: 200_000,
            project_chars: 500_000,
            diagnostics: 40,
            input_elements: 20_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextPolicy {
    #[default]
    ReadWrite,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub limits: Limits,
    /// The default of `weft_validate`'s `strict`; writers' tools always check strictly.
    pub mode: Mode,
    /// Whether `weft_patch` may change context; `weft_context` reads either way.
    pub context: ContextPolicy,
}

#[derive(Default, Deserialize)]
struct Wire {
    #[serde(default)]
    validate: WireValidate,
    #[serde(default)]
    mcp: WireMcp,
}

#[derive(Default, Deserialize)]
struct WireValidate {
    mode: Option<WireMode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireMode {
    Lenient,
    Strict,
}

#[derive(Default, Deserialize)]
struct WireMcp {
    #[serde(default)]
    limits: Limits,
    #[serde(default)]
    context: ContextPolicy,
}

impl Settings {
    /// The settings of `weft.json`'s tool sections; keys that are absent keep their defaults.
    /// The project loader has already checked the sections, so a shape it would refuse is a
    /// host bug and reads as an error.
    pub fn from_json(settings: &Json) -> Result<Settings, serde_json::Error> {
        let wire = Wire::deserialize(settings)?;
        Ok(Settings {
            limits: wire.mcp.limits,
            mode: match wire.validate.mode {
                Some(WireMode::Strict) => Mode::Strict,
                _ => Mode::Lenient,
            },
            context: wire.mcp.context,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absent_sections_keep_the_defaults() {
        assert_eq!(
            Settings::from_json(&json!({})).unwrap(),
            Settings::default()
        );
    }

    #[test]
    fn weft_json_sections_set_mode_limits_and_context() {
        let s = Settings::from_json(&json!({
            "validate": {"mode": "strict"},
            "mcp": {"limits": {"markupChars": 80}, "context": "read-only"},
            "render": {"outDir": "x"}
        }))
        .unwrap();
        assert_eq!(s.mode, Mode::Strict);
        assert_eq!(s.limits.markup_chars, 80);
        assert_eq!(s.limits.patches, 100);
        assert_eq!(s.context, ContextPolicy::ReadOnly);
    }
}
