import { isTokenName, toTree, type Token } from "./tree.ts";
import { MAX_SOURCE_LENGTH, type Loss, type Mapped } from "./types.ts";
import {
  parseColor,
  parseCubicBezier,
  parseDimension,
  parseDuration,
  parseEm,
  parseFontFamily,
  parseShadow,
  splitTop,
} from "./values.ts";

// Open Design keeps a design system's tokens in `tokens.css`: one `:root` block of custom properties
// (https://github.com/nexu-io/open-design, docs/design-systems.md). A stylesheet carries no token
// types, so a value's own shape decides the DTCG type and its name decides the group.

type Classified = { group: string; type: string; value: unknown };
type Failure = { kind: Loss["kind"]; why: string };
type Declarations = Map<string, string>;

const NUMBER = /^[+-]?(?:\d+\.?\d*|\.\d+)$/;
const COMPUTED = /^(?:color-mix|light-dark|calc|clamp|min|max|env)\(/i;

const show = (value: string): string => (value.length > 60 ? `${value.slice(0, 57)}...` : value);

/** The declarations of every `:root` block, and a loss for each other block, whose custom properties have no home in one token file. */
function readBlocks(css: string, losses: Loss[]): Declarations {
  const text = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const declarations: Declarations = new Map();
  let depth = 0;
  let start = 0;
  let selector = "";
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (char === "{") {
      if (depth === 0) {
        selector = text.slice(start, i).trim();
        start = i + 1;
      }
      depth += 1;
    } else if (char === "}" && depth > 0) {
      depth -= 1;
      if (depth > 0) continue;
      const body = text.slice(start, i);
      start = i + 1;
      if (/^(?::root|html)$/i.test(selector)) {
        for (const declaration of splitTop(body, ";")) {
          const colon = declaration.indexOf(":");
          const name = declaration.slice(0, colon).trim();
          if (colon > 0 && name.startsWith("--"))
            declarations.set(name.slice(2), declaration.slice(colon + 1).trim());
        }
      } else {
        const count = (body.match(/(?:^|[;{\s])--[\w-]+\s*:/g) ?? []).length;
        losses.push({
          kind: "theme",
          path: selector,
          note: `${count} custom properties under "${show(selector)}" are not mapped: a token file holds one theme, the :root one. Map the others with their own files.`,
        });
      }
    }
  }
  return declarations;
}

const VAR = /^var\(\s*--([^\s,()]+)\s*(?:,[^()]*)?\)$/i;
const varTarget = (value: string): string | undefined => VAR.exec(value.trim())?.[1];

/** The group a bare length belongs to, from what its name says it measures. */
function lengthGroup(name: string): string {
  if (/radius|rounded/.test(name)) return "rounded";
  if (/^(?:space|spacing|gap|gutter|pad|margin)|section|gutter/.test(name)) return "spacing";
  if (/^(?:text|font-size|fs)(?:-|$)/.test(name)) return "fontSizes";
  return "sizes";
}

export function fromTokensCss(css: string): Mapped {
  const losses: Loss[] = [];
  const problems: string[] = [];
  if (css.length > MAX_SOURCE_LENGTH) {
    problems.push(`The file is longer than ${MAX_SOURCE_LENGTH} characters and was not read.`);
    return { tokens: {}, losses, problems };
  }
  const declarations = readBlocks(css, losses);
  if (declarations.size === 0) problems.push("No custom properties found in a :root block.");

  const classified = new Map<string, Classified | Failure>();
  /** A literal's group, type and value, or the reason it has none. Follows `var()` to the literal at the end of the chain. */
  const literal = (name: string, seen: Set<string> = new Set()): Classified | Failure => {
    const known = classified.get(name);
    if (known !== undefined) return known;
    const raw = declarations.get(name);
    if (raw === undefined)
      return {
        kind: "unresolved-alias",
        why: `refers to --${name}, which is not declared in :root`,
      };
    const target = varTarget(raw);
    let outcome: Classified | Failure;
    if (target !== undefined) {
      outcome = seen.has(target)
        ? { kind: "unresolved-alias", why: "loops back to itself" }
        : declarations.has(target)
          ? literal(target, new Set(seen).add(name))
          : {
              kind: "unresolved-alias",
              why: `refers to --${target}, which is not declared in :root`,
            };
    } else outcome = classifyLiteral(name, raw);
    classified.set(name, outcome);
    return outcome;
  };

  const colorWord = (word: string): unknown => {
    const target = varTarget(word);
    if (target === undefined) return parseColor(word);
    const found = literal(target);
    return "group" in found && found.group === "colors" ? `{colors.${target}}` : undefined;
  };

  function classifyLiteral(name: string, raw: string): Classified | Failure {
    const fail = (why: string): Failure => ({ kind: "unsupported-value", why });
    const text = raw.trim();
    if (COMPUTED.test(text))
      return fail("is computed in CSS (color-mix, calc and the like) and has no fixed DTCG value");
    const color = parseColor(text);
    if (color !== undefined) return { group: "colors", type: "color", value: color };
    if (/ease|easing|timing/.test(name) || /^cubic-bezier\(/i.test(text)) {
      const bezier = parseCubicBezier(text);
      if (bezier !== undefined) return { group: "easing", type: "cubicBezier", value: bezier };
    }
    const duration = parseDuration(text);
    if (duration !== undefined) return { group: "duration", type: "duration", value: duration };
    if (/elev|shadow|ring/.test(name)) {
      if (text.toLowerCase() === "none") return fail("is `none`, and DTCG has no empty shadow");
      const shadow = parseShadow(text, colorWord);
      return shadow === undefined
        ? fail("is not a box-shadow this mapping reads")
        : { group: "shadows", type: "shadow", value: shadow };
    }
    if (
      /^font(?:-family|-display|-body|-mono|-sans|-serif|-heading)?(?:-|$)/.test(name) &&
      !/size|weight/.test(name)
    ) {
      const family = parseFontFamily(text);
      if (family !== undefined) return { group: "fontFamilies", type: "fontFamily", value: family };
    }
    const length = parseDimension(text);
    if (length !== undefined) return { group: lengthGroup(name), type: "dimension", value: length };
    if (parseEm(text) !== undefined)
      return fail("is in em, which is relative to a font size and has no DTCG dimension");
    if (NUMBER.test(text)) {
      if (/weight/.test(name))
        return { group: "fontWeights", type: "fontWeight", value: Number(text) };
      return {
        group: /leading|line-height/.test(name) ? "lineHeights" : "numbers",
        type: "number",
        value: Number(text),
      };
    }
    return fail("is not a value this mapping reads");
  }

  const tokens: Token[] = [];
  for (const [name, raw] of declarations) {
    if (!isTokenName(name)) {
      losses.push({
        kind: "invalid-name",
        path: `--${name}`,
        note: `"${name}" cannot name a DTCG token; the token is left out.`,
      });
      continue;
    }
    const found = literal(name);
    if ("why" in found) {
      losses.push({
        kind: found.kind,
        path: `--${name}`,
        note: `${show(raw)} ${found.why}; the token is left out.`,
      });
      continue;
    }
    const target = varTarget(raw);
    const value = target === undefined ? found.value : `{${found.group}.${target}}`;
    tokens.push({ path: [found.group, name], type: found.type, value, source: `--${name}` });
  }
  return { tokens: toTree(tokens, losses), losses, problems };
}
