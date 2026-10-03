// Markup → canonical JSON (SPEC §2–§4): the tokenizer checks syntax, the builder types literals
// by the catalog and lifts `<slot>` into `slots`, and validation adds the schema and semantic layers.
import { appendText, assembleNode, normalizeText } from "./canonical.ts";
import { byPosition, diagnostic, hasErrors, type Position } from "./diagnostics.ts";
import type { Catalog, Child, Diagnostic, Document, Node, PropDef, Value } from "./model.ts";
import { EACH, NAME, own, SLOT, UNIVERSAL_PROPS } from "./rules.ts";
import { pathSegment, type ListSource, type NodeSource, type SourceMap } from "./source.ts";
import { tokenize, type RawElement } from "./syntax.ts";
import { validate, type ValidateOptions } from "./validate.ts";
import { readValue } from "./values.ts";

/** Without a catalog only the syntax layer runs and every literal stays a string. */
export type ParseOptions = Omit<ValidateOptions, "source" | "catalog"> & {
  catalog?: Catalog | undefined;
};

export type ParseResult = {
  /** Absent when the markup has syntax errors. */
  document?: Document | undefined;
  diagnostics: Diagnostic[];
  source?: SourceMap | undefined;
};

export function parse(markup: string, options: ParseOptions = {}): ParseResult {
  const syntax = tokenize(markup);
  if (syntax.root === undefined || hasErrors(syntax.diagnostics))
    return { diagnostics: syntax.diagnostics };
  const built = build(syntax.root, options.catalog);
  if (hasErrors(built.diagnostics)) return { diagnostics: built.diagnostics.toSorted(byPosition) };
  const { catalog } = options;
  if (catalog === undefined)
    return { document: built.document, diagnostics: [], source: built.source };
  const diagnostics = validate(built.document, { ...options, catalog, source: built.source });
  return { document: built.document, diagnostics, source: built.source };
}

const FRAGMENT = "x-weft-fragment";

/**
 * Parses markup that has no `<screen>` root (several sibling elements, for patch `insert`) by
 * wrapping it in a throwaway element, so the tokenizer, literal typing and slot handling stay the
 * single implementation. Returns the wrapper: its `children` are the fragment. The wrapper is
 * hidden from diagnostics: its path segment is dropped and line 1 columns are shifted back.
 */
export function parseFragment(
  markup: string,
  catalog: Catalog,
): { wrapper?: Node | undefined; diagnostics: Diagnostic[] } {
  const open = `<${FRAGMENT}>`;
  const syntax = tokenize(`${open}${markup}</${FRAGMENT}>`);
  const built =
    syntax.root === undefined || hasErrors(syntax.diagnostics)
      ? undefined
      : build(syntax.root, catalog);
  const raw = built === undefined ? syntax.diagnostics : built.diagnostics;
  const diagnostics = raw.toSorted(byPosition).map((d): Diagnostic => {
    const path = d.path.startsWith(`/${FRAGMENT}`) ? d.path.slice(FRAGMENT.length + 1) : d.path;
    const shifted =
      d.line === 1 && d.column !== undefined ? { column: d.column - open.length } : {};
    return { ...d, ...shifted, path: path === "" ? "/" : path };
  });
  if (built === undefined || hasErrors(diagnostics)) return { diagnostics };
  return { wrapper: built.document.root, diagnostics };
}

function propType(
  component: Catalog["components"][string] | undefined,
  name: string,
): PropDef["type"] | undefined {
  // Extension and unknown elements keep every literal a string (SPEC §3).
  if (component === undefined) return undefined;
  return (own(component.props, name) ?? own(UNIVERSAL_PROPS, name))?.type;
}

function build(rawRoot: RawElement, catalog: Catalog | undefined) {
  const diagnostics: Diagnostic[] = [];
  const source: SourceMap = new WeakMap();
  let weft = "";

  const element = (raw: RawElement, path: string, isRoot: boolean): Node => {
    const kind = raw.name;
    const component = kind === EACH ? undefined : own(catalog?.components, kind);
    let id: string | undefined;
    const props: [string, Value][] = [];
    const on: [string, string][] = [];
    const attrs = new Map<string, Position>();
    for (const attr of raw.attrs) {
      attrs.set(attr.name, attr.pos);
      if (attr.name === "id") id = attr.value;
      else if (attr.name.startsWith("on-")) on.push([attr.name.slice(3), attr.value]);
      else if (isRoot && attr.name === "weft") weft = attr.value;
      else {
        const read = readValue(attr.value, propType(component, attr.name));
        if (read.ok) props.push([attr.name, read.value]);
        else {
          diagnostics.push(
            diagnostic("W116", {
              message: read.message,
              expected: "{$…}, {!$…}, {token.…} or a literal starting with {{",
              path: `${path}/@${attr.name}`,
              pos: attr.pos,
              got: attr.value,
              hint: read.hint,
            }),
          );
        }
      }
    }

    const children: Child[] = [];
    const childPositions: (Position | undefined)[] = [];
    const slots = new Map<string, { list: Child[]; source: ListSource }>();
    for (const child of raw.children) {
      if (child.type === "text") {
        if (appendText(children, normalizeText(child.text))) childPositions.push(child.pos);
        continue;
      }
      if (child.name !== SLOT) {
        children.push(element(child, `${path}/${segment(child)}`, false));
        childPositions.push(child.pos);
        continue;
      }
      const slot = readSlot(child, raw, path);
      if (slot === undefined) continue;
      const list: Child[] = [];
      const positions: (Position | undefined)[] = [];
      const slotPath = `${path}/slot[${slot}]`;
      for (const grandchild of child.children) {
        if (grandchild.type === "text") {
          if (appendText(list, normalizeText(grandchild.text))) positions.push(grandchild.pos);
        } else if (grandchild.name === SLOT) {
          misplacedSlot(
            grandchild,
            slotPath,
            "A <slot> cannot be placed directly inside another <slot>.",
          );
        } else {
          list.push(element(grandchild, `${slotPath}/${segment(grandchild)}`, false));
          positions.push(grandchild.pos);
        }
      }
      slots.set(slot, { list, source: { pos: child.pos, children: positions } });
    }

    const node = assembleNode({
      kind,
      id,
      props,
      on,
      slots: [...slots].map(([name, s]) => [name, s.list] as const),
      children,
    });
    const nodeSource: NodeSource = {
      pos: raw.pos,
      attrs,
      children: childPositions,
      slots: new Map([...slots].map(([name, s]) => [name, s.source])),
    };
    source.set(node, nodeSource);
    return node;
  };

  const misplacedSlot = (raw: RawElement, path: string, message: string) => {
    diagnostics.push(
      diagnostic("W118", {
        message,
        expected: "a <slot> directly inside a component",
        path: `${path}/slot`,
        pos: raw.pos,
        hint: "make the <slot> a direct child of the component that declares it",
      }),
    );
  };

  const seenSlots = new WeakMap<RawElement, Set<string>>();
  const readSlot = (
    raw: RawElement,
    parent: RawElement,
    parentPath: string,
  ): string | undefined => {
    if (parent.name === EACH) {
      misplacedSlot(raw, parentPath, "A <slot> cannot be placed directly inside <each>.");
      return undefined;
    }
    const name = raw.attrs.find((a) => a.name === "name")?.value;
    const extra = raw.attrs.find((a) => a.name !== "name");
    if (name === undefined || !NAME.test(name) || extra !== undefined) {
      diagnostics.push(
        diagnostic("W118", {
          message:
            extra === undefined
              ? "A <slot> needs a `name` that matches the name grammar."
              : `A <slot> takes only \`name\`, not "${extra.name}".`,
          path: `${parentPath}/slot`,
          pos: extra?.pos ?? raw.pos,
          expected: '<slot name="…">',
          got: name,
        }),
      );
      return undefined;
    }
    const seen = seenSlots.get(parent) ?? new Set<string>();
    seenSlots.set(parent, seen);
    if (seen.has(name)) {
      diagnostics.push(
        diagnostic("W119", {
          message: `Slot "${name}" appears twice under one parent.`,
          expected: "each slot name once per parent",
          path: `${parentPath}/slot[${name}]`,
          pos: raw.pos,
          hint: `merge both <slot name="${name}"> elements into one`,
        }),
      );
      return undefined;
    }
    seen.add(name);
    return name;
  };

  let root: Node;
  if (rawRoot.name === SLOT) {
    misplacedSlot(rawRoot, "", "The root element cannot be a <slot>.");
    root = { kind: SLOT };
  } else {
    root = element(rawRoot, `/${segment(rawRoot)}`, true);
  }
  const document: Document = { weft, root };
  return { document, diagnostics, source };
}

function segment(raw: RawElement): string {
  return pathSegment(raw.name, raw.attrs.find((a) => a.name === "id")?.value);
}
