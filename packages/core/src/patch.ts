// Patches (SPEC §7): an agent edits a document by id instead of rewriting it. The whole list is
// applied to a copy and validated, so a rejected list leaves no trace and the caller's document
// is never touched.
import { canonicalize } from "./canonical.ts";
import {
  diagnostic,
  didYouMean,
  hasErrors,
  oneOf,
  quote,
  type DiagnosticCode,
  type DiagnosticInit,
} from "./diagnostics.ts";
import {
  PatchSchema,
  type Catalog,
  type Child,
  type Diagnostic,
  type Document,
  type Node,
  type Patch,
} from "./model.ts";
import { parseFragment } from "./parse.ts";
import { EACH, NAME, own } from "./rules.ts";
import { validate, type ValidateOptions } from "./validate.ts";

export type ApplyOptions = Omit<ValidateOptions, "source">;

export type PatchResult = {
  /** Canonical result; absent when anything was rejected. Holds no errors, possibly warnings. */
  document?: Document | undefined;
  diagnostics: Diagnostic[];
};

const FORMS: Readonly<Record<string, string>> = {
  set: '{"op":"set","id":"…","prop":"…","value":<literal|{bind}|{token}|null>}',
  insert: '{"op":"insert","parent":"…","slot"?:"…","index"?:0,"markup":"<…/>"}',
  remove: '{"op":"remove","id":"…"}',
  move: '{"op":"move","id":"…","parent":"…","slot"?:"…","index"?:0}',
};

type Hit = { node: Node; list?: Child[] | undefined; index?: number | undefined };

export function applyPatches(
  document: Document,
  patches: unknown,
  options: ApplyOptions,
): PatchResult {
  const read = readPatches(patches);
  if ("diagnostics" in read) return read;

  const work = structuredClone(document);
  for (const [i, patch] of read.patches.entries()) {
    const failure = applyOne(work.root, patch, i, options);
    if (failure.length > 0) return { diagnostics: failure };
  }
  const result = canonicalize(work);
  const diagnostics = validate(result, options);
  return hasErrors(diagnostics) ? { diagnostics } : { document: result, diagnostics };
}

function readPatches(patches: unknown): { patches: Patch[] } | { diagnostics: Diagnostic[] } {
  if (!Array.isArray(patches)) {
    return {
      diagnostics: [
        diagnostic("W501", {
          message: "The patch list must be an array of patches.",
          path: "#/patches",
          expected: "an array such as " + FORMS["remove"],
          got: typeof patches,
        }),
      ],
    };
  }
  const diagnostics: Diagnostic[] = [];
  const out: Patch[] = [];
  for (const [i, item] of patches.entries()) {
    const parsed = PatchSchema.safeParse(item);
    if (parsed.success) {
      out.push(parsed.data);
      continue;
    }
    const op =
      typeof item === "object" && item !== null ? (item as { op?: unknown }).op : undefined;
    const form = typeof op === "string" ? own(FORMS, op) : undefined;
    for (const issue of parsed.error.issues.slice(0, 3)) {
      diagnostics.push(
        diagnostic("W501", {
          message: `Patch ${i} is malformed: ${issue.message}.`,
          path: ["#/patches", i, ...issue.path].join("/"),
          expected: form ?? oneOf(Object.keys(FORMS).map((name) => name)),
          hint:
            form === undefined
              ? '"op" must be "set", "insert", "remove" or "move"'
              : `write the patch as ${form}`,
        }),
      );
    }
  }
  return diagnostics.length > 0 ? { diagnostics } : { patches: out };
}

function applyOne(root: Node, patch: Patch, i: number, options: ApplyOptions): Diagnostic[] {
  const fail = (code: DiagnosticCode, init: DiagnosticInit) => [diagnostic(code, init)];
  const at = (field: string) => `#/patches/${i}/${field}`;

  const findOrFail = (id: string, field: string): Hit | Diagnostic[] => {
    const hit = find(root, id);
    if (hit !== undefined) return hit;
    return fail("W502", {
      message: `No element has the id ${quote(id)}.`,
      path: at(field),
      expected: "the id of an element in the document",
      got: id,
      hint: didYouMean(id, ids(root)) ?? "copy an id from the document",
    });
  };

  if (patch.op === "set") {
    const hit = findOrFail(patch.id, "id");
    if (Array.isArray(hit)) return hit;
    return setProp(hit.node, patch, hit.node === root, at("prop"), options.catalog);
  }

  if (patch.op === "remove") {
    const hit = findOrFail(patch.id, "id");
    if (Array.isArray(hit)) return hit;
    if (hit.list === undefined || hit.index === undefined) return rootFailure("removed", at("id"));
    hit.list.splice(hit.index, 1);
    return [];
  }

  const parent = findOrFail(patch.parent, "parent");
  if (Array.isArray(parent)) return parent;

  let moved: Node[];
  if (patch.op === "move") {
    const hit = findOrFail(patch.id, "id");
    if (Array.isArray(hit)) return hit;
    if (hit.list === undefined || hit.index === undefined) return rootFailure("moved", at("id"));
    if (find(hit.node, patch.parent) !== undefined) {
      return fail("W506", {
        message: `Element ${quote(patch.id)} cannot move into itself or one of its descendants.`,
        path: at("parent"),
        expected: "a parent outside the moved element",
        got: patch.parent,
      });
    }
    // The index counts the target list after the element left it, so a move inside one list
    // reads the same as the resulting order.
    hit.list.splice(hit.index, 1);
    moved = [hit.node];
  } else {
    const fragment = readFragment(patch.markup, root, options, at("markup"));
    if ("diagnostics" in fragment) return fragment.diagnostics;
    moved = fragment.nodes;
  }

  const target = targetList(parent.node, patch.slot, patch.index, options.catalog, at);
  if ("diagnostics" in target) return target.diagnostics;
  target.list.splice(target.index, 0, ...moved);
  return [];
}

function rootFailure(verb: string, path: string): Diagnostic[] {
  return [
    diagnostic("W507", {
      message: `The root element cannot be ${verb}.`,
      path,
      expected: "the id of an element below the root",
      hint: verb === "removed" ? "to replace the screen, send the whole markup instead" : undefined,
    }),
  ];
}

function setProp(
  node: Node,
  patch: Extract<Patch, { op: "set" }>,
  isRoot: boolean,
  path: string,
  catalog: Catalog,
): Diagnostic[] {
  const { prop, value } = patch;
  const reject = (message: string, expected: string, hint?: string) => [
    diagnostic("W503", { message, path, expected, got: prop, hint }),
  ];
  if (prop === "id") {
    return reject(
      "An element's id cannot be changed by a patch.",
      "a prop other than id",
      "remove the element and insert it again with the new id",
    );
  }
  if (isRoot && prop === "weft") {
    return reject("The format version cannot be changed by a patch.", "a prop other than weft");
  }
  const event = prop.startsWith("on-") ? prop.slice(3) : undefined;
  if (!NAME.test(event ?? prop)) {
    return reject(
      `${quote(prop)} is not a valid prop name.`,
      "[a-z][a-z0-9]*(-[a-z0-9]+)*",
      'event bindings are written "on-<event>", for example "on-press"',
    );
  }
  if (event !== undefined) {
    if (value !== null && typeof value !== "string") {
      return reject(
        "An event binding is set to an action name.",
        "an action name string, or null to remove the binding",
      );
    }
    node.on = update(node.on, event, value);
    return [];
  }
  if (prop === "text" && holdsTextContent(node, catalog)) {
    // SPEC §7: text has two spellings, and `set` writes the one already in use. Content can only
    // hold a literal, so any other value (or null) replaces the content with the prop.
    if (typeof value === "string") {
      node.children = [value];
      return [];
    }
    delete node.children;
  }
  node.props = update(node.props, prop, value);
  return [];
}

/** A text-bearing component (SPEC §5.1) whose default content is text and nothing else. */
function holdsTextContent(node: Node, catalog: Catalog): boolean {
  const content = own(catalog.components, node.kind)?.content;
  const children = node.children ?? [];
  return (
    (content === "text" || content === "mixed") &&
    children.length > 0 &&
    children.every((c) => typeof c === "string")
  );
}

/** `null` removes the key; the keys were checked against the name grammar, so none is `__proto__`. */
function update<T>(record: Record<string, T> | undefined, key: string, value: T | null) {
  const next = { ...record };
  if (value === null) delete next[key];
  else next[key] = value;
  return next;
}

function readFragment(
  markup: string,
  root: Node,
  options: ApplyOptions,
  path: string,
): { nodes: Node[] } | { diagnostics: Diagnostic[] } {
  const { wrapper, diagnostics } = parseFragment(markup, options.catalog);
  if (wrapper === undefined) {
    return { diagnostics: diagnostics.map((d) => ({ ...d, path: `${path}${d.path}` })) };
  }
  const children = wrapper.children ?? [];
  const nodes = children.filter((c): c is Node => typeof c !== "string");
  if (nodes.length === 0 || nodes.length !== children.length || wrapper.slots !== undefined) {
    return {
      diagnostics: [
        diagnostic("W508", {
          message: "Inserted markup must be one or more elements and nothing else.",
          path,
          expected: 'elements such as <button id="…">Text</button>, without loose text or <slot>',
          got: nodes.length === 0 ? "no elements" : "text or <slot> next to elements",
        }),
      ],
    };
  }

  const taken = ids(root);
  const inserted = new Set<string>();
  for (const node of nodes) for (const id of ids(node)) inserted.add(id);
  const clashes = [...inserted].filter((id) => taken.has(id));
  if (clashes.length > 0) {
    return {
      diagnostics: clashes.map((id) =>
        diagnostic("W509", {
          message: `Id ${quote(id)} is already used in the document.`,
          path,
          expected: "ids that no element of the document has",
          got: id,
          hint: `use ${quote(freeId(id, new Set([...taken, ...inserted])))}`,
        }),
      ),
    };
  }
  return { nodes };
}

function targetList(
  parent: Node,
  slot: string | undefined,
  index: number | undefined,
  catalog: Catalog,
  at: (field: string) => string,
): { list: Child[]; index: number } | { diagnostics: Diagnostic[] } {
  let list: Child[];
  if (slot === undefined) {
    list = parent.children ??= [];
  } else {
    const declared = declaredSlots(parent, catalog);
    if (!NAME.test(slot) || (declared !== undefined && !declared.includes(slot))) {
      return {
        diagnostics: [
          diagnostic("W504", {
            message: `<${parent.kind}> does not declare a slot ${quote(slot)}.`,
            path: at("slot"),
            expected:
              declared === undefined || declared.length === 0
                ? "no slot (omit `slot` to use the default slot)"
                : oneOf(declared),
            got: slot,
            hint: didYouMean(slot, declared ?? []) ?? "omit `slot` to use the default slot",
          }),
        ],
      };
    }
    const slots = (parent.slots ??= {});
    list = slots[slot] ??= [];
  }
  const at0 = index ?? list.length;
  if (at0 > list.length) {
    return {
      diagnostics: [
        diagnostic("W505", {
          message: `Index ${at0} is past the end of a list with ${list.length} entries.`,
          path: at("index"),
          expected: `an integer from 0 to ${list.length} (text counts as an entry)`,
          got: String(at0),
          hint: "omit `index` to append",
        }),
      ],
    };
  }
  return { list, index: at0 };
}

/** `undefined` means any slot name: extension and unknown elements declare nothing. */
function declaredSlots(node: Node, catalog: Catalog): string[] | undefined {
  if (node.kind === EACH) return [];
  const component = own(catalog.components, node.kind);
  return component === undefined ? undefined : Object.keys(component.slots ?? {});
}

function childLists(node: Node): Child[][] {
  return [node.children ?? [], ...Object.values(node.slots ?? {})];
}

/** Document order, the root included; the first element with the id wins. */
function find(top: Node, id: string): Hit | undefined {
  if (top.id === id) return { node: top };
  for (const list of childLists(top)) {
    for (const [index, child] of list.entries()) {
      if (typeof child === "string") continue;
      if (child.id === id) return { node: child, list, index };
      const deeper = find(child, id);
      if (deeper !== undefined) return deeper;
    }
  }
  return undefined;
}

function ids(top: Node, into = new Set<string>()): Set<string> {
  if (top.id !== undefined) into.add(top.id);
  for (const list of childLists(top)) {
    for (const child of list) if (typeof child !== "string") ids(child, into);
  }
  return into;
}

function freeId(id: string, taken: ReadonlySet<string>): string {
  let n = 2;
  while (taken.has(`${id}-${n}`)) n++;
  return `${id}-${n}`;
}
