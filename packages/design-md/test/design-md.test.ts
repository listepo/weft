import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { loadTokens } from "@weft/catalog";
import fc from "fast-check";
import { test } from "vitest";
import { fromDesignMd } from "../src/index.ts";

const fixture = (path: string): string =>
  readFileSync(new URL(`./fixtures/${path}`, import.meta.url), "utf8");
type Tree = Record<string, Record<string, { $type: string; $value: any }>>;
const frontmatter = (yaml: string): string => `---\n${yaml}\n---\n`;

for (const name of ["paws-and-paths", "atmospheric-glass", "totality-festival"]) {
  test(`Google Labs example ${name} maps and loads in the Weft token loader`, () => {
    const {
      tokens,
      losses,
      problems,
      name: title,
    } = fromDesignMd(fixture(`google-labs/${name}/DESIGN.md`));
    assert.deepEqual(problems, []);
    assert.ok(title);
    assert.deepEqual(loadTokens(tokens).problems, []);
    assert.ok(
      losses.some((l) => l.kind === "component"),
      "the components are reported",
    );
    assert.ok(losses.some((l) => l.kind === "prose"));
    assert.equal(
      losses.filter((l) => l.kind === "unresolved-alias" || l.kind === "invalid-name").length,
      0,
    );
  });
}

test("the colors, radii, spacing and sizes agree with the design_tokens.json Google exports", () => {
  const ours = fromDesignMd(fixture("google-labs/paws-and-paths/DESIGN.md")).tokens as Tree;
  const theirs = JSON.parse(fixture("google-labs/paws-and-paths/design_tokens.json")) as Tree;
  for (const section of ["rounded", "spacing"]) {
    assert.ok(Object.keys(theirs[section] ?? {}).length > 0, section);
    assert.deepEqual(ours[section], theirs[section], section);
  }
  // Google rounds components to three digits, this mapping to four; the hex is the shared truth.
  assert.deepEqual(Object.keys(ours["colors"] ?? {}), Object.keys(theirs["colors"] ?? {}));
  for (const [name, token] of Object.entries(theirs["colors"] ?? {})) {
    const mine = ours["colors"]?.[name]?.$value;
    assert.equal(mine.hex, token.$value.hex, name);
    mine.components.forEach((c: number, i: number) =>
      assert.ok(Math.abs(c - token.$value.components[i]) < 0.001, name),
    );
  }
  for (const [name, token] of Object.entries(theirs["typography"] ?? {})) {
    const mine = ours["typography"]?.[name]?.$value;
    assert.deepEqual(mine.fontSize, token.$value.fontSize, name);
    assert.equal(mine.fontFamily, token.$value.fontFamily, name);
    assert.equal(mine.fontWeight, token.$value.fontWeight, name);
  }
});

test("typography is rewritten to the DTCG 2025.10 form and each rewrite is reported", () => {
  const { tokens, losses } = fromDesignMd(
    frontmatter(`name: T
typography:
  a: { fontFamily: Inter, fontSize: 40px, fontWeight: "600", lineHeight: 48px, letterSpacing: -0.02em }
  b: { fontFamily: "Inter, sans-serif", fontSize: 1rem, fontWeight: 400, lineHeight: 1.5, letterSpacing: 2px }
  c: { fontFamily: Inter, fontSize: 10px, lineHeight: 1.2em }`),
  );
  const t = (tokens as Tree)["typography"] ?? {};
  assert.deepEqual(t["a"]?.$value, {
    fontFamily: "Inter",
    fontSize: { value: 40, unit: "px" },
    fontWeight: 600,
    lineHeight: 1.2,
    letterSpacing: { value: -0.8, unit: "px" },
  });
  assert.deepEqual(t["b"]?.$value.fontFamily, ["Inter", "sans-serif"]);
  assert.equal(t["b"]?.$value.lineHeight, 1.5);
  assert.deepEqual(t["b"]?.$value.letterSpacing, { value: 2, unit: "px" });
  assert.equal(t["c"]?.$value.lineHeight, 1.2);
  assert.equal(t["c"]?.$value.fontWeight, 400, "a missing weight is filled and said so");
  const converted = losses.filter((l) => l.kind === "converted").map((l) => l.path);
  assert.deepEqual(converted.filter((p) => p === "typography.a").length, 2);
  assert.ok(converted.includes("typography.c"));
  assert.equal(converted.includes("typography.b"), false, "nothing was rewritten in b");
});

test("references stay references, and a reference to nothing is dropped", () => {
  const { tokens, losses } = fromDesignMd(
    frontmatter(`colors:
  primary: "#112233"
  link: "{colors.primary}"
  ghost: "{colors.nowhere}"
  a: "{colors.b}"
  b: "{colors.a}"
spacing:
  base: 8px
  gutter: "{spacing.base}"`),
  );
  const t = tokens as Tree;
  assert.equal(t["colors"]?.["link"]?.$value, "{colors.primary}");
  assert.equal(t["colors"]?.["link"]?.$type, "color");
  assert.equal(t["spacing"]?.["gutter"]?.$type, "dimension");
  assert.equal(t["colors"]?.["ghost"], undefined);
  assert.equal(t["colors"]?.["a"], undefined);
  assert.equal(losses.filter((l) => l.kind === "unresolved-alias").length, 3);
  assert.deepEqual(loadTokens(tokens).problems, []);
});

test("unreadable values are losses; an unquoted hex color is explained", () => {
  const { tokens, losses } = fromDesignMd(
    frontmatter(`colors:
  bare: #ffffff
  named: red
  fine: "#fff"
rounded:
  a: 4
  b: 50%
  c: 4px
extra:
  x: 1`),
  );
  const t = tokens as Tree;
  assert.deepEqual(Object.keys(t["colors"] ?? {}), ["fine"]);
  assert.deepEqual(Object.keys(t["rounded"] ?? {}), ["c"]);
  assert.match(losses.find((l) => l.path === "colors.bare")?.note ?? "", /quote hex colors/);
  assert.ok(losses.some((l) => l.kind === "unsupported-section" && l.path === "extra"));
  assert.equal(losses.filter((l) => l.kind === "unsupported-value").length, 4);
});

test("names DTCG reserves are losses, and a missing frontmatter or bad YAML is a problem", () => {
  const dotted = fromDesignMd(frontmatter('colors:\n  "a.b": "#000"\n  ok: "#000"'));
  assert.deepEqual(Object.keys((dotted.tokens as Tree)["colors"] ?? {}), ["ok"]);
  assert.equal(dotted.losses.filter((l) => l.kind === "invalid-name").length, 1);

  const prose = fromDesignMd(fixture("open-design/minimal/DESIGN.md"));
  assert.deepEqual(prose.tokens, {});
  assert.match(prose.problems[0] ?? "", /tokens\.css/);
  assert.equal(prose.losses[0]?.kind, "prose");

  assert.equal(fromDesignMd(frontmatter("colors: [unclosed")).problems.length, 1);
  assert.equal(fromDesignMd(frontmatter("- just\n- a list")).problems.length, 1);
  assert.equal(fromDesignMd("x".repeat(1_000_001)).problems.length, 1);
});

test("YAML aliases are refused: a token file has no use for an expansion bomb", () => {
  const bomb = frontmatter("a: &a [x, x, x]\nb: &b [*a, *a, *a]\ncolors: *b");
  assert.equal(fromDesignMd(bomb).problems.length, 1);
});

test("components are reported per component with the properties they set", () => {
  const { losses } = fromDesignMd(
    frontmatter('components:\n  button:\n    backgroundColor: "{colors.p}"\n    rounded: 4px'),
  );
  assert.equal(losses.length, 1);
  assert.equal(losses[0]?.kind, "component");
  assert.match(losses[0]?.note ?? "", /backgroundColor, rounded/);
});

test("any input maps without throwing, and what comes out always loads", () => {
  const yaml = fc.stringMatching(/^[-:{}[\]"'#\w\s.,%&*]{0,300}$/);
  fc.assert(
    fc.property(yaml, (body) => {
      const { tokens } = fromDesignMd(frontmatter(body));
      assert.deepEqual(loadTokens(tokens).problems, []);
    }),
    { numRuns: 300 },
  );
});
