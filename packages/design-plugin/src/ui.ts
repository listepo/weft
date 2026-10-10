// The UI of a Weft design-tool plugin, a browser page the tool shows in an iframe. It holds the
// Weft core (WebAssembly): it parses pasted markup before the sandbox builds it, and finishes and
// serializes what the sandbox reads back. Each tool passes how messages travel. Replies are shown
// as text, never as HTML: layer names and texts in them come from whatever file the plugin runs in.
import { coreCatalog, loadTokens } from "@weft/catalog";
import type { Diagnostic, Entry } from "@weft/core";
import {
  buildRequest,
  exportRequest,
  finishExport,
  type PluginReply,
  type PluginRequest,
  type UiOptions,
} from "@weft/design-tool";
import { MAX_RESOLVER, readResolver, type LoadedResolver } from "./resolver.ts";

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
  const defaults = loadTokens(WEFT_DEFAULT_TOKENS).tokens;
  // A loaded resolver replaces the default tokens, as `tokens` of a project does.
  let resolver: LoadedResolver | undefined;
  const options = (): UiOptions => {
    return {
      catalog: coreCatalog,
      tokens: resolver?.tokens ?? defaults,
      modifier: resolver?.modifiers.find((m) => m.name === modifier.value),
    };
  };

  const element = <T extends HTMLElement>(id: string): T => {
    const found = document.getElementById(id);
    if (found === null) throw new Error(`ui.html has no #${id}`);
    return found as T;
  };

  const source = element<HTMLTextAreaElement>("source");
  const result = element<HTMLTextAreaElement>("result");
  const copy = element<HTMLButtonElement>("copy");
  const download = element<HTMLButtonElement>("download");
  const resolverBox = element<HTMLTextAreaElement>("resolver");
  const modifier = element<HTMLSelectElement>("modifier");
  const resolverOut = element<HTMLTextAreaElement>("resolver-out");
  const downloadResolver = element<HTMLButtonElement>("download-resolver");
  let fileName = "screen.weft";

  const setStatus = (text: string) => {
    element("status").textContent = text;
  };
  const describe = (d: Diagnostic) => `${d.severity} ${d.code}: ${d.message}`;
  // The form of the readable comments in generated code: kind, status, who claims to have written it.
  const note = (e: Entry) =>
    `${e.kind}${e.status === undefined ? "" : ` ${e.status}`} (${e.by} ${e.name}): ${e.text}`;
  const showList = (id: string, items: readonly string[], keep = false) => {
    const list = element(id);
    if (!keep) list.replaceChildren();
    for (const text of items) {
      const li = document.createElement("li");
      li.textContent = text;
      list.append(li);
    }
  };
  const showNotes = (items: readonly string[], keep = false) => showList("notes", items, keep);

  element<HTMLInputElement>("file").addEventListener("change", async (event) => {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file !== undefined) source.value = await file.text();
  });

  /** Lists the loaded resolver's modifiers; the appearance one is chosen, else none (no modes). */
  const listModifiers = () => {
    const none = document.createElement("option");
    none.value = "";
    none.textContent = "No modes";
    modifier.replaceChildren();
    modifier.append(none);
    for (const { name } of resolver?.modifiers ?? []) {
      const option = document.createElement("option");
      option.value = name;
      option.textContent = name;
      modifier.append(option);
    }
    modifier.value = resolver?.appearance ?? "";
  };

  const loadResolver = () => {
    const text = resolverBox.value.trim();
    resolver = undefined;
    if (text === "") {
      listModifiers();
      showNotes([]);
      setStatus("Using the default tokens, one mode.");
      return;
    }
    const loaded = readResolver(text);
    showNotes(loaded.diagnostics.map(describe));
    if (loaded.message !== undefined || loaded.tokens === undefined) {
      listModifiers();
      setStatus(loaded.message ?? "The resolver has no tokens; the default tokens stay.");
      return;
    }
    resolver = loaded;
    listModifiers();
    const count = loaded.modifiers.length;
    setStatus(`Resolver loaded with ${count} ${count === 1 ? "modifier" : "modifiers"}.`);
  };

  element<HTMLInputElement>("resolver-file").addEventListener("change", async (event) => {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file === undefined) return;
    // Checked before reading: the box then never holds more than the bound.
    if (file.size > MAX_RESOLVER) {
      setStatus(`The resolver is larger than ${MAX_RESOLVER} characters.`);
      return;
    }
    resolverBox.value = await file.text();
    loadResolver();
  });
  element("load-resolver").addEventListener("click", loadResolver);
  listModifiers();

  element("build").addEventListener("click", () => {
    const { request, diagnostics, message } = buildRequest(source.value, options());
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
    transport.send(exportRequest(options()));
  });

  copy.addEventListener("click", () => {
    // A plugin iframe may have no clipboard permission, so the text is copied through a selection.
    result.select();
    document.execCommand("copy");
    setStatus("Copied.");
  });

  const save = (text: string, name: string, type: string) => {
    const link = document.createElement("a");
    link.href = URL.createObjectURL(new Blob([text], { type }));
    link.download = name;
    link.click();
    URL.revokeObjectURL(link.href);
  };
  download.addEventListener("click", () => save(result.value, fileName, "text/plain"));
  downloadResolver.addEventListener("click", () =>
    save(resolverOut.value, "tokens.resolver.json", "application/json"),
  );

  transport.listen((reply) => {
    if (!isReply(reply)) return;
    if (reply.type === "built") {
      setStatus("Built.");
      // Below the parse's warnings, which describe the same build.
      showNotes(reply.notes ?? [], true);
    } else if (reply.type === "exported") {
      const file = finishExport(reply, options());
      fileName = file.fileName;
      result.value = file.markup;
      copy.disabled = false;
      download.disabled = false;
      // Cleared when the file has one mode, so an earlier export's modes are not offered again.
      resolverOut.value = file.resolver ?? "";
      downloadResolver.disabled = file.resolver === undefined;
      const losses = file.losses.map((l) => `${l.kind} at ${l.path}: ${l.note}`);
      setStatus(losses.length === 0 ? "Exported." : `Exported with ${losses.length} losses.`);
      showNotes([...losses, ...file.diagnostics.map(describe)]);
    } else if (reply.type === "context") {
      showList("context", reply.entries.map(note));
    } else if (reply.type === "error") {
      setStatus(reply.message);
      showNotes((reply.diagnostics ?? []).map(describe));
    }
  });
}
