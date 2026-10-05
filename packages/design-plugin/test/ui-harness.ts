// The UI half of a plugin harness, shared by the Figma and Penpot plugin tests. It runs the module
// script of a built `ui.html` in this realm, with a fake DOM and with `process` and `fetch`
// removed, so the WebAssembly core must load from the bytes inlined in the page, as it must in the
// tool's iframe. Each plugin's harness runs its sandbox half and wires the messages.
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

export class FakeElement {
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

/** Loads the page's script with `window` and `parent` as the tool's iframe would give them. */
export async function loadUi(
  dist: string,
  frame: { window: object; parent: object },
): Promise<(id: string) => FakeElement> {
  const elements = new Map<string, FakeElement>();
  const byId = (id: string): FakeElement => {
    let found = elements.get(id);
    if (found === undefined) elements.set(id, (found = new FakeElement()));
    return found;
  };
  Object.assign(globalThis, {
    ...frame,
    document: {
      getElementById: byId,
      createElement: () => new FakeElement(),
      execCommand: () => true,
    },
  });
  Object.defineProperty(globalThis, "process", { value: undefined, configurable: true });
  Object.defineProperty(globalThis, "fetch", { value: undefined, configurable: true });

  const html = readFileSync(join(dist, "ui.html"), "utf8");
  const script = /<script type="module">\n([\s\S]*)<\/script>/.exec(html)?.[1];
  if (script === undefined) throw new Error("ui.html has no module script");
  const file = join(mkdtempSync(join(tmpdir(), "weft-plugin-ui-")), "ui.mjs");
  writeFileSync(file, script);
  await import(pathToFileURL(file).href);
  return byId;
}

/**
 * Loads `resolver` when given, builds `markup`, exports the selection, then tries broken markup;
 * returns what the UI shows.
 */
export async function runUi(
  byId: (id: string) => FakeElement,
  markup: string,
  settle: () => Promise<void>,
  resolver?: string,
): Promise<{
  loaded: string;
  built: string;
  exported: {
    status: string;
    markup: string;
    notes: string[];
    resolver: string;
    downloadable: boolean;
  };
  broken: { status: string; notes: number };
}> {
  if (resolver !== undefined) {
    byId("resolver").value = resolver;
    byId("load-resolver").click();
  }
  const loaded = byId("status").textContent;
  byId("source").value = markup;
  byId("build").click();
  await settle();
  const built = byId("status").textContent;

  byId("export").click();
  await settle();
  const exported = {
    status: byId("status").textContent,
    markup: byId("result").value,
    notes: byId("notes").items.map((i) => i.textContent),
    resolver: byId("resolver-out").value,
    downloadable: !byId("download-resolver").disabled,
  };

  byId("source").value = "<screen";
  byId("build").click();
  await settle();
  const broken = { status: byId("status").textContent, notes: byId("notes").items.length };
  return { loaded, built, exported, broken };
}
