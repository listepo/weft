// The diffCatalogs case table, shared by diff.test.ts and the TypeScript–Rust differential fixture.
import type { Catalog, ComponentDef, PropDef } from "@weft/core";

const prop = (extra: Partial<PropDef> = {}): PropDef => ({
  description: "A prop.",
  type: "string",
  ...extra,
});

export const base: Catalog = {
  weft: "0.1",
  name: "t",
  version: "1.0.0",
  components: {
    list: {
      description: "A list.",
      role: "list",
      content: "nodes",
      allowedChildren: ["item", "other"],
      slots: { header: { description: "Header.", allowedChildren: ["item"] } },
    },
    item: { description: "An item.", role: "listitem", content: "mixed", allowedParents: ["list"] },
    button: {
      description: "A button.",
      role: "button",
      content: "text",
      requiresLabel: false,
      props: {
        variant: prop({ type: "enum", values: ["primary", "danger"], default: "primary" }),
        count: Object.assign(prop({ type: "number", required: true }), { min: 1, max: 10 }),
        title: prop(),
      },
      slots: { icon: { description: "Icon." } },
      states: ["idle", "busy"],
      events: ["press"],
    },
  },
};

export const edit = (change: (components: Record<string, ComponentDef>) => void): Catalog => {
  const next = structuredClone(base);
  change(next.components);
  return next;
};
// Fields the typed model may not declare yet are reached through a loose view.
const loose = (value: unknown) => value as Record<string, unknown>;

export type Row = [name: string, next: Catalog, level: string, paths: string[]];

export const rows: Row[] = [
  ["identical", structuredClone(base), "none", []],
  ["component removed", edit((c) => delete c["item"]), "major", ["components.item"]],
  [
    "component added",
    edit((c) => (c["menu"] = { description: "Menu.", role: "menu", content: "nodes" })),
    "minor",
    ["components.menu"],
  ],
  [
    "description only",
    edit((c) => (c["button"]!.description = "Reworded.")),
    "none",
    ["components.button.description"],
  ],
  [
    "prop description only",
    edit((c) => (c["button"]!.props!["title"]!.description = "Reworded.")),
    "none",
    ["components.button.props.title.description"],
  ],
  [
    "prop removed",
    edit((c) => delete c["button"]!.props!["title"]),
    "major",
    ["components.button.props.title"],
  ],
  [
    "optional prop added",
    edit((c) => (c["button"]!.props!["tone"] = prop())),
    "minor",
    ["components.button.props.tone"],
  ],
  [
    "required prop added",
    edit((c) => (c["button"]!.props!["tone"] = prop({ required: true }))),
    "major",
    ["components.button.props.tone"],
  ],
  [
    "prop becomes required",
    edit((c) => (c["button"]!.props!["title"]!.required = true)),
    "major",
    ["components.button.props.title.required"],
  ],
  [
    "prop stops being required",
    edit((c) => delete c["button"]!.props!["count"]!.required),
    "minor",
    ["components.button.props.count.required"],
  ],
  [
    "enum value removed",
    edit((c) => (c["button"]!.props!["variant"]!.values = ["primary"])),
    "major",
    ["components.button.props.variant.values"],
  ],
  [
    "enum value added",
    edit((c) => c["button"]!.props!["variant"]!.values!.push("quiet")),
    "minor",
    ["components.button.props.variant.values"],
  ],
  [
    "prop type changed",
    edit((c) => (c["button"]!.props!["title"]!.type = "number")),
    "major",
    ["components.button.props.title.type"],
  ],
  [
    "default changed",
    edit((c) => (c["button"]!.props!["variant"]!.default = "danger")),
    "major",
    ["components.button.props.variant.default"],
  ],
  [
    "default removed",
    edit((c) => delete c["button"]!.props!["variant"]!.default),
    "major",
    ["components.button.props.variant.default"],
  ],
  [
    "bindable turned off",
    edit((c) => (c["button"]!.props!["title"]!.bindable = false)),
    "major",
    ["components.button.props.title.bindable"],
  ],
  [
    "writable turned on",
    edit((c) => (c["button"]!.props!["title"]!.writable = true)),
    "minor",
    ["components.button.props.title.writable"],
  ],
  [
    "range narrowed (min raised)",
    edit((c) => (loose(c["button"]!.props!["count"])["min"] = 2)),
    "major",
    ["components.button.props.count.min"],
  ],
  [
    "range narrowed (max lowered)",
    edit((c) => (loose(c["button"]!.props!["count"])["max"] = 5)),
    "major",
    ["components.button.props.count.max"],
  ],
  [
    "range widened",
    edit((c) => {
      loose(c["button"]!.props!["count"])["min"] = 0;
      loose(c["button"]!.props!["count"])["max"] = 20;
    }),
    "minor",
    ["components.button.props.count.min", "components.button.props.count.max"],
  ],
  [
    "range bound removed",
    edit((c) => delete loose(c["button"]!.props!["count"])["max"]),
    "minor",
    ["components.button.props.count.max"],
  ],
  [
    "unknown prop field changed",
    edit((c) => (loose(c["button"]!.props!["title"])["pattern"] = "^a")),
    "major",
    ["components.button.props.title.pattern"],
  ],
  ["role changed", edit((c) => (c["button"]!.role = "link")), "major", ["components.button.role"]],
  [
    "content narrowed",
    edit((c) => (c["item"]!.content = "text")),
    "major",
    ["components.item.content"],
  ],
  [
    "content widened",
    edit((c) => (c["button"]!.content = "mixed")),
    "minor",
    ["components.button.content"],
  ],
  [
    "content changed sideways",
    edit((c) => (c["button"]!.content = "nodes")),
    "major",
    ["components.button.content"],
  ],
  [
    "allowedChildren narrowed",
    edit((c) => (c["list"]!.allowedChildren = ["item"])),
    "major",
    ["components.list.allowedChildren"],
  ],
  [
    "allowedChildren widened",
    edit((c) => c["list"]!.allowedChildren!.push("third")),
    "minor",
    ["components.list.allowedChildren"],
  ],
  [
    "allowedChildren introduced",
    edit((c) => (c["button"]!.allowedChildren = ["item"])),
    "major",
    ["components.button.allowedChildren"],
  ],
  [
    "allowedChildren lifted",
    edit((c) => delete c["list"]!.allowedChildren),
    "minor",
    ["components.list.allowedChildren"],
  ],
  [
    "allowedParents narrowed",
    edit((c) => (c["item"]!.allowedParents = ["menu"])),
    "major",
    ["components.item.allowedParents", "components.item.allowedParents"],
  ],
  [
    "allowedParents widened",
    edit((c) => c["item"]!.allowedParents!.push("menu")),
    "minor",
    ["components.item.allowedParents"],
  ],
  [
    "allowedParents lifted",
    edit((c) => delete c["item"]!.allowedParents),
    "minor",
    ["components.item.allowedParents"],
  ],
  [
    "requiresLabel turned on",
    edit((c) => (c["button"]!.requiresLabel = true)),
    "major",
    ["components.button.requiresLabel"],
  ],
  [
    "slot removed",
    edit((c) => delete c["button"]!.slots!["icon"]),
    "major",
    ["components.button.slots.icon"],
  ],
  [
    "slot added",
    edit((c) => (c["button"]!.slots!["badge"] = { description: "Badge." })),
    "minor",
    ["components.button.slots.badge"],
  ],
  [
    "required slot added",
    edit((c) => (c["button"]!.slots!["badge"] = { description: "Badge.", required: true })),
    "major",
    ["components.button.slots.badge"],
  ],
  [
    "slot becomes required",
    edit((c) => (c["button"]!.slots!["icon"]!.required = true)),
    "major",
    ["components.button.slots.icon.required"],
  ],
  [
    "slot allowedChildren narrowed",
    edit((c) => (c["list"]!.slots!["header"]!.allowedChildren = [])),
    "major",
    ["components.list.slots.header.allowedChildren"],
  ],
  [
    "state removed",
    edit((c) => (c["button"]!.states = ["idle"])),
    "major",
    ["components.button.states"],
  ],
  [
    "state added",
    edit((c) => c["button"]!.states!.push("done")),
    "minor",
    ["components.button.states"],
  ],
  ["event removed", edit((c) => delete c["button"]!.events), "major", ["components.button.events"]],
  [
    "event added",
    edit((c) => c["button"]!.events!.push("hover")),
    "minor",
    ["components.button.events"],
  ],
  [
    "worst change wins",
    edit((c) => {
      c["button"]!.events!.push("hover");
      c["button"]!.description = "Reworded.";
      delete c["button"]!.props!["title"];
    }),
    "major",
    ["components.button.description", "components.button.props.title", "components.button.events"],
  ],
];
