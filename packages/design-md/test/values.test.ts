import assert from "node:assert/strict";
import { test } from "vitest";
import {
  parseColor,
  parseCubicBezier,
  parseDimension,
  parseDuration,
  parseFontFamily,
  parseShadow,
  splitTop,
} from "../src/values.ts";

test("hex colors, short and with alpha, become srgb components with a hex fallback", () => {
  assert.deepEqual(parseColor("#f00"), {
    colorSpace: "srgb",
    components: [1, 0, 0],
    hex: "#ff0000",
  });
  assert.deepEqual(parseColor("#00000080"), {
    colorSpace: "srgb",
    components: [0, 0, 0],
    hex: "#000000",
    alpha: 0.502,
  });
  assert.equal(parseColor("#12"), undefined);
  assert.equal(parseColor("#ggg"), undefined);
});

test("color functions keep their own space; percentages and angles follow CSS Color 4", () => {
  assert.deepEqual(parseColor("rgb(255 0 0 / 50%)"), {
    colorSpace: "srgb",
    components: [1, 0, 0],
    alpha: 0.5,
    hex: "#ff0000",
  });
  assert.deepEqual(parseColor("rgba(0, 227, 253, 0.1)"), {
    colorSpace: "srgb",
    components: [0, 0.8902, 0.9922],
    alpha: 0.1,
    hex: "#00e3fd",
  });
  assert.deepEqual(parseColor("rgb(100% 0% 0%)")?.components, [1, 0, 0]);
  assert.deepEqual(parseColor("oklch(70% 0.1 200deg)"), {
    colorSpace: "oklch",
    components: [0.7, 0.1, 200],
  });
  assert.deepEqual(parseColor("hsl(210 50% 40%)"), {
    colorSpace: "hsl",
    components: [210, 50, 40],
  });
  assert.deepEqual(parseColor("oklab(none 0 0)")?.components, ["none", 0, 0]);
  assert.equal(parseColor("oklch(70% 0.1 20%)"), undefined, "a hue takes no percentage");
  assert.equal(parseColor("rgb(1 2 3deg)"), undefined, "only a hue takes an angle");
  assert.equal(parseColor("rgb(1 2)"), undefined);
  assert.equal(parseColor("red"), undefined, "named colors are not read");
  assert.deepEqual(parseColor("transparent")?.alpha, 0);
});

test("dimensions are px or rem; a bare zero is zero px; everything else is refused", () => {
  assert.deepEqual(parseDimension("1.5rem"), { value: 1.5, unit: "rem" });
  assert.deepEqual(parseDimension("-4PX"), { value: -4, unit: "px" });
  assert.deepEqual(parseDimension(0), { value: 0, unit: "px" });
  for (const bad of ["12", "12pt", "1em", "50%", "", "px", "calc(1px + 2px)"])
    assert.equal(parseDimension(bad), undefined, bad);
});

test("durations, easings and font stacks", () => {
  assert.deepEqual(parseDuration("140ms"), { value: 140, unit: "ms" });
  assert.deepEqual(parseDuration("0.2s"), { value: 0.2, unit: "s" });
  assert.equal(parseDuration("2m"), undefined);
  assert.deepEqual(parseCubicBezier("cubic-bezier(0.2, 0, 0, 1)"), [0.2, 0, 0, 1]);
  assert.deepEqual(parseCubicBezier("ease-in-out"), [0.42, 0, 0.58, 1]);
  assert.equal(
    parseCubicBezier("cubic-bezier(2, 0, 0, 1)"),
    undefined,
    "x outside 0..1 is not a DTCG curve",
  );
  assert.equal(parseFontFamily("Inter"), "Inter");
  assert.deepEqual(parseFontFamily('"SF Mono", ui-monospace, Menlo'), [
    "SF Mono",
    "ui-monospace",
    "Menlo",
  ]);
  assert.equal(parseFontFamily("Inter,"), undefined);
  assert.equal(parseFontFamily('"Inter'), undefined);
});

test("shadows: lengths, an optional inset, and a color through the callback", () => {
  const color = (word: string) => (word === "red" ? "{colors.red}" : undefined);
  assert.deepEqual(parseShadow("inset 1px 2px red", color), {
    color: "{colors.red}",
    offsetX: { value: 1, unit: "px" },
    offsetY: { value: 2, unit: "px" },
    blur: { value: 0, unit: "px" },
    spread: { value: 0, unit: "px" },
    inset: true,
  });
  assert.equal(Array.isArray(parseShadow("0 0 1px red, 0 2px 4px red", color)), true);
  assert.equal(parseShadow("0 0 1px", color), undefined, "no color");
  assert.equal(parseShadow("1px red", color), undefined, "no second offset");
  assert.equal(parseShadow("1px 1px 1px 1px 1px red", color), undefined);
  assert.equal(parseShadow("1px 1px blue", color), undefined, "the callback refused the color");
});

test("splitTop keeps parentheses and quotes whole", () => {
  assert.deepEqual(splitTop('a, rgba(0, 0, 0, 1), "b, c"', ","), [
    "a",
    "rgba(0, 0, 0, 1)",
    '"b, c"',
  ]);
  assert.deepEqual(splitTop("0 0 0 1px rgb(0 0 0)", " "), ["0", "0", "0", "1px", "rgb(0 0 0)"]);
  assert.deepEqual(splitTop("--a: url(x;y); --b: 1", ";"), ["--a: url(x;y)", "--b: 1"]);
});
