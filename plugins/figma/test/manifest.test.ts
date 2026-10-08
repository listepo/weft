// The pull reads the plugin data this plugin writes, which Figma keys by the manifest's id, so the
// pull's default plugin id (and `import.figma.pluginId`'s default, SPEC §10.6) must be that id.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { PLUGIN_ID } from "@weft/figma/pull";
import { test } from "vitest";

test("the pull's default plugin id is the manifest's id", () => {
  const manifest = JSON.parse(readFileSync(new URL("../manifest.json", import.meta.url), "utf8"));
  assert.equal(manifest.id, PLUGIN_ID);
});
