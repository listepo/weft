// CSS-flavoured values as DTCG 2025.10 values. Each parser returns `undefined` for what it does not
// read, so the caller decides whether that is a loss; none throws.

export type Color = {
  colorSpace: string;
  components: (number | "none")[];
  alpha?: number;
  hex?: string;
};
export type Dimension = { value: number; unit: "px" | "rem" };
export type Duration = { value: number; unit: "ms" | "s" };

/** Rounds away binary noise (0.1 + 0.2) without changing a value a person wrote. */
export const round = (value: number, digits = 6): number => {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
};

const NUMBER = String.raw`[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?`;
const HEX = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

const hexByte = (component: number): string =>
  Math.round(Math.min(1, Math.max(0, component)) * 255)
    .toString(16)
    .padStart(2, "0");

/** One function argument: a number, a percentage (as a fraction of `scale`), an angle, or `none`. */
function argument(text: string, percentScale: number): number | "none" | undefined {
  if (text.toLowerCase() === "none") return "none";
  const match = new RegExp(`^(${NUMBER})(%|deg)?$`, "i").exec(text);
  if (match === null) return undefined;
  const value = Number(match[1]);
  // Only a hue, which has no percentage, takes an angle unit.
  if (match[2] === "deg" && !Number.isNaN(percentScale)) return undefined;
  if (match[2] === "%")
    return Number.isNaN(percentScale) ? undefined : (value * percentScale) / 100;
  return value;
}

// Per color function: how many components, and what 100% means for each (a hue has no percentage).
// `percent` is the value of 100% in the component's own unit, the numbers CSS Color 4 gives.
const FUNCTIONS: Record<string, { space: string; percent: number[] }> = {
  rgb: { space: "srgb", percent: [1, 1, 1] },
  hsl: { space: "hsl", percent: [Number.NaN, 100, 100] },
  hwb: { space: "hwb", percent: [Number.NaN, 100, 100] },
  lab: { space: "lab", percent: [100, 125, 125] },
  lch: { space: "lch", percent: [100, 150, Number.NaN] },
  oklab: { space: "oklab", percent: [1, 0.4, 0.4] },
  oklch: { space: "oklch", percent: [1, 0.4, Number.NaN] },
};

/** The DTCG color for a CSS color: hex, `rgb()`, `hsl()`, `hwb()`, `lab()`, `lch()`, `oklab()`, `oklch()`, `transparent`. */
export function parseColor(input: string): Color | undefined {
  const text = input.trim();
  if (text.toLowerCase() === "transparent") {
    return { colorSpace: "srgb", components: [0, 0, 0], alpha: 0, hex: "#000000" };
  }
  if (HEX.test(text)) {
    let digits = text.slice(1).toLowerCase();
    if (digits.length <= 4) digits = [...digits].map((d) => d + d).join("");
    const bytes = digits.match(/../g)?.map((pair) => Number.parseInt(pair, 16)) ?? [];
    const [r = 0, g = 0, b = 0, a = 255] = bytes;
    const color: Color = {
      colorSpace: "srgb",
      components: [r, g, b].map((byte) => round(byte / 255, 4)),
      hex: `#${digits.slice(0, 6)}`,
    };
    if (a !== 255) color.alpha = round(a / 255, 4);
    return color;
  }
  const call = /^(rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(\s*([^()]*?)\s*\)$/i.exec(text);
  if (call === null) return undefined;
  const name = (call[1] as string).toLowerCase().replace(/^(rgb|hsl)a$/, "$1");
  const shape = FUNCTIONS[name];
  if (shape === undefined) return undefined;
  const [channels = "", alphaText] = (call[2] as string).split("/").map((part) => part.trim());
  const words = channels.split(/[\s,]+/).filter((word) => word !== "");
  // The legacy comma syntax of rgba() and hsla() carries alpha as a fourth argument.
  const legacyAlpha = alphaText === undefined && words.length === 4 ? words.pop() : undefined;
  if (words.length !== 3) return undefined;
  const parsed = words.map((word, index) => argument(word, shape.percent[index] as number));
  if (parsed.includes(undefined)) return undefined;
  // `rgb()` takes 0 to 255 or a percentage; the DTCG components are 0 to 1.
  const components = (parsed as (number | "none")[]).map((component, index) =>
    name === "rgb" && typeof component === "number" && !words[index]?.endsWith("%")
      ? component / 255
      : component,
  );
  const color: Color = {
    colorSpace: shape.space,
    components: components.map((n) => (typeof n === "number" ? round(n, 4) : n)),
  };
  const alphaWord = alphaText ?? legacyAlpha;
  if (alphaWord !== undefined) {
    const alpha = argument(alphaWord, 1);
    if (typeof alpha !== "number") return undefined;
    if (alpha !== 1) color.alpha = round(Math.min(1, Math.max(0, alpha)), 4);
  }
  if (name === "rgb") {
    color.hex = `#${components.map((n) => hexByte(n === "none" ? 0 : n)).join("")}`;
  }
  return color;
}

/** `16px`, `1.5rem`, or a bare `0`; the only units of a DTCG 2025.10 dimension are `px` and `rem`. */
export function parseDimension(input: string | number): Dimension | undefined {
  if (input === 0 || input === "0") return { value: 0, unit: "px" };
  const match = new RegExp(`^(${NUMBER})(px|rem)$`, "i").exec(String(input).trim());
  if (match === null) return undefined;
  return { value: Number(match[1]), unit: (match[2] as string).toLowerCase() as "px" | "rem" };
}

/** A length in `em`: relative to a font size, which a DTCG dimension cannot express. */
export function parseEm(input: string | number): number | undefined {
  const match = new RegExp(`^(${NUMBER})em$`, "i").exec(String(input).trim());
  return match === null ? undefined : Number(match[1]);
}

export function parseDuration(input: string): Duration | undefined {
  const match = new RegExp(`^(${NUMBER})(ms|s)$`, "i").exec(input.trim());
  if (match === null) return undefined;
  return { value: Number(match[1]), unit: (match[2] as string).toLowerCase() as "ms" | "s" };
}

const KEYWORD_EASINGS: Record<string, [number, number, number, number]> = {
  linear: [0, 0, 1, 1],
  ease: [0.25, 0.1, 0.25, 1],
  "ease-in": [0.42, 0, 1, 1],
  "ease-out": [0, 0, 0.58, 1],
  "ease-in-out": [0.42, 0, 0.58, 1],
};

export function parseCubicBezier(input: string): [number, number, number, number] | undefined {
  const text = input.trim().toLowerCase();
  if (Object.hasOwn(KEYWORD_EASINGS, text)) return KEYWORD_EASINGS[text];
  const match = /^cubic-bezier\(([^()]*)\)$/.exec(text);
  if (match === null) return undefined;
  const points = (match[1] as string).split(",").map((part) => Number(part.trim()));
  const [x1, y1, x2, y2] = points;
  if (points.length !== 4 || points.some((p) => !Number.isFinite(p))) return undefined;
  if (!(x1! >= 0 && x1! <= 1 && x2! >= 0 && x2! <= 1)) return undefined;
  return [x1!, y1!, x2!, y2!];
}

/** A CSS font stack as DTCG's `fontFamily`: a name, or an ordered list of names. */
export function parseFontFamily(input: string): string | string[] | undefined {
  const names: string[] = [];
  let current = "";
  let quote = "";
  for (const char of input) {
    if (quote !== "") {
      if (char === quote) quote = "";
      else current += char;
    } else if (char === '"' || char === "'") quote = char;
    else if (char === ",") {
      names.push(current.trim());
      current = "";
    } else current += char;
  }
  names.push(current.trim());
  if (quote !== "" || names.some((name) => name === "" || /[(){};]/.test(name))) return undefined;
  return names.length === 1 ? (names[0] as string) : names;
}

/** Splits at the separators that are not inside parentheses or quotes, so `rgba(0, 0, 0, 0.1)` stays one piece. */
export function splitTop(text: string, separator: "," | " " | ";"): string[] {
  const parts: string[] = [];
  let depth = 0;
  let quote = "";
  let current = "";
  for (const char of text) {
    if (quote !== "") {
      if (char === quote) quote = "";
    } else if (char === '"' || char === "'") quote = char;
    else if (char === "(") depth += 1;
    else if (char === ")") depth = Math.max(0, depth - 1);
    else if (depth === 0 && (separator === " " ? /\s/.test(char) : char === separator)) {
      parts.push(current.trim());
      current = "";
      continue;
    }
    current += char;
  }
  parts.push(current.trim());
  return parts.filter((part) => part !== "");
}

export type Shadow = {
  color: unknown;
  offsetX: Dimension;
  offsetY: Dimension;
  blur: Dimension;
  spread: Dimension;
  inset?: true;
};

/**
 * A CSS `box-shadow` list as DTCG shadows. `color` turns the color word into a DTCG color or an
 * alias, because a stylesheet writes it as `var(--border)` as often as a literal.
 */
export function parseShadow(
  input: string,
  color: (text: string) => unknown,
): Shadow | Shadow[] | undefined {
  const layers: Shadow[] = [];
  for (const layer of splitTop(input, ",")) {
    let inset = false;
    const lengths: Dimension[] = [];
    let paint: unknown;
    for (const word of splitTop(layer, " ")) {
      if (word.toLowerCase() === "inset") inset = true;
      else {
        const length = parseDimension(word);
        if (length !== undefined) lengths.push(length);
        else if (paint === undefined) paint = color(word);
        else return undefined;
      }
    }
    const [offsetX, offsetY, blur = { value: 0, unit: "px" }, spread = { value: 0, unit: "px" }] =
      lengths;
    if (paint === undefined || offsetX === undefined || offsetY === undefined || lengths.length > 4)
      return undefined;
    layers.push({
      color: paint,
      offsetX,
      offsetY,
      blur: blur as Dimension,
      spread: spread as Dimension,
      ...(inset ? { inset: true as const } : {}),
    });
  }
  if (layers.length === 0) return undefined;
  return layers.length === 1 ? (layers[0] as Shadow) : layers;
}
