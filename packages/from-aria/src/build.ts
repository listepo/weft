// The helpers every importer shares: ids, literals, the loss log, required-prop stand-ins and
// limits. The importers themselves run in Rust (crates/weft-import, crates/weft-web, through
// ./engine.ts); these stay TypeScript because importers that run without WebAssembly reuse them
// (`@weft/figma` reads layers on Figma's main thread). crates/weft-import has the same helpers,
// and test/helpers.test.ts keeps the two in step.
import {
  diagnostic,
  EMBEDDED_REFERENCE,
  NON_XML_CHAR,
  WEFT_VERSION,
  type ComponentDef,
  type Diagnostic,
  type Value,
} from "@weft/core";
import type { ImportResult, Loss, LossKind } from "./types.ts";

// Below the 256 levels a document may nest (SPEC §2), leaving room for wrappers an importer adds.
export const MAX_DEPTH = 200;
export const MAX_NODES = 20_000;

const NON_XML = new RegExp(NON_XML_CHAR.source, "gu");
const REFERENCE_START = /\{(?=!?\$|token\.)/g;

export const squash = (s: string): string => s.replace(/\s+/g, " ").trim();
export const clean = (s: string): string => s.replace(NON_XML, "");

// A readable id fragment: lower-case ASCII words joined by hyphens.
export function slug(text: string): string {
  return text
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, 32)
    .replace(/-+$/, "");
}

// What the helpers below need, so that other importers (`@weft/figma`) can reuse them.
export type IdState = { used: Set<string>; counters: Map<string, number> };
export type LossLog = { losses: Loss[] };

const lose = (ctx: LossLog, kind: LossKind, path: string, note: string) =>
  ctx.losses.push({ kind, path, note });

export function limitReached(diagnostics: Diagnostic[], path: string, what: string): void {
  diagnostics.push(
    diagnostic("W602", {
      path,
      message: `The input ${what}; the rest was not imported.`,
      expected: `at most ${MAX_NODES} elements, nested at most ${MAX_DEPTH} levels deep`,
    }),
  );
}

// A literal must not contain a reference after its first character (SPEC §2.1, W213); such text
// is real content of the UI, so the brace is replaced by a look-alike instead of dropping it.
export function literal(ctx: LossLog, path: string, s: string): string {
  const out = clean(s);
  if (out.search(EMBEDDED_REFERENCE) <= 0) return out;
  lose(ctx, "text", path, "text that reads as a binding or token reference had its brace replaced");
  return out[0] + out.slice(1).replace(REFERENCE_START, "｛");
}

export function freshId(ctx: IdState, base: string, name: string): string {
  const s = slug(name);
  if (s !== "" && !ctx.used.has(`${base}-${s}`)) {
    ctx.used.add(`${base}-${s}`);
    return `${base}-${s}`;
  }
  // Counters remember where the last search stopped, so many equal names stay linear.
  const stem = s === "" ? base : `${base}-${s}`;
  let n = ctx.counters.get(stem) ?? (s === "" ? 1 : 2);
  while (ctx.used.has(`${stem}-${n}`)) n++;
  ctx.counters.set(stem, n + 1);
  ctx.used.add(`${stem}-${n}`);
  return `${stem}-${n}`;
}

/**
 * Stand-ins for required props the input has no value for. `name` is the element's text or name,
 * used for string props; `parentDef` is the parent's component (absent at the root).
 */
export function fillRequired(
  ctx: LossLog,
  props: Record<string, Value>,
  def: ComponentDef,
  parentDef: ComponentDef | undefined,
  root: boolean,
  name: string,
  path: string,
): void {
  for (const [prop, pd] of Object.entries(def.props ?? {})) {
    if (pd.required !== true || Object.hasOwn(props, prop)) continue;
    // The root's `weft` is `Document.weft`, never a prop (SPEC §3).
    if (prop === "weft" && root) continue;
    // A value the parent selects by (radio and option `value`) must tell the children apart.
    const selects = parentDef?.props?.[prop]?.writable === true;
    let value: Value;
    if (pd.type === "number") value = pd.default ?? pd.min ?? 0;
    else if (pd.type === "boolean") value = pd.default ?? false;
    else if (pd.type === "enum") value = pd.default ?? pd.values?.[0] ?? "";
    else value = selects ? slug(name) || prop : "";
    props[prop] = value;
    lose(
      ctx,
      "values",
      path,
      `required ${prop} is not in the input; ${JSON.stringify(value)} stands in`,
    );
  }
}

// The result when nothing could be read: the smallest document SPEC §2 allows.
export function emptyResult(diagnostics: Diagnostic[]): ImportResult {
  return {
    document: { weft: WEFT_VERSION, root: { kind: "screen", id: "screen" } },
    losses: [{ kind: "structure", path: "/screen#screen", note: "nothing could be imported" }],
    diagnostics,
  };
}
