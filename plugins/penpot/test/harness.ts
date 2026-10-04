// Runs a built plugin the way Penpot splits it, in a process of its own so the UI half can lose
// Node's globals: `node test/harness.ts <dist> <screen.weft>` prints one JSON line of results.
//
// - Sandbox: `plugin.js` evaluated as a script in an SES compartment of a locked-down realm, with
//   the endowments Penpot gives a plugin (plugins-runtime `create-sandbox.ts`), over the fake file
//   of @weft/penpot's tests.
// - UI: the module script of `ui.html` in this realm (`@weft/design-plugin`'s UI harness).
// Messages between the two are structured-cloned, as `postMessage` does.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { createContext, runInContext } from "node:vm";
import { loadUi, runUi } from "../../../packages/design-plugin/test/ui-harness.ts";
import { FakePenpot, type FakeShape } from "../../../packages/penpot/test/fake-penpot.ts";

const [dist = "", markupPath = ""] = process.argv.slice(2);
const print = process.stdout.write.bind(process.stdout);

// The sandbox.
const fake = new FakePenpot();
let onMessage: ((message: unknown) => Promise<void>) | undefined;
let opened: { name: string; url: string } | undefined;
const penpot = Object.assign(fake, {
  selection: [] as FakeShape[],
  viewport: { zoomIntoView: () => {} },
  ui: {
    open: (name: string, url: string) => {
      opened = { name, url };
    },
    onMessage: (callback: (message: unknown) => Promise<void>) => {
      onMessage = callback;
    },
    sendMessage: (message: unknown) => toUi(structuredClone(message)),
  },
});
Object.assign(fake.currentPage, {
  getShapeById: (id: string) => fake.currentPage.root.children.find((s) => s.id === id) ?? null,
});

const ses = join(dirname(createRequire(import.meta.url).resolve("ses/package.json")), "dist");
const realm = createContext({});
runInContext(readFileSync(join(ses, "ses.umd.js"), "utf8"), realm);
runInContext("lockdown();", realm);
const lockedDown = runInContext("Object.isFrozen(Object.prototype)", realm) as boolean;
Object.assign(realm, {
  endowments: {
    penpot,
    fetch: () => Promise.reject(new Error("the harness has no network")),
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    console,
    structuredClone,
    atob,
    btoa,
  },
  code: readFileSync(join(dist, "plugin.js"), "utf8"),
});
const sandboxHasWasm = runInContext(
  `const compartment = new Compartment(endowments);
   compartment.evaluate(code);
   compartment.evaluate("typeof WebAssembly") !== "undefined";`,
  realm,
) as boolean;

// The UI.
const listeners: ((event: { data: unknown; source: unknown }) => void)[] = [];
const pending: Promise<void>[] = [];
const parent = {
  postMessage: (message: unknown) => {
    pending.push(onMessage?.(structuredClone(message)) ?? Promise.resolve());
  },
};
const toUi = (data: unknown) => {
  for (const listener of listeners) listener({ data, source: parent });
};
const byId = await loadUi(dist, {
  window: {
    addEventListener: (_type: string, listener: (typeof listeners)[number]) =>
      listeners.push(listener),
  },
  parent,
});

let selected = 0;
const result = await runUi(byId, readFileSync(markupPath, "utf8"), async () => {
  while (pending.length > 0) await pending.shift();
  selected ||= penpot.selection.length;
});

print(
  `${JSON.stringify({
    lockedDown,
    sandboxHasWasm,
    opened,
    built: { status: result.built, selected },
    exported: result.exported,
    broken: result.broken,
  })}\n`,
);
