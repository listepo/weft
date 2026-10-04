import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { loadTokens } from "@weft/catalog";
import fc from "fast-check";
import { test } from "vitest";
import { fromTokensCss } from "../src/index.ts";

const fixture = (path: string): string =>
  readFileSync(new URL(`./fixtures/${path}`, import.meta.url), "utf8");
const at = (tree: Record<string, unknown>, path: string): unknown =>
  path
    .split(".")
    .reduce<unknown>((node, key) => (node as Record<string, unknown> | undefined)?.[key], tree);

test("Open Design's minimal tokens.css maps whole, and loads in the Weft token loader", () => {
  const { tokens, losses, problems } = fromTokensCss(fixture("open-design/minimal/tokens.css"));
  assert.deepEqual(problems, []);
  assert.deepEqual(loadTokens(tokens).problems, []);
  assert.deepEqual(at(tokens, "spacing.space-4.$value"), { value: 16, unit: "px" });
  assert.deepEqual(at(tokens, "rounded.radius-pill.$value"), { value: 9999, unit: "px" });
  assert.deepEqual(at(tokens, "fontSizes.text-base.$value"), { value: 16, unit: "px" });
  assert.equal(at(tokens, "lineHeights.leading-body.$value"), 1.55);
  assert.deepEqual(at(tokens, "duration.motion-fast.$value"), { value: 140, unit: "ms" });
  assert.deepEqual(at(tokens, "easing.ease-standard.$value"), [0.2, 0, 0, 1]);
  assert.deepEqual(at(tokens, "fontFamilies.font-mono.$value"), [
    "SF Mono",
    "ui-monospace",
    "Menlo",
    "monospace",
  ]);
  assert.equal(at(tokens, "colors.accent.$value.hex"), "#111111");
  assert.equal(at(tokens, "shadows.elev-ring.$value.color"), "{colors.border}");
  // The four it cannot carry are exactly the computed colors, the em tracking and the `none` shadow.
  assert.deepEqual(losses.map((loss) => loss.path).sort(), [
    "--accent-active",
    "--accent-hover",
    "--elev-flat",
    "--tracking-display",
  ]);
});

test("var() chains become aliases in the target's group, and broken ones are dropped with a loss", () => {
  const { tokens, losses } = fromTokensCss(`:root {
    --brand: #123456; --link: var(--brand); --link-2: var(--link);
    --a: var(--b); --b: var(--a); --gone: var(--nowhere); --gap: 8px; --pad: var(--gap, 1px);
  }`);
  assert.equal(at(tokens, "colors.link.$value"), "{colors.brand}");
  assert.equal(at(tokens, "colors.link-2.$value"), "{colors.link}");
  assert.equal(at(tokens, "spacing.pad.$value"), "{spacing.gap}");
  assert.equal(at(tokens, "colors.a"), undefined);
  assert.deepEqual(losses.map((l) => `${l.kind} ${l.path}`).sort(), [
    "unresolved-alias --a",
    "unresolved-alias --b",
    "unresolved-alias --gone",
  ]);
  assert.deepEqual(loadTokens(tokens).problems, []);
});

test("blocks other than :root are one theme loss each, with the number of properties they held", () => {
  const { tokens, losses } = fromTokensCss(`
    :root { --bg: #fff; }
    [data-theme="dark"] { --bg: #000; --fg: #eee; }
    @media (prefers-color-scheme: dark) { :root { --bg: #111; } }
    body { color: red; }`);
  assert.equal(at(tokens, "colors.bg.$value.hex"), "#ffffff");
  const themes = losses.filter((l) => l.kind === "theme");
  assert.equal(themes.length, 3);
  assert.match(themes[0]?.note ?? "", /^2 custom properties/);
});

test("comments are ignored, and an empty or non-CSS file is a problem, not an exception", () => {
  assert.equal(
    at(
      fromTokensCss("/* :root { --x: #000 } */ :root { --y: #fff }").tokens,
      "colors.y.$value.hex",
    ),
    "#ffffff",
  );
  assert.equal(fromTokensCss("").problems.length, 1);
  assert.equal(fromTokensCss("not css {{{").problems.length, 1);
  assert.equal(fromTokensCss("x".repeat(1_000_001)).problems.length, 1);
});

test("a name that cannot be a DTCG token is a loss", () => {
  const { tokens, losses } = fromTokensCss(":root { --a.b: #000; --ok: #000; }");
  assert.deepEqual(Object.keys(tokens["colors"] as object), ["ok"]);
  assert.deepEqual(
    losses.map((l) => l.kind),
    ["invalid-name"],
  );
});

test("any input maps without throwing, and what comes out always loads", () => {
  const css = fc.stringMatching(/^[-:;{}(),.%#\w\s"'/*]{0,300}$/);
  const declarations = fc.array(fc.tuple(fc.stringMatching(/^[a-z-]{1,12}$/), css), {
    maxLength: 12,
  });
  fc.assert(
    fc.property(
      fc.oneof(
        css,
        declarations.map(
          (list) =>
            `:root{${list.map(([n, v]) => `--${n}:${v.replaceAll(/[{};]/g, "")}`).join(";")}}`,
        ),
      ),
      (source) => {
        const { tokens } = fromTokensCss(source);
        assert.deepEqual(loadTokens(tokens).problems, []);
      },
    ),
    { numRuns: 300 },
  );
});
