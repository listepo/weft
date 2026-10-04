import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { coreCatalog, loadTokens, tokenTypes } from "@weft/catalog";
import { applyPatches, DIAGNOSTIC_CODES, parse, serialize } from "@weft/core";

// Lives in bench because it reads the root document, the catalog and the core together. These
// checks keep AGENT-SPEC.md from drifting: an agent that follows a stale guide writes errors.
const ROOT = new URL("../../", import.meta.url);
const guide = readFileSync(new URL("AGENT-SPEC.md", ROOT), "utf8");
const tokens = tokenTypes(
  loadTokens(
    JSON.parse(readFileSync(new URL("packages/catalog/tokens/default.tokens.json", ROOT), "utf8")),
  ).tokens,
);
const options = { catalog: coreCatalog, mode: "strict" as const, tokens };

const blocks = (lang: string) =>
  [...guide.matchAll(new RegExp("```" + lang + "\\n([\\s\\S]*?)```", "g"))].map((m) => m[1] ?? "");

test("the guide names every component of the core catalog", () => {
  const missing = Object.keys(coreCatalog.components).filter((k) => !guide.includes(`\`${k}\``));
  assert.deepEqual(missing, []);
});

test("the guide tells how to fix every diagnostic code", () => {
  const missing = Object.keys(DIAGNOSTIC_CODES).filter((c) => !guide.includes(`| ${c} |`));
  assert.deepEqual(missing, []);
});

test("every markup example is valid in strict mode and canonical", () => {
  const examples = blocks("xml");
  assert.ok(examples.length > 0);
  for (const src of examples) {
    const result = parse(src, options);
    assert.deepEqual(result.diagnostics, []);
    assert.ok(result.document);
    assert.equal(serialize(result.document), src);
  }
});

test("every patch example applies to the first markup example", () => {
  const screen = parse(blocks("xml")[0] ?? "", options).document;
  assert.ok(screen);
  const lists = blocks("json");
  assert.ok(lists.length > 0);
  for (const list of lists) {
    const result = applyPatches(screen, JSON.parse(list), options);
    assert.deepEqual(result.diagnostics, []);
    assert.ok(result.document);
  }
});
