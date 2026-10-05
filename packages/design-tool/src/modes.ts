// Token modes in a design tool, and back (SPEC §10.3): a resolver modifier's contexts become
// Figma variable modes or Penpot token themes, and what a file holds per mode becomes a DTCG
// Resolver Module 2025.10 document again. Values read from a file are untrusted: each one is
// checked before it becomes a token, and names are written so that no key can reach a prototype.
import type { Token, TokenModifier } from "@weft/catalog";
import { hexOf, tokenColor, tokenPx, type RGBA } from "./tokens.ts";

/** The loader reads at most this many contexts of a modifier (SPEC §10.3). */
export const MAX_CONTEXTS = 64;

/** The modifier as a request carries it: contexts as entries, which every channel keeps. */
export type ModifierEntries = {
  name: string;
  default: string;
  contexts: [string, [string, Token][]][];
};

export const modifierEntries = (m: TokenModifier): ModifierEntries => ({
  name: m.name,
  default: m.default,
  contexts: [...m.contexts].map(([name, tokens]) => [name, [...tokens]]),
});

export const modifierOf = (m: ModifierEntries): TokenModifier => ({
  name: m.name,
  default: m.default,
  contexts: new Map(m.contexts.map(([name, tokens]) => [name, new Map(tokens)])),
});

/** A value a design tool keeps for a token: pixels (or a plain number) or a colour. */
export type ModeValue = number | RGBA;

const unit = (n: unknown): n is number => typeof n === "number" && n >= 0 && n <= 1;

/** Equal as a tool shows them: the same 8-bit channels and nearly the same opacity. */
export const sameColor = (a: RGBA, b: RGBA): boolean =>
  hexOf(a) === hexOf(b) && Math.abs(a.a - b.a) < 0.005;

/**
 * The token a tool's value stands for. When the value is what `original` already gives, the
 * original comes back unchanged, so a round trip keeps `rem`, aliases' results and hex spelling;
 * otherwise a number stays a number token and pixels become a `px` dimension.
 */
export function modeToken(original: Token | undefined, value: unknown): Token | undefined {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return undefined;
    if (original !== undefined && tokenPx(original) === value) return original;
    return original?.type === "number"
      ? { type: "number", value }
      : { type: "dimension", value: { value, unit: "px" } };
  }
  if (typeof value !== "object" || value === null) return undefined;
  const { r, g, b, a } = value as Record<string, unknown>;
  if (!unit(r) || !unit(g) || !unit(b) || (a !== undefined && !unit(a))) return undefined;
  const color = { r, g, b, a: a ?? 1 };
  const was = tokenColor(original);
  if (original !== undefined && was !== undefined && sameColor(was, color)) return original;
  return {
    type: "color",
    value: {
      colorSpace: "srgb",
      components: [r, g, b],
      ...(color.a < 1 ? { alpha: color.a } : {}),
      hex: hexOf(color),
    },
  };
}

// A DTCG name may not start with `$` or hold `{`, `}` or `.` (Format 2025.10 §5.1.1 Character restrictions).
const NAME = /^[^${}.][^{}.]*$/;

/** A token tree of `tokens`; a path that is not a valid name, or clashes with another, is left out. */
function tokenTree(tokens: Iterable<[string, Token]>): Record<string, unknown> {
  const root: Record<string, unknown> = Object.create(null);
  for (const [path, token] of tokens) {
    const names = path.split(".");
    if (!names.every((n) => NAME.test(n))) continue;
    let group: Record<string, unknown> | undefined = root;
    for (const name of names.slice(0, -1)) {
      const next: unknown = group[name];
      if (next === undefined) {
        const made: Record<string, unknown> = Object.create(null);
        group[name] = made;
        group = made;
      } else if (typeof next === "object" && next !== null && !("$value" in next)) {
        group = next as Record<string, unknown>;
      } else {
        group = undefined;
        break;
      }
    }
    const leaf = names.at(-1) as string;
    if (group === undefined || leaf in group) continue;
    group[leaf] = { $type: token.type, $value: token.value };
  }
  return root;
}

const same = (a: Token | undefined, b: Token): boolean =>
  a !== undefined && a.type === b.type && JSON.stringify(a.value) === JSON.stringify(b.value);

// JSON Pointer escaping (RFC 6901 §4): `~` first, so the `~1` written for `/` stays as it is.
const pointer = (name: string) => name.replaceAll("~", "~0").replaceAll("/", "~1");

/**
 * A resolver document with one set, the default context's tokens, and the modifier, each context
 * holding only the tokens that differ from the default. Loading it gives every context the tokens
 * `modifier` has (SPEC §10.3). A modifier without its default context yields nothing.
 */
export function resolverDocument(modifier: TokenModifier): Record<string, unknown> | undefined {
  const base = modifier.contexts.get(modifier.default);
  if (base === undefined) return undefined;
  const contexts: Record<string, unknown> = Object.create(null);
  for (const [name, tokens] of [...modifier.contexts].slice(0, MAX_CONTEXTS)) {
    const differs = [...tokens].filter(([path, token]) => !same(base.get(path), token));
    contexts[name] = differs.length === 0 ? [] : [tokenTree(differs)];
  }
  return {
    $schema: "https://www.designtokens.org/schemas/2025.10/resolver.json",
    version: "2025.10",
    sets: { base: { sources: [tokenTree(base)] } },
    modifiers: { [modifier.name]: { contexts, default: modifier.default } },
    resolutionOrder: [{ $ref: "#/sets/base" }, { $ref: `#/modifiers/${pointer(modifier.name)}` }],
  };
}
