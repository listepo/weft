// Token modes as Penpot token themes (SPEC §10.3): a resolver modifier is a theme group, each of
// its contexts a theme. Every theme turns on the library's base set; a context other than the
// default also turns on a set of its own with the tokens whose value differs, which wins because
// it comes later. A theme carries no plugin data of its own, so the build records the default
// context's name on the library as shared plugin data (`Library` extends `PluginData`, and private
// data would not survive another install, see `NAMESPACE`); read back, that theme is the
// default, and the first theme of the group stands in when nothing is recorded or it was deleted.
// Token values in the file are untrusted text: what does not parse is left out.
import type { Token, TokenModifier } from "@weft/catalog";
import {
  hexOf,
  MAX_CONTEXTS,
  modeToken,
  type PluginData,
  resolverDocument,
  TOKEN_COLLECTION,
  tokenColor,
  tokenPx,
  type ModeValue,
  type RGBA,
} from "@weft/design-tool";
import type { PTokenCatalog, PTokenSet, PTokenTheme, PTokenType } from "./api.ts";

/** The group of the context sets: `Weft modes/<modifier>/<context>`. */
export const MODES_GROUP = "Weft modes";

export function tokenValue(
  token: Token | undefined,
): { type: PTokenType; text: string; px?: number; color?: RGBA } | undefined {
  const px = tokenPx(token);
  if (px !== undefined)
    return { type: token?.type === "number" ? "number" : "spacing", text: String(px), px };
  const color = tokenColor(token);
  return color === undefined ? undefined : { type: "color", text: cssColor(color), color };
}

const cssColor = (c: RGBA): string =>
  c.a >= 1
    ? hexOf(c)
    : `rgba(${[c.r, c.g, c.b].map((v) => Math.round(v * 255)).join(", ")}, ${c.a})`;

const defaultKey = (group: string) => `weft.default-context/${group}`;

const has = (sets: readonly PTokenSet[], set: PTokenSet) => sets.some((s) => s.id === set.id);

/**
 * A theme per context in the group named after the modifier, the default context's first, and
 * the default theme turned on. A context's set holds the tokens of `base` whose value differs; a
 * token it already has follows the context, so an override that is no longer one stops differing.
 */
export function writeThemes(
  catalog: PTokenCatalog,
  base: PTokenSet,
  tokens: ReadonlyMap<string, Token>,
  modifier: TokenModifier,
  data: PluginData,
): void {
  const contexts = [...modifier.contexts].slice(0, MAX_CONTEXTS);
  contexts.sort(([a], [b]) => Number(b === modifier.default) - Number(a === modifier.default));
  let first: PTokenTheme | undefined;
  for (const [name, context] of contexts) {
    const theme =
      catalog.themes.find((t) => t.group === modifier.name && t.name === name) ??
      catalog.addTheme({ group: modifier.name, name });
    first ??= theme;
    if (!has(theme.activeSets, base)) theme.addSet(base.id);
    if (name === modifier.default) continue;
    const setName = `${MODES_GROUP}/${modifier.name}/${name}`;
    const set = catalog.sets.find((s) => s.name === setName) ?? catalog.addSet({ name: setName });
    const byName = new Map(set.tokens.map((t) => [t.name, t]));
    for (const [path, token] of tokens) {
      const fallback = tokenValue(token);
      if (fallback === undefined) continue;
      const own = tokenValue(context.get(path));
      const value = own !== undefined && own.type === fallback.type ? own : fallback;
      const made = byName.get(path);
      if (made !== undefined) {
        if (made.type === value.type && made.value !== value.text) made.value = value.text;
      } else if (value.text !== fallback.text) {
        set.addToken({ type: value.type, name: path, value: value.text });
      }
    }
    if (!has(theme.activeSets, set)) theme.addSet(set.id);
  }
  if (first === undefined) return;
  if (!first.active) first.toggleActive();
  data.setPluginData(defaultKey(modifier.name), first.name);
}

const RGBA_TEXT =
  /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*(?:,\s*(\d*\.?\d+)\s*)?\)$/i;
const PX_TEXT = /^(-?\d*\.?\d+)(px)?$/;

/** A Penpot token's text as the value a design tool keeps, or nothing for an alias or a formula. */
function modeValue(type: string, text: unknown): ModeValue | undefined {
  if (typeof text !== "string") return undefined;
  const t = text.trim();
  if (type === "color") {
    const hex = tokenColor({ type: "color", value: t });
    if (hex !== undefined) return hex;
    const m = RGBA_TEXT.exec(t);
    if (m === null) return undefined;
    const [r, g, b] = [m[1], m[2], m[3]].map((n) => Number(n) / 255) as [number, number, number];
    return { r, g, b, a: m[4] === undefined ? 1 : Number(m[4]) };
  }
  const m = PX_TEXT.exec(t);
  if (m === null || (type === "number" && m[2] !== undefined)) return undefined;
  return type === "number" || type === "spacing" || type === "dimension" ? Number(m[1]) : undefined;
}

/**
 * The library's theme group as a resolver document: the first group with a theme that turns on the
 * base set. Each theme is `tokens` with the values its sets give, later sets winning.
 */
export function readThemes(
  catalog: PTokenCatalog,
  tokens: ReadonlyMap<string, Token>,
  data: PluginData,
): Record<string, unknown> | undefined {
  const base = catalog.sets.find((s) => s.name === TOKEN_COLLECTION);
  if (base === undefined) return undefined;
  const group = catalog.themes.find((t) => has(t.activeSets, base))?.group;
  if (group === undefined) return undefined;
  const contexts = new Map<string, Map<string, Token>>();
  for (const theme of catalog.themes.filter((t) => t.group === group)) {
    if (contexts.size >= MAX_CONTEXTS || contexts.has(theme.name)) continue;
    const context = new Map(tokens);
    for (const set of catalog.sets.filter((s) => has(theme.activeSets, s))) {
      for (const t of set.tokens) {
        const token = modeToken(tokens.get(t.name), modeValue(t.type, t.value));
        if (token !== undefined) context.set(t.name, token);
      }
    }
    contexts.set(theme.name, context);
  }
  const recorded = data.getPluginData(defaultKey(group));
  const first = contexts.has(recorded) ? recorded : (contexts.keys().next().value as string);
  return resolverDocument({ name: group, default: first, contexts });
}
