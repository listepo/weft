// Classifies the difference between two catalog versions by the SPEC §8 rule, so that a catalog
// author learns whether the next version is a major, minor or no-op bump.
import { isDeepStrictEqual } from "node:util";
import type { Catalog, ComponentDef, PropDef, SlotDef } from "@weft/core";

export type ChangeLevel = "none" | "minor" | "major";
export type CatalogChange = { path: string; level: ChangeLevel; message: string };
export type CatalogDiff = { level: ChangeLevel; changes: CatalogChange[] };

const RANK: Record<ChangeLevel, number> = { none: 0, minor: 1, major: 2 };
const KNOWN_PROP_FIELDS = new Set([
  "description",
  "type",
  "values",
  "tokenType",
  "required",
  "default",
  "bindable",
  "writable",
  "min",
  "max",
]);

type Out = CatalogChange[];
const record = (out: Out, path: string, level: ChangeLevel, message: string) =>
  out.push({ path, level, message });

const show = (value: unknown) => JSON.stringify(value) ?? "absent";

// Which kinds of content a content model accepts; a model is wider when it accepts a superset.
const ACCEPTS: Record<ComponentDef["content"], readonly string[]> = {
  none: [],
  text: ["text"],
  nodes: ["nodes"],
  mixed: ["text", "nodes"],
};

/** Removing a name is major, adding one is minor. */
function diffNames(
  out: Out,
  path: string,
  noun: string,
  previous: readonly string[] | undefined,
  next: readonly string[] | undefined,
) {
  for (const name of previous ?? [])
    if (!next?.includes(name)) record(out, path, "major", `${noun} "${name}" was removed.`);
  for (const name of next ?? [])
    if (!previous?.includes(name)) record(out, path, "minor", `${noun} "${name}" was added.`);
}

/** Absent means "any", so a list narrows what absent allowed and widens what a list allowed. */
function diffRestriction(
  out: Out,
  path: string,
  noun: string,
  previous: readonly string[] | undefined,
  next: readonly string[] | undefined,
) {
  if (previous === undefined && next === undefined) return;
  if (previous === undefined && next !== undefined)
    return record(out, path, "major", `${noun} was restricted to ${show(next)}.`);
  if (next === undefined) return record(out, path, "minor", `${noun} restriction was lifted.`);
  for (const name of previous ?? [])
    if (!next.includes(name)) record(out, path, "major", `${noun} no longer allows "${name}".`);
  for (const name of next)
    if (!previous?.includes(name)) record(out, path, "minor", `${noun} now allows "${name}".`);
}

function diffBound(out: Out, path: string, field: "min" | "max", previous: unknown, next: unknown) {
  if (previous === next) return;
  const message = `${field} changed from ${show(previous)} to ${show(next)}.`;
  if (typeof previous !== "number" || typeof next !== "number") {
    // A bound appearing narrows the accepted range and one disappearing widens it.
    return record(out, path, next === undefined ? "minor" : "major", message);
  }
  const narrower = field === "min" ? next > previous : next < previous;
  record(out, path, narrower ? "major" : "minor", message);
}

function diffProp(out: Out, path: string, previous: PropDef, next: PropDef) {
  const a: Record<string, unknown> = previous;
  const b: Record<string, unknown> = next;
  if (previous.description !== next.description)
    record(out, `${path}.description`, "none", "The description changed.");
  if (previous.type !== next.type)
    record(out, `${path}.type`, "major", `type changed from ${previous.type} to ${next.type}.`);
  diffNames(out, `${path}.values`, "Enum value", previous.values, next.values);
  if (previous.tokenType !== next.tokenType)
    record(
      out,
      `${path}.tokenType`,
      "major",
      `tokenType changed from ${show(previous.tokenType)} to ${show(next.tokenType)}.`,
    );
  if ((previous.required ?? false) !== (next.required ?? false)) {
    const nowRequired = next.required === true;
    record(
      out,
      `${path}.required`,
      nowRequired ? "major" : "minor",
      nowRequired ? "The prop became required." : "The prop is no longer required.",
    );
  }
  if (!isDeepStrictEqual(previous.default, next.default))
    record(
      out,
      `${path}.default`,
      "major",
      `default changed from ${show(previous.default)} to ${show(next.default)}.`,
    );
  // `bindable` defaults to true and `writable` to false: only the move away from the default
  // that takes a capability away is breaking.
  if ((previous.bindable ?? true) !== (next.bindable ?? true))
    record(
      out,
      `${path}.bindable`,
      next.bindable === false ? "major" : "minor",
      next.bindable === false
        ? "The prop no longer accepts bindings."
        : "The prop accepts bindings.",
    );
  if ((previous.writable ?? false) !== (next.writable ?? false))
    record(
      out,
      `${path}.writable`,
      next.writable === true ? "minor" : "major",
      next.writable === true
        ? "The prop became a two-way target."
        : "The prop is no longer a two-way target.",
    );
  diffBound(out, `${path}.min`, "min", a["min"], b["min"]);
  diffBound(out, `${path}.max`, "max", a["max"], b["max"]);
  // A field this classifier does not know yet may constrain documents, so it is breaking until
  // the rule for it is written down.
  for (const field of new Set([...Object.keys(a), ...Object.keys(b)]))
    if (!KNOWN_PROP_FIELDS.has(field) && !isDeepStrictEqual(a[field], b[field]))
      record(
        out,
        `${path}.${field}`,
        "major",
        `${field} changed from ${show(a[field])} to ${show(b[field])}.`,
      );
}

function diffSlot(out: Out, path: string, previous: SlotDef, next: SlotDef) {
  if (previous.description !== next.description)
    record(out, `${path}.description`, "none", "The description changed.");
  diffRestriction(
    out,
    `${path}.allowedChildren`,
    "Slot content",
    previous.allowedChildren,
    next.allowedChildren,
  );
  if ((previous.required ?? false) !== (next.required ?? false))
    record(
      out,
      `${path}.required`,
      next.required === true ? "major" : "minor",
      next.required === true ? "The slot became required." : "The slot is no longer required.",
    );
}

/** Names on one side only are removed (major) or added (`added` decides); shared names go to `both`. */
function diffRecord<T>(
  out: Out,
  path: string,
  noun: string,
  previous: Record<string, T> | undefined,
  next: Record<string, T> | undefined,
  both: (path: string, previous: T, next: T) => void,
  added: (value: T) => ChangeLevel = () => "minor",
) {
  for (const name of Object.keys(previous ?? {}))
    if (next === undefined || !Object.hasOwn(next, name))
      record(out, `${path}.${name}`, "major", `${noun} "${name}" was removed.`);
  for (const [name, value] of Object.entries(next ?? {})) {
    const before =
      previous !== undefined && Object.hasOwn(previous, name) ? previous[name] : undefined;
    if (before === undefined)
      record(out, `${path}.${name}`, added(value), `${noun} "${name}" was added.`);
    else both(`${path}.${name}`, before, value);
  }
}

function diffComponent(out: Out, path: string, previous: ComponentDef, next: ComponentDef) {
  if (previous.description !== next.description)
    record(out, `${path}.description`, "none", "The description changed.");
  if (previous.role !== next.role)
    record(out, `${path}.role`, "major", `role changed from "${previous.role}" to "${next.role}".`);
  if (previous.content !== next.content) {
    const before = ACCEPTS[previous.content];
    const wider = before.every((kind) => ACCEPTS[next.content].includes(kind));
    record(
      out,
      `${path}.content`,
      wider ? "minor" : "major",
      `content changed from "${previous.content}" to "${next.content}", ${wider ? "widening" : "narrowing"} the content model.`,
    );
  }
  diffRestriction(
    out,
    `${path}.allowedChildren`,
    "Child kinds",
    previous.allowedChildren,
    next.allowedChildren,
  );
  diffRestriction(
    out,
    `${path}.allowedParents`,
    "Parent kinds",
    previous.allowedParents,
    next.allowedParents,
  );
  if ((previous.requiresLabel ?? false) !== (next.requiresLabel ?? false))
    record(
      out,
      `${path}.requiresLabel`,
      next.requiresLabel === true ? "major" : "minor",
      next.requiresLabel === true ? "A label became required." : "A label is no longer required.",
    );
  diffRecord(
    out,
    `${path}.props`,
    "Prop",
    previous.props,
    next.props,
    (p, a, b) => diffProp(out, p, a, b),
    // Existing documents lack a prop that is newly required.
    (prop) => (prop.required === true ? "major" : "minor"),
  );
  diffRecord(
    out,
    `${path}.slots`,
    "Slot",
    previous.slots,
    next.slots,
    (p, a, b) => diffSlot(out, p, a, b),
    (slot) => (slot.required === true ? "major" : "minor"),
  );
  diffNames(out, `${path}.states`, "State", previous.states, next.states);
  diffNames(out, `${path}.events`, "Event", previous.events, next.events);
}

export function diffCatalogs(previous: Catalog, next: Catalog): CatalogDiff {
  const changes: Out = [];
  diffRecord(
    changes,
    "components",
    "Component",
    previous.components,
    next.components,
    (path, a, b) => diffComponent(changes, path, a, b),
  );
  const level = changes.reduce<ChangeLevel>(
    (top, change) => (RANK[change.level] > RANK[top] ? change.level : top),
    "none",
  );
  return { level, changes };
}
