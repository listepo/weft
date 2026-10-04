// The plugin's UI, a browser page Figma shows in an iframe. It holds the Weft core (WebAssembly):
// it parses pasted markup before the main thread builds it, and finishes and serializes what the
// main thread reads back. Replies are shown as text, never as HTML: layer names and texts in them
// come from whatever file the plugin runs in.
import { coreCatalog, loadTokens } from "@weft/catalog";
import type { Diagnostic } from "@weft/core";
import {
  buildRequest,
  exportRequest,
  finishExport,
  type PluginReply,
  type PluginRequest,
} from "@weft/figma";

/** The default token set, inlined by the build: the UI has no files beside it. */
declare const WEFT_DEFAULT_TOKENS: unknown;

const options = { catalog: coreCatalog, tokens: loadTokens(WEFT_DEFAULT_TOKENS).tokens };

const element = <T extends HTMLElement>(id: string): T => {
  const found = document.getElementById(id);
  if (found === null) throw new Error(`ui.html has no #${id}`);
  return found as T;
};

const source = element<HTMLTextAreaElement>("source");
const result = element<HTMLTextAreaElement>("result");
const copy = element<HTMLButtonElement>("copy");
const download = element<HTMLButtonElement>("download");
let fileName = "screen.weft";

const send = (pluginMessage: PluginRequest) => parent.postMessage({ pluginMessage }, "*");
const setStatus = (text: string) => {
  element("status").textContent = text;
};
const describe = (d: Diagnostic) => `${d.severity} ${d.code}: ${d.message}`;
const showNotes = (items: readonly string[]) => {
  const list = element("notes");
  list.replaceChildren();
  for (const text of items) {
    const li = document.createElement("li");
    li.textContent = text;
    list.append(li);
  }
};

element<HTMLInputElement>("file").addEventListener("change", async (event) => {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (file !== undefined) source.value = await file.text();
});

element("build").addEventListener("click", () => {
  const { request, diagnostics, message } = buildRequest(source.value, options);
  showNotes(diagnostics.map(describe));
  if (request === undefined) {
    setStatus(message ?? "Nothing to build.");
    return;
  }
  setStatus("Building…");
  send(request);
});

element("export").addEventListener("click", () => {
  setStatus("Exporting…");
  send(exportRequest(options));
});

copy.addEventListener("click", () => {
  // The plugin iframe has no clipboard permission, so the text is copied through a selection.
  result.select();
  document.execCommand("copy");
  setStatus("Copied.");
});

download.addEventListener("click", () => {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([result.value], { type: "text/plain" }));
  link.download = fileName;
  link.click();
  URL.revokeObjectURL(link.href);
});

window.onmessage = (event: MessageEvent) => {
  const reply = (event.data as { pluginMessage?: PluginReply } | null)?.pluginMessage;
  if (reply === undefined || typeof reply.type !== "string") return;
  if (reply.type === "built") {
    setStatus("Built.");
  } else if (reply.type === "exported") {
    const file = finishExport(reply, options);
    fileName = file.fileName;
    result.value = file.markup;
    copy.disabled = false;
    download.disabled = false;
    const losses = file.losses.map((l) => `${l.kind} at ${l.path}: ${l.note}`);
    setStatus(losses.length === 0 ? "Exported." : `Exported with ${losses.length} losses.`);
    showNotes([...losses, ...file.diagnostics.map(describe)]);
  } else if (reply.type === "error") {
    setStatus(reply.message);
    showNotes((reply.diagnostics ?? []).map(describe));
  }
};
