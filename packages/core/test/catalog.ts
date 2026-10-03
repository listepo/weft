// A subset of SPEC §5.1 for the core tests, so that they do not depend on packages/catalog.
import { CatalogSchema, type Catalog } from "../src/model.ts";

const d = "Fixture.";

export const catalog: Catalog = CatalogSchema.parse({
  weft: "0.1",
  name: "fixture",
  version: "0.1.0",
  components: {
    screen: {
      description: d,
      role: "main",
      content: "nodes",
      allowedParents: [],
      props: { weft: { description: d, type: "string", required: true, bindable: false } },
      states: ["ready", "loading", "error"],
    },
    stack: {
      description: d,
      role: "none",
      content: "nodes",
      props: {
        direction: { description: d, type: "enum", values: ["column", "row"], default: "column" },
        gap: { description: d, type: "token", tokenType: "dimension" },
        align: { description: d, type: "enum", values: ["start", "center", "end", "stretch"] },
        wrap: { description: d, type: "boolean" },
      },
    },
    heading: {
      description: d,
      role: "heading",
      content: "text",
      props: {
        level: { description: d, type: "number", required: true, integer: true, min: 1, max: 6 },
        text: { description: d, type: "string" },
      },
    },
    text: {
      description: d,
      role: "none",
      content: "text",
      props: {
        text: { description: d, type: "string" },
        tone: {
          description: d,
          type: "enum",
          values: ["default", "muted", "success", "warning", "danger"],
        },
      },
    },
    link: {
      description: d,
      role: "link",
      content: "text",
      props: { href: { description: d, type: "string" }, text: { description: d, type: "string" } },
      events: ["press"],
    },
    button: {
      description: d,
      role: "button",
      content: "text",
      props: {
        variant: {
          description: d,
          type: "enum",
          values: ["primary", "secondary", "danger"],
          default: "secondary",
        },
        disabled: { description: d, type: "boolean" },
        submit: { description: d, type: "boolean", default: false, bindable: false },
      },
      states: ["idle", "busy"],
      events: ["press"],
    },
    form: {
      description: d,
      role: "form",
      content: "nodes",
      slots: { footer: { description: d } },
      states: ["idle", "submitting", "invalid"],
      events: ["submit"],
    },
    field: {
      description: d,
      role: "textbox",
      content: "none",
      props: {
        label: { description: d, type: "string", required: true },
        type: {
          description: d,
          type: "enum",
          values: ["text", "email", "password", "number", "search", "multiline"],
          default: "text",
        },
        value: { description: d, type: "string", writable: true },
        placeholder: { description: d, type: "string" },
        required: { description: d, type: "boolean" },
        disabled: { description: d, type: "boolean" },
        error: { description: d, type: "string" },
      },
      states: ["valid", "invalid"],
      events: ["change"],
    },
    select: {
      description: d,
      role: "combobox",
      content: "nodes",
      allowedChildren: ["option", "each"],
      props: {
        label: { description: d, type: "string", required: true },
        value: { description: d, type: "string", writable: true },
        disabled: { description: d, type: "boolean" },
      },
      events: ["change"],
    },
    option: {
      description: d,
      role: "option",
      content: "text",
      allowedParents: ["select"],
      props: { value: { description: d, type: "string", required: true } },
    },
    list: {
      description: d,
      role: "list",
      content: "nodes",
      allowedChildren: ["item", "each"],
      props: { ordered: { description: d, type: "boolean" } },
      states: ["ready", "loading", "empty"],
    },
    item: {
      description: d,
      role: "listitem",
      content: "mixed",
      allowedParents: ["list"],
      events: ["press"],
    },
    tabs: {
      description: d,
      role: "tablist",
      content: "nodes",
      allowedChildren: ["tab"],
      props: { selected: { description: d, type: "string", writable: true } },
      events: ["change"],
    },
    tab: {
      description: d,
      role: "tab",
      content: "nodes",
      allowedParents: ["tabs"],
      props: { label: { description: d, type: "string", required: true } },
    },
    dialog: {
      description: d,
      role: "dialog",
      content: "nodes",
      requiresLabel: true,
      props: {
        modal: { description: d, type: "boolean", default: true },
        open: { description: d, type: "boolean", writable: true },
      },
      slots: { actions: { description: d, allowedChildren: ["button"], required: true } },
      events: ["close"],
    },
  },
});

export const tokens: ReadonlyMap<string, string> = new Map([
  ["space.sm", "dimension"],
  ["space.md", "dimension"],
  ["color.accent", "color"],
]);
