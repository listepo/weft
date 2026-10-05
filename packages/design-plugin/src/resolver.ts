// A DTCG resolver document the user pastes or picks (SPEC §10.3), as the plugin UI uses it: the
// modifiers a build can turn into modes, and the tokens of the default input. The document is
// untrusted: it is size-bounded here, parsed as JSON only, and read by the project loader (the
// Rust resolver module through WebAssembly), which reports every breach as `W705` while the rest
// still loads. No resolver parser lives in TypeScript.
import { isResolver, loadProject, type Token, type TokenModifier } from "@weft/catalog";
import type { Diagnostic } from "@weft/core";
import { MAX_MARKUP } from "@weft/design-tool";

export type LoadedResolver = {
  /** The default context's tokens: the file's base collection. Absent when nothing loaded. */
  tokens?: Map<string, Token>;
  modifiers: TokenModifier[];
  /** The modifier a build uses unless the user picks another: the appearance one. */
  appearance?: string;
  diagnostics: Diagnostic[];
  /** Why nothing loaded. */
  message?: string;
};

/** A resolver's text larger than this is refused before parsing, as a `.weft` file is. */
export const MAX_RESOLVER = MAX_MARKUP;

export function readResolver(text: string): LoadedResolver {
  const refused = (message: string): LoadedResolver => ({
    modifiers: [],
    diagnostics: [],
    message,
  });
  if (text.length > MAX_RESOLVER)
    return refused(`The resolver is larger than ${MAX_RESOLVER} characters.`);
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    return refused("The resolver is not valid JSON.");
  }
  if (!isResolver(json)) return refused("The document is not a resolver: no resolutionOrder.");
  // The loader's project file takes a resolver as `tokens`, given as content (SPEC §10.1).
  const { project, diagnostics } = loadProject({ tokens: json });
  return {
    ...(project.tokens === undefined ? {} : { tokens: project.tokens }),
    modifiers: project.modifiers ?? [],
    ...(project.appearance === undefined ? {} : { appearance: project.appearance.modifier }),
    diagnostics,
  };
}
