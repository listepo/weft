// Design tokens as design-tool values, and back: a dimension becomes pixels, a color an RGBA.
// Neither tool's numbers carry units, so `rem` is converted at a fixed base.
import type { Token } from "@weft/catalog";

/** A color with channels from 0 to 1, as Figma takes it. */
export type RGB = { readonly r: number; readonly g: number; readonly b: number };
export type RGBA = RGB & { readonly a: number };

/** CSS pixels per `rem`: the browser default, which Weft's renderer leaves unchanged. */
export const REM_PX = 16;

const HEX = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i;

const isRecord = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

export function tokenPx(token: Token | undefined): number | undefined {
  if (token === undefined) return undefined;
  const v = token.value;
  if (token.type === "number" && typeof v === "number" && Number.isFinite(v)) return v;
  if (token.type !== "dimension" || !isRecord(v)) return undefined;
  const value = v["value"];
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  if (v["unit"] === "px") return value;
  if (v["unit"] === "rem") return value * REM_PX;
  return undefined;
}

export function tokenColor(token: Token | undefined): RGBA | undefined {
  if (token?.type !== "color") return undefined;
  // The format's colour is an object, but hand-written token files often keep the older hex
  // string, which the loader accepts; skipping it would leave such a colour out of the library.
  if (typeof token.value === "string") return fromHex(HEX.exec(token.value));
  if (!isRecord(token.value)) return undefined;
  const v = token.value;
  const alpha = typeof v["alpha"] === "number" ? v["alpha"] : 1;
  const c = v["components"];
  if (
    v["colorSpace"] === "srgb" &&
    Array.isArray(c) &&
    c.length === 3 &&
    c.every((n) => typeof n === "number" && n >= 0 && n <= 1)
  ) {
    const [r, g, b] = c as [number, number, number];
    return { r, g, b, a: alpha };
  }
  const hex = fromHex(typeof v["hex"] === "string" ? HEX.exec(v["hex"]) : null);
  return hex === undefined ? undefined : { ...hex, a: alpha };
}

function fromHex(hex: RegExpExecArray | null): RGBA | undefined {
  if (hex === null) return undefined;
  const n = Number.parseInt(hex[1] ?? "", 16);
  const a = hex[2] === undefined ? 1 : Number.parseInt(hex[2], 16) / 255;
  return { r: ((n >> 16) & 255) / 255, g: ((n >> 8) & 255) / 255, b: (n & 255) / 255, a };
}

const channel = (c: number) => Math.round(c * 255);

/** `#rrggbb`, the 8-bit colour both tools show for channels from 0 to 1. */
export const hexOf = (c: RGB): string =>
  `#${[c.r, c.g, c.b].map((v) => channel(v).toString(16).padStart(2, "0")).join("")}`;

/** The group of a token path: `space` for `space.md`. */
export const tokenGroup = (path: string): string =>
  path.slice(0, Math.max(0, path.lastIndexOf(".")));

/**
 * The token a raw pixel value stands for, if one has exactly that value. Ties prefer the group
 * of the token the element had, then the groups `preferred` lists (in order), then path order,
 * so the same edit always maps to the same token.
 */
export function matchToken(
  px: number,
  tokens: ReadonlyMap<string, Token>,
  tokenType: string | undefined,
  previous: string | undefined,
  preferred: readonly string[],
): string | undefined {
  const candidates = [...tokens]
    .filter(([, t]) => (tokenType === undefined || t.type === tokenType) && tokenPx(t) === px)
    .map(([path]) => path)
    .sort();
  const rank = (path: string): number => {
    if (previous !== undefined && tokenGroup(path) === tokenGroup(previous)) return -1;
    const at = preferred.indexOf(tokenGroup(path));
    return at === -1 ? preferred.length : at;
  };
  return candidates.sort((a, b) => rank(a) - rank(b))[0];
}
