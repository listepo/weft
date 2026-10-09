// SPEC §8: a 0.1 reader survives documents from the future and from other vendors.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";
import { hasErrors, parse, serialize, stringify, validate, type Mode } from "../src/index.ts";
import { catalog } from "./catalog.ts";

const read = (name: string) =>
  readFileSync(new URL(`../../../compat/${name}.weft`, import.meta.url), "utf8");

type Expected = [code: string, path: string][];

// Codes whose severity follows the mode; every other code is an error whatever the mode.
const MODE_CODES = new Set(["W401", "W402", "W403"]);

const fixtures: {
  name: string;
  diagnostics: Expected;
  /** Unknown content the round trip must keep. */
  kept: string[];
}[] = [
  {
    name: "unknown-element",
    diagnostics: [
      ["W403", "/screen#s/@weft"],
      ["W401", "/screen#s/stack#st/hologram#holo"],
    ],
    kept: ["hologram", "intensity"],
  },
  {
    name: "unknown-attribute",
    diagnostics: [
      ["W403", "/screen#s/@weft"],
      ["W402", "/screen#s/stack#st/button#go/@glow"],
    ],
    kept: ["glow"],
  },
  {
    name: "unknown-with-children",
    diagnostics: [
      ["W403", "/screen#s/@weft"],
      ["W401", "/screen#s/stack#st/carousel#car"],
    ],
    kept: ["carousel", "autoplay", "controls", "Previous"],
  },
  {
    name: "unknown-in-restricted-parent",
    diagnostics: [
      ["W403", "/screen#s/@weft"],
      ["W401", "/screen#s/list#l/badge#b1"],
    ],
    kept: ["badge"],
  },
  {
    name: "extensions",
    diagnostics: [],
    kept: ["x-acme-chart", "x-acme-kind", "x-acme-density", "x-acme-theme", "legend", "chart.open"],
  },
  {
    name: "extension-without-role",
    diagnostics: [["W210", "/screen#s/stack#st/x-acme-chart#chart"]],
    kept: ["x-acme-chart"],
  },
  { name: "unsupported-major", diagnostics: [["W404", "/screen#s/@weft"]], kept: [] },
];

const summary = (diagnostics: { code: string; path: string; severity: string }[]) =>
  diagnostics.map((d) => [d.code, d.path, d.severity]);

for (const { name, diagnostics, kept } of fixtures) {
  const markup = read(name);
  const expect = (mode: Mode) =>
    diagnostics.map(([code, path]) => [
      code,
      path,
      MODE_CODES.has(code) && mode === "lenient" ? "warning" : "error",
    ]);

  test(`${name}: lenient diagnostics`, () => {
    const result = parse(markup, { catalog, mode: "lenient" });
    assert.deepEqual(summary(result.diagnostics), expect("lenient"));
    assert.equal(
      hasErrors(result.diagnostics),
      diagnostics.some(([code]) => !MODE_CODES.has(code)),
    );
  });

  test(`${name}: strict diagnostics`, () => {
    const result = parse(markup, { catalog, mode: "strict" });
    assert.deepEqual(summary(result.diagnostics), expect("strict"));
    assert.equal(hasErrors(result.diagnostics), diagnostics.length > 0);
  });

  test(`${name}: markup and canonical JSON round-trip keep unknown content`, () => {
    const { document } = parse(markup, { catalog });
    assert.ok(document);

    const again = serialize(document);
    for (const needle of kept) assert.ok(again.includes(needle), `${needle} survives serialize`);
    const reparsed = parse(again, { catalog });
    assert.deepEqual(reparsed.document, document);
    assert.equal(serialize(reparsed.document ?? document), again);

    const json = stringify(document);
    for (const needle of kept) assert.ok(json.includes(needle), `${needle} survives stringify`);
    const decoded: unknown = JSON.parse(json);
    assert.deepEqual(decoded, document);
    assert.deepEqual(
      summary(validate(decoded, { catalog })).map(([code]) => code),
      diagnostics.map(([code]) => code),
    );
    assert.equal(stringify(decoded as typeof document), json);
  });
}

test("the parent's content model still applies to an unknown child", () => {
  const markup = `<screen id="s" label="L" weft="0.3"><button id="b"><hologram id="h"/></button></screen>`;
  const result = parse(markup, { catalog, mode: "lenient" });
  assert.deepEqual(
    result.diagnostics.map((d) => [d.code, d.severity]),
    [
      ["W403", "warning"],
      ["W304", "error"],
      ["W401", "warning"],
    ],
  );
});

test("a known child keeps its own rules below an unknown parent", () => {
  const markup = `<screen id="s" label="L" weft="0.3"><hologram id="h"><item id="i">x</item><button id="b" bogus="1">B</button></hologram></screen>`;
  const result = parse(markup, { catalog, mode: "lenient" });
  assert.deepEqual(
    result.diagnostics.map((d) => [d.code, d.severity]),
    [
      ["W403", "warning"],
      ["W401", "warning"],
      ["W402", "warning"],
    ],
  );
});

test("undeclared events, slots and states of a known component stay schema errors", () => {
  const markup = `<screen id="s" label="L" weft="0.3"><button id="b" state="melting" on-hover="a.b"><slot name="extra"><text id="t">x</text></slot>B</button></screen>`;
  const codes = parse(markup, { catalog, mode: "lenient" }).diagnostics.map((d) => d.code);
  assert.deepEqual(codes.toSorted(), ["W203", "W206", "W207", "W403"]);
});
