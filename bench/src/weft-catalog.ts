// SPEC §5.1 as a table for the harness. T3 delivers the real catalog in packages/; this copy only
// lets the benchmark judge model output before that package can be imported.

type Prop = "string" | "number" | "boolean" | readonly string[];

export interface Def {
  props: Record<string, Prop>;
  required?: string[];
  events?: string[];
  states?: string[];
  text?: boolean;
}

const dim = "string";
const press = ["press"];
const change = ["change"];

export const WEFT_KINDS: Record<string, Def> = {
  screen: { props: { weft: "string" }, required: ["weft"], states: ["ready", "loading", "error"] },
  stack: {
    props: {
      direction: ["column", "row"],
      gap: dim,
      align: ["start", "center", "end", "stretch"],
      wrap: "boolean",
    },
  },
  grid: { props: { columns: "number", gap: dim }, required: ["columns"] },
  section: { props: {}, required: ["label"] },
  heading: { props: { level: "number", value: "string" }, required: ["level"], text: true },
  text: {
    props: { value: "string", tone: ["default", "muted", "success", "warning", "danger"] },
    text: true,
  },
  image: { props: { src: "string" }, required: ["src", "label"] },
  link: { props: { href: "string" }, events: press, text: true },
  button: {
    props: { variant: ["primary", "secondary", "danger"], disabled: "boolean" },
    states: ["idle", "busy"],
    events: press,
    text: true,
  },
  form: { props: {}, states: ["idle", "submitting", "invalid"], events: ["submit"] },
  field: {
    props: {
      type: ["text", "email", "password", "number", "search", "multiline"],
      value: "string",
      placeholder: "string",
      required: "boolean",
      disabled: "boolean",
      error: "string",
    },
    required: ["label"],
    states: ["valid", "invalid"],
    events: change,
  },
  checkbox: {
    props: { checked: "boolean", disabled: "boolean" },
    required: ["label"],
    events: change,
  },
  switch: {
    props: { checked: "boolean", disabled: "boolean" },
    required: ["label"],
    events: change,
  },
  "radio-group": { props: { value: "string" }, required: ["label"], events: change },
  radio: { props: { value: "string", disabled: "boolean" }, required: ["value"], text: true },
  select: { props: { value: "string", disabled: "boolean" }, required: ["label"], events: change },
  option: { props: { value: "string" }, required: ["value"], text: true },
  list: { props: { ordered: "boolean" }, states: ["ready", "loading", "empty"] },
  item: { props: {}, events: press, text: true },
  table: { props: {}, required: ["label"], states: ["ready", "loading", "empty"] },
  column: { props: { sort: ["none", "ascending", "descending"] }, events: press, text: true },
  row: { props: { selected: "boolean" }, events: press },
  cell: { props: {}, text: true },
  tabs: { props: { selected: "string" }, events: change },
  tab: { props: {}, required: ["label"] },
  dialog: { props: { modal: "boolean", open: "boolean" }, required: ["label"], events: ["close"] },
  alert: { props: { tone: ["info", "success", "warning", "danger"] }, text: true },
  menu: { props: {}, required: ["label"] },
  "menu-item": { props: { disabled: "boolean" }, events: press, text: true },
};

export const UNIVERSAL_ATTRS = ["id", "label", "hidden", "state", "role"];
export const STRUCTURAL = ["slot", "each"];
