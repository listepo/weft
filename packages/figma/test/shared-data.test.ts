// The Weft source is shared plugin data (namespace `weft`), so other readers (the Figma MCP
// server, the REST API with `plugin_data=shared`) see it; files built before T14.2 hold it in
// private plugin data and must keep working, and move to shared data as the plugin touches them.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { serialize } from "@weft/core";
import { describe, test } from "vitest";
import { dataOf, NAMESPACE } from "../src/data.ts";
import { buildScreen, displayTexts, ensureLibrary } from "../src/index.ts";
import type { FakeBase, FakeFigma } from "./fake-figma.ts";
import { built, corpusMarkup, parseStrict, read, tokens } from "./helpers.ts";

const privateKeys = (figma: FakeFigma): string[] =>
  figma.everyDataHolderFor().flatMap((h) => [...h.data.keys()].map((k) => `${h.name}: ${k}`));

const sharedCount = (figma: FakeFigma): number =>
  figma.everyDataHolderFor().reduce((n, h) => n + (h.shared.get(NAMESPACE)?.size ?? 0), 0);

describe("a build", () => {
  test("writes the Weft source only as shared plugin data", async () => {
    const { figma } = await built(corpusMarkup("login"));
    assert.deepEqual(privateKeys(figma), []);
    assert.ok(sharedCount(figma) > 0);
    // The library page, its board, the token collection and its variables are marked too.
    const page = figma.root.children.find((p) => p.name === "Weft library");
    assert.ok(page?.shared.get(NAMESPACE)?.has("weft.library"));
    assert.ok(figma.variables.all.every((v) => v.shared.get(NAMESPACE)?.has("weft.token")));
  });
});

describe("a file built before T14.2", () => {
  test("reads back byte-identical from private data, which then moves to shared data", async () => {
    const markup = corpusMarkup("login");
    const { figma, frame } = await built(markup);
    figma.privateDataFor(NAMESPACE);
    const result = await read(figma, frame);
    assert.deepEqual(result.losses, []);
    assert.equal(serialize(result.document), markup);
    // Every layer of the frame had its data read, so all of it moved on load.
    const left: string[] = [];
    const walk = (n: FakeBase) => {
      left.push(...n.data.keys());
      for (const c of "children" in n ? (n.children as FakeBase[]) : []) walk(c);
    };
    walk(frame);
    assert.deepEqual(left, []);
    assert.ok((frame.shared.get(NAMESPACE)?.size ?? 0) > 0);
    // A second read finds everything it reads in shared data.
    assert.equal(serialize((await read(figma, frame)).document), markup);
  });

  test("keeps its library: a new build finds it through the private marks", async () => {
    const { figma } = await built(corpusMarkup("login"));
    figma.privateDataFor(NAMESPACE);
    const pages = figma.root.children.length;
    const variables = figma.variables.all.length;
    const library = await ensureLibrary(figma, coreCatalog, tokens);
    const document = parseStrict(corpusMarkup("settings"));
    const frame = await buildScreen(figma, document, {
      catalog: coreCatalog,
      library,
      tokens,
      display: displayTexts(document),
    });
    assert.equal(figma.root.children.length, pages, "no second library page");
    assert.equal(figma.variables.all.length, variables, "no second set of variables");
    assert.notEqual(dataOf(frame).getPluginData("weft.document"), "");
    // The library marks the build wrote again are shared now.
    const page = figma.root.children.find((p) => p.name === "Weft library");
    assert.equal(page?.data.size, 0);
    assert.ok(page?.shared.get(NAMESPACE)?.has("weft.library"));
  });
});

describe("dataOf", () => {
  const node = (writable: boolean) => {
    const own = new Map<string, string>();
    const shared = new Map<string, string>();
    const write = (map: Map<string, string>) => (key: string, value: string) => {
      if (!writable) throw new Error("read-only");
      if (value === "") map.delete(key);
      else map.set(key, value);
    };
    return {
      own,
      shared,
      getPluginData: (key: string) => own.get(key) ?? "",
      setPluginData: write(own),
      getSharedPluginData: (namespace: string, key: string) =>
        namespace === NAMESPACE ? (shared.get(key) ?? "") : "",
      setSharedPluginData: (_namespace: string, key: string, value: string) =>
        write(shared)(key, value),
    };
  };

  test("prefers shared data to private data", () => {
    const n = node(true);
    n.own.set("k", "old");
    n.shared.set("k", "new");
    assert.equal(dataOf(n).getPluginData("k"), "new");
  });

  test("a write goes to shared data and clears the private value", () => {
    const n = node(true);
    n.own.set("k", "old");
    dataOf(n).setPluginData("k", "new");
    assert.deepEqual([[...n.own], [...n.shared]], [[], [["k", "new"]]]);
  });

  test("a node that cannot be written still reads its private data", () => {
    const n = node(false);
    n.own.set("k", "old");
    assert.equal(dataOf(n).getPluginData("k"), "old");
    assert.equal(n.shared.size, 0);
  });
});
