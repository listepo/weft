import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { coreCatalog, loadTokens, tokenTypes } from "@weft/catalog";
import { parse, serialize } from "@weft/core";
import { CORPUS_DIR, SCREENS } from "../src/corpus.ts";

// Lives in bench because it reads both the corpus and the catalog package; neither depends on bench.
const CATALOG = new URL("../../packages/catalog/", import.meta.url);
const examples = readdirSync(new URL("examples/", CATALOG))
  .filter((f) => f.endsWith(".weft"))
  .map((f) => new URL(`examples/${f}`, CATALOG));
const screens = SCREENS.map((s) => new URL(`file://${CORPUS_DIR}${s}/screen.weft`));
const tokens = tokenTypes(
  loadTokens(JSON.parse(readFileSync(new URL("tokens/default.tokens.json", CATALOG), "utf8")))
    .tokens,
);

for (const file of [...screens, ...examples]) {
  const name = file.pathname.split("/").slice(-2).join("/");
  test(`${name} is valid in strict mode and canonical`, () => {
    const src = readFileSync(file, "utf8");
    const result = parse(src, { catalog: coreCatalog, mode: "strict", tokens });
    assert.deepEqual(result.diagnostics, []);
    assert.ok(result.document);
    assert.equal(
      serialize(result.document),
      src,
      "run `node packages/core/src/cli.ts fmt --write`",
    );
  });
}
