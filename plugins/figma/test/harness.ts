// Runs a built plugin the way Figma splits it, in a process of its own so the UI half can lose
// Node's globals: `node test/harness.ts <dist> <screen.weft>` prints one JSON line of results.
//
// - Main thread: `code.js` in a VM context without WebAssembly or fetch, over the fake file.
// - UI: the module script of `ui.html` in this realm, with `process` and `fetch` removed, so the
//   WebAssembly core must load from the bytes inlined in the page, as it must in Figma's iframe.
// Messages between the two are structured-cloned, as `postMessage` does.
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { createContext, runInContext } from "node:vm";
import { FakeFigma, type FakeNode } from "../../../packages/figma/test/fake-figma.ts";

const [dist = "", markupPath = ""] = process.argv.slice(2);
const print = process.stdout.write.bind(process.stdout);

class FakeElement {
  value = "";
  textContent = "";
  disabled = false;
  items: FakeElement[] = [];
  readonly listeners = new Map<string, (event: unknown) => unknown>();
  addEventListener(type: string, listener: (event: unknown) => unknown) {
    this.listeners.set(type, listener);
  }
  click() {
    return this.listeners.get("click")?.({ target: this });
  }
  replaceChildren() {
    this.items = [];
  }
  append(child: FakeElement) {
    this.items.push(child);
  }
  select() {}
}

const elements = new Map<string, FakeElement>();
const byId = (id: string): FakeElement => {
  let found = elements.get(id);
  if (found === undefined) elements.set(id, (found = new FakeElement()));
  return found;
};

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
Object.assign(globalThis, {
  window,
  document: {
    getElementById: byId,
    createElement: () => new FakeElement(),
    execCommand: () => true,
  },
  parent: {
    postMessage: (message: { pluginMessage: unknown }) => {
      pending.push(ui.onmessage?.(structuredClone(message.pluginMessage)) ?? Promise.resolve());
    },
  },
});
Object.defineProperty(globalThis, "process", { value: undefined, configurable: true });
Object.defineProperty(globalThis, "fetch", { value: undefined, configurable: true });

const html = readFileSync(join(dist, "ui.html"), "utf8");
const script = /<script type="module">\n([\s\S]*)<\/script>/.exec(html)?.[1];
if (script === undefined) throw new Error("ui.html has no module script");
const file = join(mkdtempSync(join(tmpdir(), "weft-figma-ui-")), "ui.mjs");
writeFileSync(file, script);
await import(pathToFileURL(file).href);

const settle = async () => {
  while (pending.length > 0) await pending.shift();
};

byId("source").value = readFileSync(markupPath, "utf8");
byId("build").click();
await settle();
const built = { status: byId("status").textContent, selected: page.selection.length };

byId("export").click();
await settle();
const exported = {
  status: byId("status").textContent,
  markup: byId("result").value,
  notes: byId("notes").items.map((i) => i.textContent),
};

byId("source").value = "<screen";
byId("build").click();
await settle();
const broken = { status: byId("status").textContent, notes: byId("notes").items.length };

print(`${JSON.stringify({ mainHasWasm, built, exported, broken })}\n`);
