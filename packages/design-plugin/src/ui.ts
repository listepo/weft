// The UI of a Weft design-tool plugin, a browser page the tool shows in an iframe. It holds the
// Weft core (WebAssembly): it parses pasted markup before the sandbox builds it, and finishes and
// serializes what the sandbox reads back. Each tool passes how messages travel. Replies are shown
// as text, never as HTML: layer names and texts in them come from whatever file the plugin runs in.
import { coreCatalog, loadTokens } from "@weft/catalog";
import type { Diagnostic } from "@weft/core";
import {
  buildRequest,
  exportRequest,
  finishExport,
  type PluginReply,
  type PluginRequest,
} from "@weft/design-tool";

/** The default token set, inlined by the build: the UI has no files beside it. */
declare const WEFT_DEFAULT_TOKENS: unknown;

/** How a tool carries messages between its UI and its sandbox. */
export type Transport = {
  send(request: PluginRequest): void;
  /** Calls `onReply` with each message from the sandbox, unchecked. */
  listen(onReply: (reply: unknown) => void): void;
};

const isReply = (value: unknown): value is PluginReply =>
  typeof value === "object" &&
  value !== null &&
  typeof (value as { type?: unknown }).type === "string";

export function startUi(transport: Transport): void {
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
    transport.send(request);
  });

  element("export").addEventListener("click", () => {
    setStatus("Exporting…");
    transport.send(exportRequest(options));
  });

  copy.addEventListener("click", () => {
    // A plugin iframe may have no clipboard permission, so the text is copied through a selection.
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

  transport.listen((reply) => {
    if (!isReply(reply)) return;
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
  });
}
