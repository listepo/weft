// Resolution of SPEC §2.1 values against host data, loop variables and design tokens.
// Documents are untrusted: nothing here evaluates strings, and every lookup tolerates bad input.
import type { Token } from "@weft/catalog";

export type LoopVar = { value: unknown; path: string };

export type Scope = {
  data: unknown;
  vars: ReadonlyMap<string, LoopVar>;
  // Appended to template ids of every node rendered inside `each`, e.g. "[2][0]".
  suffix: string;
  // Absolute path of the innermost loop item, reported to actions.
  item?: string;
};

// Where a value came from: `path` is set only for a plain binding, the one form a host may write.
export type Resolved = { value: unknown; path?: string };

const PATH = /^\$([A-Za-z_][A-Za-z0-9_]*)?((?:\.(?:[A-Za-z_][A-Za-z0-9_]*|\d+))*)$/;

export const isRecord = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

// Own properties only, so a path like `$.__proto__` or `$.constructor` never reaches built-ins.
function step(value: unknown, key: string): unknown {
  if (Array.isArray(value)) return /^\d+$/.test(key) ? value[Number(key)] : undefined;
  return isRecord(value) && Object.hasOwn(value, key) ? value[key] : undefined;
}

export function resolvePath(
  path: string,
  scope: Scope,
): { value: unknown; path: string } | undefined {
  const m = PATH.exec(path);
  if (!m) return undefined;
  const [, name, rest = ""] = m;
  let value: unknown;
  let base: string;
  if (name === undefined) {
    // The grammar requires at least one segment after the root `$`.
    if (rest === "") return undefined;
    value = scope.data;
    base = "$";
  } else {
    const v = scope.vars.get(name);
    if (!v) return undefined;
    value = v.value;
    base = v.path;
  }
  const segments = rest === "" ? [] : rest.slice(1).split(".");
  for (const s of segments) value = step(value, s);
  return { value, path: base + rest };
}

export function resolveValue(raw: unknown, scope: Scope): Resolved {
  if (!isRecord(raw)) return { value: raw };
  if (typeof raw["bind"] === "string") {
    const hit = resolvePath(raw["bind"], scope);
    if (raw["not"] === true) return { value: !truthy(hit?.value) };
    return hit ? { value: hit.value, path: hit.path } : { value: undefined };
  }
  // Token references are resolved to CSS by `tokenCss`, never to data.
  return { value: undefined };
}

// The literal strings "true"/"false" count as booleans so a lenient reader of an
// untyped attribute (SPEC §3: unknown literals stay strings) still means what was written.
export function truthy(v: unknown): boolean {
  if (v === "false") return false;
  return Boolean(v);
}

export function toText(v: unknown): string {
  if (typeof v === "string") return v;
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "";
  if (typeof v === "boolean") return String(v);
  return "";
}

const SCHEME = /^([A-Za-z][A-Za-z0-9+.-]*):/;
const SAFE_SCHEMES = new Set(["http", "https", "mailto"]);

// Returns the URL when it is http(s), mailto or relative; anything else (javascript:, data:, …)
// is dropped. Tabs and newlines are removed first because the URL parser ignores them, which
// would otherwise let "java\nscript:" through.
export function safeUrl(value: string): string | undefined {
  // oxlint-disable-next-line no-control-regex -- matching control characters is the point
  const url = value.replace(/[\t\n\r]/g, "").replace(/^[\u0000- ]+|[\u0000- ]+$/g, "");
  if (url === "") return undefined;
  const colon = url.indexOf(":");
  const delimiter = url.search(/[/?#]/);
  const hasScheme = colon >= 0 && (delimiter < 0 || colon < delimiter);
  if (!hasScheme) return url;
  const scheme = SCHEME.exec(url)?.[1]?.toLowerCase();
  return scheme !== undefined && SAFE_SCHEMES.has(scheme) ? url : undefined;
}

const TOKEN_PATH = /^[A-Za-z0-9_$-]+(\.[A-Za-z0-9_$-]+)*$/;
const DIMENSION_UNITS = new Set(["px", "rem"]);
const HEX = /^#[0-9a-fA-F]{3,8}$/;

function tokenValueCss(token: Token): string | undefined {
  const v = token.value;
  if (token.type === "dimension" && isRecord(v)) {
    const n = v["value"];
    const unit = v["unit"];
    if (typeof n === "number" && Number.isFinite(n) && typeof unit === "string") {
      return DIMENSION_UNITS.has(unit) ? `${n}${unit}` : undefined;
    }
  }
  if (token.type === "color" && isRecord(v) && typeof v["hex"] === "string" && HEX.test(v["hex"])) {
    return v["hex"];
  }
  if (token.type === "number" && typeof v === "number" && Number.isFinite(v)) return String(v);
  return undefined;
}

// A token resolves to its CSS value when the host supplied it, otherwise to a CSS custom
// property so the host stylesheet can still provide it. Only the shapes above are emitted, so a
// token value can never smuggle `url(…)` or other CSS into the page.
export function tokenCss(
  raw: unknown,
  tokens: ReadonlyMap<string, Token> | undefined,
): string | undefined {
  if (!isRecord(raw) || typeof raw["token"] !== "string") return undefined;
  const path = raw["token"];
  if (!TOKEN_PATH.test(path)) return undefined;
  const token = tokens?.get(path);
  const css = token ? tokenValueCss(token) : undefined;
  return css ?? `var(--weft-${path.replaceAll(".", "-").replaceAll("$", "_")})`;
}

const MATERIAL_PROPS = ["tint", "solid", "blur"] as const;
const HEX6 = /^#[0-9a-fA-F]{6}$/;

/** A material token's three CSS values, as `tokens_css` writes them; undefined if it has no form. */
function materialValues(token: Token): Record<(typeof MATERIAL_PROPS)[number], string> | undefined {
  const v = token.value;
  if (token.type !== "material" || !isRecord(v) || !isRecord(v["tint"]) || !isRecord(v["blur"]))
    return undefined;
  const hex = v["tint"]["hex"];
  const alpha = v["tint"]["alpha"] ?? 1;
  const { value, unit } = v["blur"];
  if (
    typeof hex !== "string" ||
    !HEX6.test(hex) ||
    typeof alpha !== "number" ||
    !(alpha >= 0 && alpha <= 1) ||
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    (unit !== "px" && unit !== "rem")
  )
    return undefined;
  const byte = Math.round(alpha * 255)
    .toString(16)
    .padStart(2, "0");
  return { tint: `${hex}${byte}`, solid: hex, blur: `${unit === "rem" ? value * 16 : value}px` };
}

/**
 * The custom properties of an element that takes a material (`data-weft-material`): each the
 * token's value when the host supplied it, otherwise a reference to the custom property
 * `tokens_css` writes, as `tokenCss` does for the other token types.
 */
export function materialStyle(
  raw: unknown,
  tokens: ReadonlyMap<string, Token> | undefined,
): Record<string, string> | undefined {
  if (!isRecord(raw) || typeof raw["token"] !== "string") return undefined;
  const path = raw["token"];
  if (!TOKEN_PATH.test(path)) return undefined;
  const token = tokens?.get(path);
  const known = token === undefined ? undefined : materialValues(token);
  const name = `--weft-${path.replaceAll(".", "-").replaceAll("$", "_")}`;
  return Object.fromEntries(
    MATERIAL_PROPS.map((p) => [`--_weft-material-${p}`, known?.[p] ?? `var(${name}-${p})`]),
  );
}
