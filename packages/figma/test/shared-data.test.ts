// The Weft source is shared plugin data (namespace `weft`), so other readers (the Figma MCP
// server, the REST API with `plugin_data=shared`) see it. Only shared data is supported: the
// narrow API (`FPluginData`) has no private calls, so the package can neither write nor read any.
import assert from "node:assert/strict";
import { serialize } from "@weft/core";
import { test } from "vitest";
import { NAMESPACE } from "../src/data.ts";
import type { FakeInstance, FakeText } from "./fake-figma.ts";
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

// Real Figma, checked through `use_figma` in T14.4: an instance shows its main component's shared
// data, and a detach keeps the instance's data (the inherited `weft.kind` included).
test("an instance shows its main's kind, and the read-back still reads it from the main", async () => {
  const markup = corpusMarkup("login");
  const { figma, frame } = await built(markup);
  const button = figma.find<FakeInstance>(frame, "button#submit");
  assert.equal(button.getSharedPluginData(NAMESPACE, "weft.kind"), "button");
  assert.equal(button.shared.get(NAMESPACE)?.has("weft.kind"), false);
  const result = await read(figma, frame);
  assert.deepEqual(result.losses, []);
  assert.equal(serialize(result.document), markup);
});

test("a detached instance keeps its source and reads back as the element it was", async () => {
  const markup = corpusMarkup("login");
  const { figma, frame } = await built(markup);
  const detached = figma.find<FakeInstance>(frame, "button#submit").detachInstance();
  assert.equal(detached.type, "FRAME");
  assert.deepEqual([...(detached.shared.get(NAMESPACE)?.keys() ?? [])].sort(), [
    "weft.kind",
    "weft.origin",
    "weft.source",
  ]);
  const unedited = await read(figma, frame);
  assert.deepEqual(unedited.losses, []);
  assert.equal(serialize(unedited.document), markup);

  figma.find<FakeText>(detached, "text").characters = "Log in";
  const edited = await read(figma, frame);
  assert.deepEqual(edited.losses, []);
  assert.equal(
    serialize(edited.document),
    markup.replace('variant="primary">Sign in</button>', 'variant="primary">Log in</button>'),
  );
});
