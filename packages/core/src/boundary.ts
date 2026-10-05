// The JSON boundary in front of the two engines (`./wasm.ts`, `./web.ts`): what crosses
// is JSON text, and these helpers write it. No module is loaded here.
import type { Mode } from "./diagnostics.ts";
import type { Catalog } from "./model.ts";

/**
 * Stands in for a value JSON cannot carry (NaN, ±Infinity, BigInt, a function, a symbol). The
 * TypeScript core rejected such values; `null` would turn some into valid input (a patch `value`
 * of `null` removes a prop), while an object with an empty-object member fails every schema
 * position a document, patch or value has.
 */
const UNSUPPORTED = { $unsupported: {} };

function replacer(_key: string, value: unknown): unknown {
  switch (typeof value) {
    case "number":
      return Number.isFinite(value) ? value : UNSUPPORTED;
    case "bigint":
    case "function":
    case "symbol":
      return UNSUPPORTED;
    default:
      return value;
  }
}

/**
 * `JSON.stringify` writes a lone surrogate as an escape, which Rust strings cannot hold. U+FFFF
 * is also one UTF-16 unit and also outside XML, so columns and the W113/W221 checks stay put.
 * The lookbehind skips an escaped backslash followed by the letters `ud…`.
 */
const LONE_SURROGATE = /(?<!\\)((?:\\\\)*)\\ud[89a-f][0-9a-f]{2}/g;

/**
 * Untrusted JavaScript value → JSON text for the Rust side. `undefined` when `JSON.stringify`
 * gives up (a cycle, or nesting past the JavaScript stack); Rust reads that as too deep, which is
 * what the TypeScript core's depth check said about such input.
 */
export function toJson(value: unknown): string | undefined {
  let text: string | undefined;
  try {
    text = JSON.stringify(value, replacer);
  } catch {
    return undefined;
  }
  // `undefined` at the top has no JSON form; `null` is rejected wherever a value is required.
  if (text === undefined) return "null";
  return text.includes("\\ud") ? text.replace(LONE_SURROGATE, "$1\\uffff") : text;
}

/** A string argument for Rust: lone surrogates become U+FFFF, as in `toJson`. */
export function wellFormed(text: string): string {
  return text.isWellFormed() ? text : text.replace(/\p{Cs}/gu, "￿");
}

export type CoreOptions = {
  mode?: Mode | undefined;
  tokens?: ReadonlyMap<string, string> | undefined;
  actions?: readonly string[] | undefined;
  /** `parse` only: the markup may stop anywhere (SPEC §6.3). */
  partial?: boolean | undefined;
};

export function options(o: CoreOptions): string {
  return (
    toJson({
      strict: o.mode === "strict",
      tokens: o.tokens === undefined ? undefined : [...o.tokens],
      actions: o.actions,
      partial: o.partial === true ? true : undefined,
    }) ?? "{}"
  );
}

/**
 * `catalogHandle` for one module: the parsed catalog for a catalog object, one per object. The
 * JSON text is compared on every call, so a catalog changed in place is parsed again instead of
 * answering from a stale copy.
 */
export function catalogHandles<H extends { free?(): void }>(
  make: (json: string) => H,
): (catalog: Catalog) => H {
  const catalogs = new WeakMap<object, { text: string; handle: H }>();
  return (catalog) => {
    const text = toJson(catalog) ?? "null";
    const cached = catalogs.get(catalog);
    if (cached?.text === text) return cached.handle;
    // Only WebAssembly handles own memory to release; the addon's are garbage collected.
    cached?.handle.free?.();
    const handle = make(text);
    catalogs.set(catalog, { text, handle });
    return handle;
  };
}
