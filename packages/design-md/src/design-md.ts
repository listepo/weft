import { parse } from "yaml";
import { aliasPath, isTokenName, toTree, type Token } from "./tree.ts";
import { MAX_SOURCE_LENGTH, type Loss, type Mapped } from "./types.ts";
import { parseColor, parseDimension, parseEm, parseFontFamily, round } from "./values.ts";

// DESIGN.md of Google Labs (https://github.com/google-labs-code/design.md, spec/ in that repository):
// YAML frontmatter with `colors`, `typography`, `rounded`, `spacing` and `components`, then Markdown prose.
// Open Design reads the same file but keeps its tokens in `tokens.css` (see tokens-css.ts).

const FRONTMATTER = /^﻿?---[ \t]*\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n([\s\S]*))?$/;
const SECTIONS = ["colors", "typography", "rounded", "spacing", "components"] as const;
// Metadata the format allows next to the sections; none of it is a token.
const METADATA = new Set(["name", "description", "version"]);

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const show = (value: unknown): string => {
  const text = typeof value === "string" ? value : JSON.stringify(value);
  return text.length > 60 ? `${text.slice(0, 57)}...` : text;
};

/** A section's entries, or `undefined` when the section is not a mapping (an empty `colors:` is `null`, which is just empty). */
function entries(section: string, value: unknown, losses: Loss[]): [string, unknown][] {
  if (value === null || value === undefined) return [];
  if (!isRecord(value)) {
    losses.push({
      kind: "unsupported-value",
      path: section,
      note: `${section} is not a mapping; the section is left out.`,
    });
    return [];
  }
  return Object.entries(value);
}

function checkName(path: string, name: string, losses: Loss[]): boolean {
  if (isTokenName(name)) return true;
  losses.push({
    kind: "invalid-name",
    path,
    note: `"${name}" cannot name a DTCG token (dots, braces and a leading $ are reserved); the token is left out.`,
  });
  return false;
}

type Typography = Record<string, unknown>;

/** The typography properties a `{ref}` carries through unchanged; DTCG allows an alias for any property. */
const keep = (value: unknown): value is string => aliasPath(value) !== undefined;

function typography(
  path: string,
  raw: Record<string, unknown>,
  losses: Loss[],
): Typography | undefined {
  const lose = (kind: Loss["kind"], note: string): void => void losses.push({ kind, path, note });
  const out: Typography = {};

  const family = raw["fontFamily"];
  if (keep(family)) out["fontFamily"] = family;
  else if (typeof family === "string" && parseFontFamily(family) !== undefined)
    out["fontFamily"] = parseFontFamily(family);
  const size = raw["fontSize"];
  const fontSize = keep(size)
    ? size
    : parseDimension(typeof size === "number" || typeof size === "string" ? size : "");
  if (fontSize !== undefined) out["fontSize"] = fontSize;
  if (out["fontFamily"] === undefined || out["fontSize"] === undefined) {
    lose(
      "unsupported-value",
      "A typography token needs a readable fontFamily and fontSize; the token is left out.",
    );
    return undefined;
  }

  const weight = raw["fontWeight"];
  const weightNumber = keep(weight) ? weight : Number(weight);
  if (
    keep(weight) ||
    (weight !== undefined &&
      weight !== null &&
      weight !== "" &&
      Number.isFinite(weightNumber) &&
      (weightNumber as number) >= 1 &&
      (weightNumber as number) <= 1000)
  ) {
    out["fontWeight"] = weightNumber;
  } else {
    out["fontWeight"] = 400;
    lose(
      "converted",
      `fontWeight ${weight === undefined ? "is missing" : `${show(weight)} is not a number from 1 to 1000`}; DTCG requires one, so 400 (normal) stands in.`,
    );
  }

  // Both of these are relative to the font size in CSS and in DESIGN.md, but a DTCG lineHeight is a
  // multiplier and its letterSpacing a dimension, so they are rewritten against fontSize.
  const sizeValue = isRecord(fontSize) ? (fontSize as { value: number; unit: string }) : undefined;
  const line = raw["lineHeight"];
  if (keep(line)) out["lineHeight"] = line;
  else if (typeof line === "number" && Number.isFinite(line)) out["lineHeight"] = line;
  else {
    const absolute = typeof line === "string" ? parseDimension(line) : undefined;
    const multiplier =
      typeof line === "string" && /^[+-]?(\d+\.?\d*|\.\d+)$/.test(line.trim())
        ? Number(line)
        : undefined;
    const em = typeof line === "string" ? parseEm(line) : undefined;
    if (multiplier !== undefined) out["lineHeight"] = multiplier;
    else if (em !== undefined) out["lineHeight"] = em;
    else if (
      absolute !== undefined &&
      sizeValue !== undefined &&
      absolute.unit === sizeValue.unit &&
      sizeValue.value !== 0
    ) {
      out["lineHeight"] = round(absolute.value / sizeValue.value);
      lose(
        "converted",
        `lineHeight ${show(line)} became the multiplier ${String(out["lineHeight"])} of fontSize, the form DTCG uses.`,
      );
    } else {
      out["lineHeight"] = 1.2;
      lose(
        "converted",
        `lineHeight ${line === undefined ? "is missing" : `${show(line)} cannot be a multiplier of fontSize`}; DTCG requires one, so 1.2 stands in.`,
      );
    }
  }

  const tracking = raw["letterSpacing"];
  if (keep(tracking)) out["letterSpacing"] = tracking;
  else {
    const em = typeof tracking === "string" ? parseEm(tracking) : undefined;
    const absolute =
      typeof tracking === "string" || tracking === 0 ? parseDimension(tracking) : undefined;
    if (absolute !== undefined) out["letterSpacing"] = absolute;
    else if (em !== undefined && sizeValue !== undefined) {
      out["letterSpacing"] = { value: round(em * sizeValue.value), unit: sizeValue.unit };
      lose(
        "converted",
        `letterSpacing ${show(tracking)} became ${String((out["letterSpacing"] as { value: number }).value)}${sizeValue.unit} at this fontSize; DTCG has no em dimension.`,
      );
    } else {
      out["letterSpacing"] = { value: 0, unit: "px" };
      if (tracking !== undefined)
        lose("converted", `letterSpacing ${show(tracking)} is not a length; 0px stands in.`);
    }
  }

  for (const key of Object.keys(raw)) {
    if (!["fontFamily", "fontSize", "fontWeight", "lineHeight", "letterSpacing"].includes(key)) {
      lose("unsupported-value", `${key} has no place in a DTCG typography token; it is left out.`);
    }
  }
  return out;
}

/** The tokens a DESIGN.md frontmatter declares as a DTCG 2025.10 tree, and what did not fit. */
export function fromDesignMd(text: string): Mapped {
  const losses: Loss[] = [];
  const problems: string[] = [];
  const result = (tokens: Token[], name?: string): Mapped => ({
    tokens: toTree(tokens, losses),
    losses,
    problems,
    ...(name === undefined ? {} : { name }),
  });
  if (text.length > MAX_SOURCE_LENGTH) {
    problems.push(`The file is longer than ${MAX_SOURCE_LENGTH} characters and was not read.`);
    return result([]);
  }
  const match = FRONTMATTER.exec(text);
  if (match === null) {
    problems.push(
      "No YAML frontmatter found: a DESIGN.md that keeps its tokens elsewhere (Open Design's keeps them in tokens.css) has nothing to map here.",
    );
    if (text.trim() !== "")
      losses.push({
        kind: "prose",
        path: "(body)",
        note: "The whole file is prose, which guides an agent and has no token form.",
      });
    return result([]);
  }
  let data: unknown;
  try {
    // An alias in a token file could only be an expansion bomb.
    data = parse(match[1] as string, { maxAliasCount: 0, strict: true, logLevel: "error" });
  } catch (error) {
    problems.push(`The frontmatter is not valid YAML: ${(error as Error).message.split("\n")[0]}`);
    return result([]);
  }
  if (!isRecord(data)) {
    problems.push("The frontmatter is not a mapping.");
    return result([]);
  }
  const body = (match[2] ?? "").trim();
  if (body !== "")
    losses.push({
      kind: "prose",
      path: "(body)",
      note: `The Markdown body (${body.length} characters) guides an agent and has no token form; it is left as it is.`,
    });

  const tokens: Token[] = [];
  const add = (section: string, name: string, type: string, value: unknown): void => {
    tokens.push({ path: [section, name], type, value, source: `${section}.${name}` });
  };
  const unreadable = (section: string, name: string, value: unknown, why: string): void => {
    const hint =
      value === null ? " (an unquoted # starts a YAML comment, so quote hex colors)" : "";
    losses.push({
      kind: "unsupported-value",
      path: `${section}.${name}`,
      note: `${show(value)} ${why}${hint}; the token is left out.`,
    });
  };

  for (const [name, value] of entries("colors", data["colors"], losses)) {
    if (!checkName(`colors.${name}`, name, losses)) continue;
    const color =
      typeof value === "string"
        ? aliasPath(value) === undefined
          ? parseColor(value)
          : value
        : undefined;
    if (color === undefined)
      unreadable(
        "colors",
        name,
        value,
        "is not a color this mapping reads (hex, rgb, hsl, hwb, lab, lch, oklab, oklch or a {ref})",
      );
    else add("colors", name, "color", color);
  }
  for (const section of ["rounded", "spacing"] as const) {
    for (const [name, value] of entries(section, data[section], losses)) {
      if (!checkName(`${section}.${name}`, name, losses)) continue;
      const dimension =
        typeof value === "string" && aliasPath(value) !== undefined
          ? value
          : parseDimension(typeof value === "number" || typeof value === "string" ? value : "");
      if (dimension === undefined)
        unreadable(section, name, value, "is not a length in px or rem (DTCG has no other units)");
      else add(section, name, "dimension", dimension);
    }
  }
  for (const [name, value] of entries("typography", data["typography"], losses)) {
    if (!checkName(`typography.${name}`, name, losses)) continue;
    if (!isRecord(value)) {
      unreadable("typography", name, value, "is not a mapping of typography properties");
      continue;
    }
    const mapped = typography(`typography.${name}`, value, losses);
    if (mapped !== undefined) add("typography", name, "typography", mapped);
  }

  for (const [name, value] of entries("components", data["components"], losses)) {
    const properties = isRecord(value) ? Object.keys(value).join(", ") : "";
    losses.push({
      kind: "component",
      path: `components.${name}`,
      note: `Weft keeps no component styles in its tokens${properties === "" ? "" : ` (${properties})`}; the references they hold stay readable in the colors, typography, rounded and spacing groups.`,
    });
  }
  for (const key of Object.keys(data)) {
    if (!METADATA.has(key) && !(SECTIONS as readonly string[]).includes(key)) {
      losses.push({
        kind: "unsupported-section",
        path: key,
        note: `${key} is not a section this mapping knows; it is left out.`,
      });
    }
  }
  const name = typeof data["name"] === "string" && data["name"] !== "" ? data["name"] : undefined;
  return result(tokens, name);
}
