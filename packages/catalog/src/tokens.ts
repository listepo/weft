// Design Tokens loader for the DTCG 2025.10 format
// (https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/).
//
// Supported subset: groups and tokens (`$value`), `$type` on a token or inherited from the nearest
// ancestor group, the `$root` token name, and whole-value aliases written `{group.token}`
// (chains allowed; a token without its own `$type` takes the type of the token it aliases).
// A `color` token with a `dev.weft.material` extension is a `material` token (SPEC §10.3); other
// `$`-properties (`$description`, `$extensions`, `$deprecated`) are ignored.
// Not supported: `$extends`, JSON-pointer `$ref`, the resolver module, and aliases nested inside
// composite values - such an object is kept as-is, so its inner aliases stay unresolved.
// Bad input never throws: it is reported as problems and the offending tokens are left out.
// The loader is the Rust catalog crate (crates/weft-catalog/src/tokens.rs) through WebAssembly.
import { toJson, wasm } from "@weft/core/wasm";

export type Token = { type: string; value: unknown };

export type TokenProblem = {
  code: "T001" | "T002" | "T003" | "T004" | "T005" | "T006" | "T007";
  path: string;
  message: string;
};

export function loadTokens(json: unknown): {
  tokens: Map<string, Token>;
  problems: TokenProblem[];
} {
  const out = JSON.parse(wasm.loadTokens(toJson(json))) as {
    tokens: [string, Token][];
    problems: TokenProblem[];
  };
  return { tokens: new Map(out.tokens), problems: out.problems };
}

export function tokenTypes(tokens: Map<string, Token>): Map<string, string> {
  return new Map([...tokens].map(([path, token]) => [path, token.type]));
}
