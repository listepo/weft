// Accessibility snapshot → Weft (SPEC §9, "From a running UI"). The snapshot is untrusted: any
// shape is tolerated, nothing is evaluated, and size, depth and node count are bounded.
import { diagnostic, type Diagnostic } from "@weft/core";
import { parseAriaSnapshot, type AriaNode } from "@weft/render-react";
import { buildDocument, emptyResult, limitReached, MAX_DEPTH, MAX_NODES } from "./build.ts";
import type { ImportOptions, ImportResult, Loss, LossKind, Scalar, Sem } from "./types.ts";

// Large enough for any real screen; the YAML parser is line-based, so this bounds its work.
export const MAX_SNAPSHOT_LENGTH = 2_000_000;

// The ARIA states a Playwright snapshot reports, all of which the importer reads.
const STATES = ["checked", "disabled", "expanded", "invalid", "level", "pressed", "selected"];

const isRecord = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

// Never recoverable from an accessibility snapshot, whatever it holds.
const SNAPSHOT_LOSSES: readonly [LossKind, string][] = [
  ["ids", "an accessibility snapshot carries no ids; every id is generated from the kind and name"],
  ["bindings", "values are the resolved values shown at snapshot time, not bindings"],
  ["actions", "event handlers and their action names are not in the accessibility tree"],
  ["tokens", "design token references are not in the accessibility tree"],
  ["layout", "stack and grid add no accessibility node; their children are imported in place"],
  ["repetition", "repeated content is imported as static siblings, not as <each>"],
  ["slots", "slot membership is not exposed; slot content is imported as default content"],
  ["hidden", "hidden elements, closed dialogs and unselected tab panels are not in the tree"],
  [
    "props",
    "button variant, text and alert tone, placeholder, required, field types other than number and search, column sort, dialog modal and states without an ARIA equivalent are not exposed",
  ],
];

type Budget = { nodes: number; truncated: boolean };

function toSem(node: unknown, depth: number, budget: Budget): Sem | undefined {
  if (!isRecord(node) || typeof node["role"] !== "string") return undefined;
  if (depth > MAX_DEPTH || ++budget.nodes > MAX_NODES) {
    budget.truncated = true;
    return undefined;
  }
  const role = node["role"];
  const name = typeof node["name"] === "string" ? node["name"] : "";
  if (role === "text") return { role, name, states: {}, props: {}, children: [] };
  const states: Record<string, Scalar> = {};
  if (isRecord(node["states"])) {
    for (const key of STATES) {
      const v = node["states"][key];
      if (typeof v === "boolean" || typeof v === "number" || typeof v === "string") states[key] = v;
    }
  }
  const props: Record<string, string> = {};
  if (typeof node["url"] === "string") props["href"] = node["url"];
  const children: Sem[] = [];
  if (Array.isArray(node["children"])) {
    for (const c of node["children"]) {
      const s = toSem(c, depth + 1, budget);
      if (s) children.push(s);
    }
  }
  return { role, name, states, props, children };
}

/** The role tree of a parsed snapshot, cut at the import limits. */
export function snapshotSems(tree: Record<string, unknown>): { sems: Sem[]; truncated: boolean } {
  const budget: Budget = { nodes: 0, truncated: false };
  const top =
    tree["role"] === "fragment" && Array.isArray(tree["children"]) ? tree["children"] : [tree];
  const sems = top.flatMap((c) => toSem(c, 1, budget) ?? []);
  return { sems, truncated: budget.truncated };
}

function readSnapshot(snapshot: unknown, diagnostics: Diagnostic[]): unknown {
  if (typeof snapshot !== "string") return snapshot;
  let text = snapshot;
  if (text.length > MAX_SNAPSHOT_LENGTH) {
    // Cut at a line end so the parser never sees half an entry.
    text = text.slice(0, text.lastIndexOf("\n", MAX_SNAPSHOT_LENGTH) + 1);
    limitReached(diagnostics, "#", `is longer than ${MAX_SNAPSHOT_LENGTH} characters`);
  }
  return parseAriaSnapshot(text);
}

export function fromAriaSnapshot(
  snapshot: AriaNode | string,
  options: ImportOptions,
): ImportResult {
  const diagnostics: Diagnostic[] = [];
  let tree: unknown;
  try {
    tree = readSnapshot(snapshot, diagnostics);
  } catch (error) {
    diagnostics.push(
      diagnostic("W601", {
        path: "#",
        message: "The snapshot is not Playwright aria snapshot YAML.",
        expected: 'lines of the form `- role "name" [state]: text`',
        got: error instanceof Error ? error.message.slice(0, 200) : undefined,
      }),
    );
    return emptyResult(diagnostics);
  }
  if (!isRecord(tree)) {
    diagnostics.push(
      diagnostic("W601", {
        path: "#",
        message: "The snapshot is neither YAML text nor an accessibility tree object.",
        expected: "an AriaNode or Playwright aria snapshot YAML",
      }),
    );
    return emptyResult(diagnostics);
  }
  try {
    const { sems, truncated } = snapshotSems(tree);
    if (truncated) limitReached(diagnostics, "#", "is larger or deeper than the import limit");
    const built = buildDocument(sems, { catalog: options.catalog, diagnostics });
    const losses: Loss[] = SNAPSHOT_LOSSES.map(([kind, note]) => ({
      kind,
      path: built.rootPath,
      note,
    }));
    return {
      document: built.document,
      losses: [...losses, ...built.losses],
      diagnostics: built.diagnostics,
    };
  } catch (error) {
    // Only a malformed catalog can get here; the input itself is handled above.
    diagnostics.push(
      diagnostic("W601", {
        path: "#",
        message: "The snapshot could not be mapped with this catalog.",
        expected: "a catalog with the shape of SPEC §5",
        got: error instanceof Error ? error.message.slice(0, 200) : undefined,
      }),
    );
    return emptyResult(diagnostics);
  }
}
