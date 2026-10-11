// `importCem` of `@weft/core/cem`: the Custom Elements Manifest importer of crates/weft-import
// through the web module (or the addon), with the catalog it returns used as a catalog.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, test } from "vitest";
import { importCem } from "../src/cem.ts";
import { hasErrors } from "../src/diagnostics.ts";
import { parse } from "../src/parse.ts";
import { catalog as fixture } from "./catalog.ts";

const MANIFEST = readFileSync(
  new URL("../../../crates/weft-import/tests/fixtures/cem/acme-ui.json", import.meta.url),
  "utf8",
);

describe("a manifest imports into a catalog", () => {
  const result = importCem(MANIFEST, { catalog: fixture, name: "acme-ui", version: "1.0.0" });

  test("each custom element is a kind, named and versioned as asked", () => {
    assert.equal(result.catalog.name, "acme-ui");
    assert.equal(result.catalog.version, "1.0.0");
    // The manifest also defines `date-picker`, a core kind the fixture catalog lacks, so it stays.
    assert.deepEqual(Object.keys(result.catalog.components), [
      "acme-button",
      "acme-rating",
      "acme-card",
      "acme-badge",
      "date-picker",
    ]);
    assert.deepEqual(result.diagnostics, []);
  });

  test("what a catalog cannot hold is listed as losses", () => {
    const kinds = new Set(result.losses.map((l) => l.kind));
    assert.ok(kinds.has("props") && kinds.has("structure"), [...kinds].join());
    for (const loss of result.losses) assert.match(loss.path, /^#/);
  });

  test("the catalog checks documents of its kinds", () => {
    const markup =
      '<acme-card id="card" weft="0.1"><acme-button id="buy" variant="primary" on-acme-click="cart.add">Buy</acme-button></acme-card>';
    const ok = parse(markup, { catalog: result.catalog, mode: "strict" });
    assert.equal(hasErrors(ok.diagnostics), false, JSON.stringify(ok.diagnostics));
    const bad = parse(markup.replace("primary", "primay"), { catalog: result.catalog });
    assert.ok(bad.diagnostics.some((d) => d.code === "W203"));
  });

  test("a kind the base catalog already has is left out", () => {
    const again = importCem(MANIFEST, { catalog: result.catalog, name: "x", version: "1.0.0" });
    assert.deepEqual(again.catalog.components, {});
    assert.ok(again.losses.some((l) => l.kind === "kinds"));
  });
});

describe("unreadable input is W601, never an exception", () => {
  for (const input of ["{ not json", '{"schemaVersion":"9.0.0","modules":[]}', 42]) {
    test(JSON.stringify(input), () => {
      const result = importCem(input as string, { catalog: fixture, name: "x", version: "0.0.0" });
      assert.deepEqual(result.catalog.components, {});
      assert.deepEqual(
        result.diagnostics.map((d) => d.code),
        ["W601"],
      );
    });
  }
});

test("a prefix makes a library: it says so, requires the core and keeps only its own tags", () => {
  const result = importCem(MANIFEST, {
    catalog: fixture,
    name: "acme-ui",
    version: "1.0.0",
    prefix: "acme",
  });
  assert.equal(result.catalog.prefix, "acme");
  assert.deepEqual(Object.keys(result.catalog.requires ?? {}), ["weft-core"]);
  assert.equal(Object.hasOwn(result.catalog.components, "date-picker"), false);
  assert.ok(
    result.losses.some(
      (l) => l.kind === "kinds" && l.note.includes("outside the catalog's prefix"),
    ),
  );
});
