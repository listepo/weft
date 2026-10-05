// Runs a built plugin the way Figma splits it, in a process of its own so the UI half can lose
// Node's globals: `node test/harness.ts <dist> <screen.weft> [<resolver.json>]` prints one JSON line of results.
//
// - Main thread: `code.js` in a VM context without WebAssembly or fetch, over the fake file.
// - UI: the module script of `ui.html` in this realm (`@weft/design-plugin`'s UI harness).
// Messages between the two are structured-cloned, as `postMessage` does.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createContext, runInContext } from "node:vm";
import { loadUi, runUi } from "../../../packages/design-plugin/test/ui-harness.ts";
import { FakeFigma, type FakeNode } from "../../../packages/figma/test/fake-figma.ts";

const [dist = "", markupPath = "", resolverPath] = process.argv.slice(2);
const print = process.stdout.write.bind(process.stdout);

// The main thread.
const figma = new FakeFigma();
const page = figma.currentPage as typeof figma.currentPage & { selection: FakeNode[] };
page.selection = [];
const ui: { onmessage?: (message: unknown) => Promise<void>; postMessage: (m: unknown) => void } = {
  postMessage: (m) => toUi(structuredClone(m)),
};
const main = createContext({
  console,
  __html__: "",
  figma: Object.assign(figma, {
    ui,
    showUI: () => {},
    viewport: { scrollAndZoomIntoView: () => {} },
    getNodeByIdAsync: async (id: string) =>
      figma.currentPage.children.find((c) => c.id === id) ?? null,
  }),
});
runInContext("delete globalThis.WebAssembly; delete globalThis.fetch;", main);
runInContext(readFileSync(join(dist, "code.js"), "utf8"), main);
const mainHasWasm = runInContext("typeof WebAssembly", main) !== "undefined";

// The UI.
const window: { onmessage?: (event: { data: unknown }) => void } = {};
const toUi = (pluginMessage: unknown) => window.onmessage?.({ data: { pluginMessage } });
const pending: Promise<void>[] = [];
const byId = await loadUi(dist, {
  window,
  parent: {
    postMessage: (message: { pluginMessage: unknown }) => {
      pending.push(ui.onmessage?.(structuredClone(message.pluginMessage)) ?? Promise.resolve());
    },
  },
});

const settle = async () => {
  while (pending.length > 0) await pending.shift();
};
let selected = 0;
const result = await runUi(
  byId,
  readFileSync(markupPath, "utf8"),
  async () => {
    await settle();
    selected ||= page.selection.length;
  },
  resolverPath === undefined ? undefined : readFileSync(resolverPath, "utf8"),
);

print(
  `${JSON.stringify({
    mainHasWasm,
    loaded: result.loaded,
    built: { status: result.built, selected },
    exported: result.exported,
    broken: result.broken,
  })}\n`,
);
