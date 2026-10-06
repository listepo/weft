// The `weft-core` 0.1 catalog of SPEC §5.1 as data. `catalog.json` is generated from this object.
import type { Catalog, ComponentDef, PropDef, SlotDef } from "@weft/core";

const str = (description: string, extra: Partial<PropDef> = {}): PropDef => ({
  description,
  type: "string",
  ...extra,
});
const bool = (description: string, extra: Partial<PropDef> = {}): PropDef => ({
  description,
  type: "boolean",
  ...extra,
});
const num = (description: string, extra: Partial<PropDef> = {}): PropDef => ({
  description,
  type: "number",
  ...extra,
});
const oneOf = (description: string, values: string[], extra: Partial<PropDef> = {}): PropDef => ({
  description,
  type: "enum",
  values,
  ...extra,
});
const dimension = (description: string): PropDef => ({
  description,
  type: "token",
  tokenType: "dimension",
});

const material = (description: string): PropDef => ({
  description,
  type: "token",
  tokenType: "material",
});

const checkedProp = bool("Whether the control is on; bind it to a boolean to read and write it.", {
  writable: true,
});
const disabledProp = (what: string): PropDef =>
  bool(`Set to true to make ${what} non-interactive.`);
const rangeProps = (what: string): Record<string, PropDef> => ({
  min: num(`The smallest value ${what} allows; the default is 0.`, { default: 0 }),
  max: num(`The largest value ${what} allows; the default is 100.`, { default: 100 }),
  step: num(`The distance between values ${what} moves in; the default is 1.`, { default: 1 }),
});
const emptySlot = (what: string): SlotDef => ({
  description: `Content shown instead of the ${what} when there are none, such as "No results found".`,
});
const tone = (values: string[], description: string, extra: Partial<PropDef> = {}) =>
  oneOf(description, values, extra);

const components: Record<string, ComponentDef> = {
  screen: {
    description: "The root of every document: one full screen of UI, carrying the format version.",
    role: "main",
    content: "nodes",
    root: true,
    props: {
      weft: str('The Weft format version of the document, always the literal "0.1".', {
        required: true,
        bindable: false,
      }),
    },
    states: ["ready", "loading", "error"],
  },
  stack: {
    description:
      "Lays its children out in one line, as a column or a row; use it for most vertical or horizontal grouping.",
    role: "none",
    content: "nodes",
    props: {
      direction: oneOf("Whether children flow down a column or along a row.", ["column", "row"], {
        default: "column",
      }),
      gap: dimension("Space between children, as a dimension token."),
      align: oneOf("How children are aligned across the main direction.", [
        "start",
        "center",
        "end",
        "stretch",
      ]),
      wrap: bool("Set to true to let children wrap onto further lines when they do not fit."),
      material: material(
        "Frosted-glass surface behind the stack, as a material token: a background blur and a tint.",
      ),
    },
  },
  grid: {
    description:
      "Lays its children out in a fixed number of equal columns; use it for card galleries and dashboards.",
    role: "none",
    content: "nodes",
    props: {
      columns: num("Number of equal columns in the grid, at least 1.", {
        required: true,
        integer: true,
        min: 1,
      }),
      gap: dimension("Space between grid cells, as a dimension token."),
      material: material(
        "Frosted-glass surface behind the grid, as a material token: a background blur and a tint.",
      ),
    },
  },
  section: {
    description:
      "A named region of the screen that groups related content; use it to divide a screen into labelled parts.",
    role: "region",
    content: "nodes",
    requiresLabel: true,
    slots: {
      header: {
        description: "Optional header content shown above the section body, such as a heading.",
      },
    },
  },
  heading: {
    description:
      "A title for the content that follows it; the level orders headings from 1 (most important) to 6.",
    role: "heading",
    content: "text",
    props: {
      level: num("Heading level from 1 to 6; use 1 for the screen title.", {
        required: true,
        integer: true,
        min: 1,
        max: 6,
      }),
    },
  },
  text: {
    description: "A run of plain text such as a paragraph, caption or message.",
    role: "none",
    content: "text",
    props: {
      tone: tone(
        ["default", "muted", "success", "warning", "danger"],
        "Semantic colouring of the text; muted for secondary text, danger for errors.",
      ),
    },
  },
  image: {
    description: "A picture; always give a label that describes it for people who cannot see it.",
    role: "img",
    content: "none",
    requiresLabel: true,
    props: {
      src: str("Where the image is loaded from, a URL or a binding to one.", { required: true }),
    },
  },
  model: {
    description:
      "A 3D model the user can turn; give it a still fallback image and a label that describes what it shows.",
    role: "img",
    content: "none",
    requiresLabel: true,
    props: {
      src: str(
        "The glTF binary or JSON file (.glb, .gltf) the web shows; a path relative to the project or an https URL, never a binding.",
        { required: true, bindable: false },
      ),
      usdz: str(
        "The USDZ file (.usdz) Apple platforms show; a path relative to the project or an https URL, never a binding.",
        { bindable: false },
      ),
      fallback: str(
        "A still image (.png, .jpg, .jpeg, .webp) shown while the model loads, where it cannot be shown, and in design tools; same path rules as src.",
        { required: true, bindable: false },
      ),
    },
  },
  link: {
    description: "Navigates to another place; use a button instead for actions that change data.",
    role: "link",
    content: "text",
    props: {
      href: str("The destination URL; leave it out when navigation is handled by `on-press`."),
    },
    events: ["press"],
  },
  button: {
    description:
      "Triggers an action when pressed; use it for submitting, confirming and other commands.",
    role: "button",
    content: "text",
    props: {
      variant: oneOf(
        "Visual emphasis: primary for the main action, danger for destructive ones.",
        ["primary", "secondary", "danger"],
        { default: "secondary" },
      ),
      disabled: disabledProp("the button"),
      submit: bool(
        "Set to true to make the button submit its enclosing form, firing the form's `submit` event; such a button needs no `on-press`.",
        { default: false, bindable: false },
      ),
    },
    states: ["idle", "busy"],
    events: ["press"],
  },
  form: {
    description:
      "Groups input controls that are submitted together; the `submit` event fires when the user submits it.",
    role: "form",
    content: "nodes",
    slots: {
      footer: { description: "Actions of the form, typically the submit and cancel buttons." },
    },
    states: ["idle", "submitting", "invalid"],
    events: ["submit"],
  },
  field: {
    description: "A single-value text input such as an email, password, number or multi-line note.",
    role: "textbox",
    content: "none",
    requiresLabel: true,
    props: {
      type: oneOf(
        "What kind of text is entered; it selects the keyboard and masking.",
        ["text", "email", "password", "number", "search", "multiline"],
        { default: "text" },
      ),
      value: str("The current text; bind it to a path to read and write the input.", {
        writable: true,
      }),
      placeholder: str("Hint text shown while the field is empty; it does not replace the label."),
      required: bool("Set to true when the user must fill the field in."),
      disabled: disabledProp("the field"),
      error: str("An error message to show for this field; set it when the value is invalid."),
    },
    states: ["valid", "invalid"],
    events: ["change"],
  },
  checkbox: {
    description: "A box the user ticks or clears to turn one independent option on or off.",
    role: "checkbox",
    content: "none",
    requiresLabel: true,
    props: { checked: checkedProp, disabled: disabledProp("the checkbox") },
    events: ["change"],
  },
  switch: {
    description:
      "A toggle that applies a setting immediately; prefer it over a checkbox for on/off settings.",
    role: "switch",
    content: "none",
    requiresLabel: true,
    props: { checked: checkedProp, disabled: disabledProp("the switch") },
    events: ["change"],
  },
  "radio-group": {
    description: "A set of radio options of which exactly one can be chosen.",
    role: "radiogroup",
    content: "nodes",
    allowedChildren: ["radio", "each"],
    requiresLabel: true,
    props: {
      value: str("The `value` of the selected radio; bind it to read and write the choice.", {
        writable: true,
      }),
    },
    events: ["change"],
  },
  radio: {
    description: "One choice inside a radio group; its content is the visible label.",
    role: "radio",
    content: "text",
    allowedParents: ["radio-group"],
    props: {
      value: str("The value the group takes when this radio is selected.", { required: true }),
      disabled: disabledProp("this choice"),
    },
  },
  select: {
    description: "A drop-down that lets the user choose one option from a longer list.",
    role: "combobox",
    content: "nodes",
    allowedChildren: ["option", "each"],
    requiresLabel: true,
    props: {
      value: str("The `value` of the chosen option; bind it to read and write the choice.", {
        writable: true,
      }),
      disabled: disabledProp("the select"),
    },
    events: ["change"],
  },
  option: {
    description: "One choice inside a select or combobox; its content is the visible text.",
    role: "option",
    content: "text",
    allowedParents: ["select", "combobox"],
    props: {
      value: str("The value the select or combobox takes when this option is chosen.", {
        required: true,
      }),
    },
  },
  combobox: {
    description:
      "A text input with suggested options, for a value the user may type or pick; use select when only the listed options are valid.",
    role: "combobox",
    content: "nodes",
    allowedChildren: ["option", "each"],
    requiresLabel: true,
    props: {
      value: str(
        "The text of the input; bind it to read and write what the user typed or picked.",
        {
          writable: true,
        },
      ),
      placeholder: str("Hint text shown while the input is empty; it does not replace the label."),
      disabled: disabledProp("the combobox"),
    },
    events: ["change"],
  },
  slider: {
    description:
      "A thumb the user drags along a range to choose a number; use it when the exact value matters less than the position.",
    role: "slider",
    content: "none",
    requiresLabel: true,
    props: {
      value: num("The current number; bind it to read and write the choice.", { writable: true }),
      ...rangeProps("the slider"),
      disabled: disabledProp("the slider"),
    },
    events: ["change"],
  },
  stepper: {
    description:
      "A number the user raises or lowers by a fixed step with plus and minus buttons; use it for small counts such as a quantity.",
    role: "spinbutton",
    content: "none",
    requiresLabel: true,
    props: {
      value: num("The current number; bind it to read and write the count.", { writable: true }),
      min: num("The smallest value the stepper allows; without it the value has no lower bound."),
      max: num("The largest value the stepper allows; without it the value has no upper bound."),
      step: num("The amount one press adds or subtracts; the default is 1.", { default: 1 }),
      disabled: disabledProp("the stepper"),
    },
    events: ["change"],
  },
  "date-picker": {
    description: "Chooses a date, a time or both; the value is written as text in a fixed format.",
    role: "textbox",
    content: "none",
    requiresLabel: true,
    props: {
      type: oneOf(
        "What is chosen: `date` as yyyy-mm-dd, `time` as hh:mm, `datetime` as yyyy-mm-ddThh:mm.",
        ["date", "time", "datetime"],
        { default: "date" },
      ),
      value: str("The chosen date or time in the format of `type`; bind it to read and write it.", {
        writable: true,
      }),
      min: str("The earliest allowed value, in the format of `type`."),
      max: str("The latest allowed value, in the format of `type`."),
      disabled: disabledProp("the picker"),
    },
    events: ["change"],
  },
  "color-picker": {
    description:
      "Chooses a colour; the value is written as a lowercase hex colour such as #3b82f6.",
    role: "textbox",
    content: "none",
    requiresLabel: true,
    props: {
      value: str("The chosen colour as #rrggbb; bind it to read and write the choice.", {
        writable: true,
      }),
      disabled: disabledProp("the colour picker"),
    },
    events: ["change"],
  },
  "segmented-control": {
    description:
      "A row of joined segments of which exactly one can be chosen; use it for a few short, mutually exclusive choices that switch a view or mode.",
    role: "radiogroup",
    content: "nodes",
    allowedChildren: ["segment", "each"],
    requiresLabel: true,
    props: {
      value: str("The `value` of the chosen segment; bind it to read and write the choice.", {
        writable: true,
      }),
    },
    events: ["change"],
  },
  segment: {
    description: "One choice inside a segmented control; its content is the visible text.",
    role: "radio",
    content: "text",
    allowedParents: ["segmented-control"],
    props: {
      value: str("The value the control takes when this segment is chosen.", { required: true }),
      disabled: disabledProp("this segment"),
    },
  },
  list: {
    description:
      "A sequence of items; use it for feeds and collections, with `ordered` when the order matters.",
    role: "list",
    content: "nodes",
    allowedChildren: ["item", "each"],
    props: { ordered: bool("Set to true when the order of the items is meaningful.") },
    slots: { empty: emptySlot("items") },
    states: ["ready", "loading", "empty"],
  },
  item: {
    description: "One entry of a list; it can hold text and other components and may be pressed.",
    role: "listitem",
    content: "mixed",
    allowedParents: ["list"],
    events: ["press"],
  },
  table: {
    description: "Tabular data with column headers and rows; always give it a label.",
    role: "table",
    content: "nodes",
    allowedChildren: ["column", "row", "each"],
    requiresLabel: true,
    slots: { empty: emptySlot("rows") },
    states: ["ready", "loading", "empty"],
  },
  column: {
    description: "A column header of a table; its content is the header text.",
    role: "columnheader",
    content: "text",
    allowedParents: ["table"],
    props: {
      sort: oneOf("The sort direction currently applied to this column.", [
        "none",
        "ascending",
        "descending",
      ]),
    },
    events: ["press"],
  },
  row: {
    description: "One row of a table, made of cells in the order of the columns.",
    role: "row",
    content: "nodes",
    allowedChildren: ["cell"],
    allowedParents: ["table"],
    props: { selected: bool("Set to true when the row is selected.") },
    events: ["press"],
  },
  cell: {
    description: "One data cell of a table row; it can hold text and other components.",
    role: "cell",
    content: "mixed",
    allowedParents: ["row"],
  },
  tabs: {
    description:
      "Switches between alternative panels, one visible at a time; each panel is a `tab` child.",
    role: "tablist",
    content: "nodes",
    allowedChildren: ["tab"],
    props: {
      selected: str("The `id` of the selected `tab`; bind it to read and write the active tab.", {
        writable: true,
        references: "tab",
      }),
    },
    events: ["change"],
  },
  tab: {
    description:
      "One panel of a tabs component: its label becomes the tab title and its content the panel.",
    role: "tab",
    content: "nodes",
    allowedParents: ["tabs"],
    requiresLabel: true,
  },
  dialog: {
    description:
      "A window above the screen for confirmations and short tasks; put its buttons in `actions`.",
    role: "dialog",
    content: "nodes",
    requiresLabel: true,
    props: {
      modal: bool("When true the rest of the screen is blocked while the dialog is open.", {
        default: true,
      }),
      open: bool("Whether the dialog is shown; bind it to read and write visibility.", {
        writable: true,
      }),
    },
    slots: {
      actions: { description: "The buttons that answer the dialog, such as Cancel and Confirm." },
    },
    events: ["close"],
  },
  alert: {
    description:
      "A message that tells the user about a result or problem, announced to assistive technology.",
    role: "alert",
    content: "mixed",
    props: {
      tone: tone(["info", "success", "warning", "danger"], "Severity of the message.", {
        default: "info",
      }),
    },
  },
  menu: {
    description: "A list of commands the user can pick from, such as an actions or overflow menu.",
    role: "menu",
    content: "nodes",
    allowedChildren: ["menu-item", "each"],
    requiresLabel: true,
  },
  "menu-item": {
    description: "One command inside a menu; its content is the visible text.",
    role: "menuitem",
    content: "text",
    allowedParents: ["menu"],
    props: { disabled: disabledProp("the command") },
    events: ["press"],
  },
};

// SPEC §5.1: the `text` prop follows from the content model, so it is added here once rather than
// written into each definition, where a new text-bearing kind could forget it.
const textProp = str(
  "The text to show, given as a value instead of content; bind it when the text comes from data. Never give both.",
);
for (const def of Object.values(components)) {
  if (def.content === "text" || def.content === "mixed")
    def.props = { ...def.props, text: textProp };
}

export const coreCatalog: Catalog = {
  weft: "0.1",
  name: "weft-core",
  version: "0.1.0",
  components,
};
