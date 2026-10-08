// The Weft source is shared plugin data (namespace `weft`), so other readers (the Figma MCP
// server, the REST API with `plugin_data=shared`) see it. Only shared data is supported: the
// narrow API (`FPluginData`) has no private calls, so the package can neither write nor read any.
import assert from "node:assert/strict";
import { serialize } from "@weft/core";
import { test } from "vitest";
import { NAMESPACE } from "../src/data.ts";
import { built, corpusMarkup, read } from "./helpers.ts";

test("a build writes the Weft source only as shared data, and the frame reads back byte-identical", async () => {
  const markup = corpusMarkup("login");
  const { figma, frame } = await built(markup);
  const holders = figma.everyDataHolderFor();
  // Nothing lands outside the `weft` namespace.
  assert.deepEqual(
    holders.flatMap((h) => [...h.shared.keys()].filter((ns) => ns !== NAMESPACE)),
    [],
  );
  assert.ok(frame.shared.get(NAMESPACE)?.has("weft.document"));
  // The library page and the token variables are marked too.
  const page = figma.root.children.find((p) => p.name === "Weft library");
  assert.ok(page?.shared.get(NAMESPACE)?.has("weft.library"));
  assert.ok(figma.variables.all.every((v) => v.shared.get(NAMESPACE)?.has("weft.token")));
  const result = await read(figma, frame);
  assert.deepEqual(result.losses, []);
  assert.equal(serialize(result.document), markup);
});
