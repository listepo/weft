// The reference React renderer (SPEC §9): one mapping per `weft-core` kind to semantic HTML and
// ARIA, so the accessibility tree has exactly the roles and names the document declares.
// Written with `createElement` so Node runs it without a JSX build step.
import type { Token } from "@weft/catalog";
import type { Catalog, Document } from "@weft/core";
import {
  createElement as h,
  Fragment,
  type CSSProperties,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import {
  contentText,
  dialogOpen,
  expandRoot,
  fieldInvalid,
  fieldValue,
  flag,
  headingLevel,
  label,
  nodes,
  options,
  ordered,
  ownText,
  prop,
  radioChecked,
  selectedTab,
  state,
  text,
  type Inst,
  type InstChild,
} from "./expand.ts";
import { exposedRole, fallbackRole, TRANSPARENT } from "./roles.ts";
import { isRecord, safeUrl, tokenCss } from "./values.ts";

export type ActionEvent = { id: string; action: string; item?: string };
export type Action = (event: ActionEvent) => void;

export type RenderOptions = {
  catalog: Catalog;
  data?: unknown;
  actions?: Record<string, Action>;
  // Receives every write to a two-way binding: the absolute data path and the new value.
  onChange?: (path: string, value: unknown) => void;
  tokens?: ReadonlyMap<string, Token>;
  // Prefix for the HTML ids that ARIA references need, so document ids cannot collide with
  // (or clobber globals named after) ids of the host page.
  idPrefix?: string;
};

type Ctx = { o: RenderOptions; prefix: string; group: Inst | undefined };
type Attrs = Record<string, unknown>;

export function render(document: Document, options: RenderOptions): ReactElement {
  const root = expandRoot(
    isRecord(document) ? document["root"] : undefined,
    options.catalog,
    options.data,
  );
  const ctx: Ctx = { o: options, prefix: options.idPrefix ?? "weft-", group: undefined };
  return h(Fragment, null, root ? renderNode(root, ctx) : null);
}

export function WeftView(props: RenderOptions & { document: Document }): ReactElement {
  return render(props.document, props);
}

// ---- Shared attribute helpers ----

const BUSY_STATES = new Set(["loading", "busy", "submitting"]);

// Every rendered element that carries a declared role gets the document id and its state:
// states with an ARIA equivalent are mapped to it, and all states stay visible as data-state.
function base(n: Inst, named = true): Attrs {
  const a: Attrs = {};
  if (n.id !== "") a["data-weft-id"] = n.id;
  const s = state(n);
  if (s !== "") a["data-state"] = s;
  if (BUSY_STATES.has(s)) a["aria-busy"] = "true";
  if (named && label(n) !== "") a["aria-label"] = label(n);
  return a;
}

function fire(ctx: Ctx, n: Inst, event: string): void {
  const action = n.on[event];
  const actions = ctx.o.actions;
  if (action === undefined || !actions || !Object.hasOwn(actions, action)) return;
  const fn = actions[action];
  if (typeof fn !== "function") return;
  fn(n.scope.item === undefined ? { id: n.id, action } : { id: n.id, action, item: n.scope.item });
}

const isBinding = (raw: unknown) => isRecord(raw) && typeof raw["bind"] === "string";

// Writes go only to plain bindings; a negated binding is read-only (SPEC §2.1).
function write(ctx: Ctx, n: Inst, name: string, value: unknown): void {
  const path = prop(n, name).path;
  if (path !== undefined) ctx.o.onChange?.(path, value);
}

const activates = (e: KeyboardEvent) => e.key === "Enter" || e.key === " ";

// Elements without native activation (list items, rows, header cells, links without href)
// become focusable and respond to Enter and Space when the document binds `on-press`.
function pressable(ctx: Ctx, n: Inst): Attrs {
  if (n.on["press"] === undefined) return {};
  return {
    tabIndex: 0,
    onClick: () => fire(ctx, n, "press"),
    onKeyDown: (e: KeyboardEvent) => {
      if (e.target !== e.currentTarget || !activates(e)) return;
      e.preventDefault();
      fire(ctx, n, "press");
    },
  };
}

const kids = (list: InstChild[], ctx: Ctx): ReactNode[] =>
  list.map((c) => (typeof c === "string" ? c : renderNode(c, ctx)));

// ---- Layout ----

const ALIGN: Record<string, string> = {
  start: "flex-start",
  center: "center",
  end: "flex-end",
  stretch: "stretch",
};

function layoutStyle(n: Inst, ctx: Ctx): CSSProperties {
  const style: CSSProperties = {};
  const gap = tokenCss(n.props["gap"], ctx.o.tokens);
  if (gap !== undefined) style.gap = gap;
  if (n.kind === "grid") {
    style.display = "grid";
    const columns = prop(n, "columns").value;
    if (typeof columns === "number" && Number.isInteger(columns) && columns >= 1 && columns <= 64) {
      style.gridTemplateColumns = `repeat(${columns}, minmax(0, 1fr))`;
    }
    return style;
  }
  style.display = "flex";
  style.flexDirection = text(n, "direction") === "row" ? "row" : "column";
  const align = text(n, "align");
  if (Object.hasOwn(ALIGN, align)) style.alignItems = ALIGN[align];
  if (flag(n, "wrap")) style.flexWrap = "wrap";
  return style;
}

// ---- Kinds ----

function renderNode(n: Inst, ctx: Ctx): ReactNode {
  if (!n.def) return fallback(n, ctx);
  switch (n.kind) {
    case "screen":
      return h("main", base(n), ...kids(ordered(n), ctx));
    case "stack":
    case "grid":
      return h("div", { ...base(n, false), style: layoutStyle(n, ctx) }, ...kids(ordered(n), ctx));
    case "section":
      return h("section", base(n), ...kids(ordered(n), ctx));
    case "heading":
      return h(`h${headingLevel(n)}`, base(n), ownText(n));
    case "text":
      return h("div", { ...base(n, false), "data-tone": text(n, "tone") || undefined }, ownText(n));
    case "image":
      return image(n);
    case "link":
      return link(n, ctx);
    case "button":
      return h(
        "button",
        {
          ...base(n),
          type: "button",
          disabled: flag(n, "disabled"),
          "data-variant": text(n, "variant") || undefined,
          onClick: () => fire(ctx, n, "press"),
        },
        contentText(n.children),
      );
    case "form":
      return h(
        "form",
        {
          ...base(n),
          onSubmit: (e: { preventDefault(): void }) => {
            e.preventDefault();
            fire(ctx, n, "submit");
          },
        },
        ...kids(ordered(n), ctx),
      );
    case "field":
      return field(n, ctx);
    case "checkbox":
    case "switch":
      return toggle(n, ctx);
    case "radio-group":
      return h(
        "div",
        { ...base(n), role: "radiogroup" },
        ...kids(ordered(n), { ...ctx, group: n }),
      );
    case "radio":
      return radio(n, ctx);
    case "select":
      return select(n, ctx);
    case "option":
      return h("div", base(n, false), contentText(n.children));
    case "list":
      return h(flag(n, "ordered") ? "ol" : "ul", base(n), ...kids(ordered(n), ctx));
    case "item":
      return h("li", { ...base(n), ...pressable(ctx, n) }, ...kids(ordered(n), ctx));
    case "table":
      return table(n, ctx);
    case "tabs":
      return tabs(n, ctx);
    case "tab":
      return h("div", { ...base(n), role: "group" }, ...kids(ordered(n), ctx));
    case "dialog":
      return dialog(n, ctx);
    case "alert":
      return h(
        "div",
        { ...base(n), role: "alert", "data-tone": text(n, "tone") || undefined },
        ...kids(ordered(n), ctx),
      );
    case "menu":
      return h("div", { ...base(n), role: "menu" }, ...kids(ordered(n), ctx));
    case "menu-item":
      return h(
        "button",
        {
          ...base(n),
          type: "button",
          role: "menuitem",
          disabled: flag(n, "disabled"),
          onClick: () => fire(ctx, n, "press"),
        },
        contentText(n.children),
      );
    // Table parts reached outside their table keep their role on a plain container.
    case "row":
    case "cell":
    case "column":
    default:
      return fallback(n, ctx);
  }
}

// Unknown kinds and `x-` extensions render their children in a container with their fallback
// role (SPEC §8); kinds the catalog knows but this renderer does not map use the catalog role.
function fallback(n: Inst, ctx: Ctx): ReactNode {
  const role = exposedRole(fallbackRole(n), n);
  const transparent = TRANSPARENT.has(role);
  return h(
    "div",
    { ...base(n, !transparent), role: transparent ? undefined : role },
    ...kids(ordered(n), ctx),
  );
}

function image(n: Inst): ReactNode {
  const src = safeUrl(text(n, "src"));
  return h("img", { ...base(n, false), alt: label(n), src });
}

function link(n: Inst, ctx: Ctx): ReactNode {
  const href = safeUrl(text(n, "href"));
  // Without an href an <a> is neither a link nor focusable, so both are restored explicitly;
  // like a native link it activates on Enter only.
  const restored =
    href === undefined
      ? {
          role: "link",
          tabIndex: 0,
          onKeyDown: (e: KeyboardEvent) => {
            if (e.key !== "Enter" || e.target !== e.currentTarget) return;
            e.preventDefault();
            fire(ctx, n, "press");
          },
        }
      : {};
  return h(
    "a",
    {
      ...base(n),
      href,
      ...restored,
      onClick: n.on["press"] === undefined ? undefined : () => fire(ctx, n, "press"),
      // An inline box would run its name into neighbouring text; see `contentText`.
      style: { display: "inline-block" },
    },
    contentText(n.children),
  );
}

const INPUT_TYPES = new Set(["text", "email", "password", "number", "search"]);

function field(n: Inst, ctx: Ctx): ReactNode {
  const name = label(n);
  const error = text(n, "error");
  const errorId = `${ctx.prefix}${n.id}-error`;
  const type = text(n, "type");
  const value = isBinding(n.props["value"])
    ? {
        value: fieldValue(n),
        onChange: (e: { currentTarget: { value: string } }) => {
          write(ctx, n, "value", e.currentTarget.value);
          fire(ctx, n, "change");
        },
      }
    : { defaultValue: text(n, "value"), onChange: () => fire(ctx, n, "change") };
  const control = h(type === "multiline" ? "textarea" : "input", {
    ...base(n, false),
    "aria-label": name,
    type: type === "multiline" ? undefined : INPUT_TYPES.has(type) ? type : "text",
    placeholder: text(n, "placeholder") || undefined,
    required: flag(n, "required"),
    disabled: flag(n, "disabled"),
    "aria-invalid": fieldInvalid(n) ? "true" : undefined,
    "aria-describedby": error ? errorId : undefined,
    ...value,
  });
  return h(
    "div",
    { "data-weft-field": "" },
    // The visible label repeats the accessible name, so it is hidden from the tree to avoid a
    // duplicate text node.
    h("label", null, h("span", { "aria-hidden": "true" }, name), control),
    error ? h("div", { id: errorId }, error) : null,
  );
}

function toggle(n: Inst, ctx: Ctx): ReactNode {
  const name = label(n);
  const checked = isBinding(n.props["checked"])
    ? {
        checked: flag(n, "checked"),
        onChange: (e: { currentTarget: { checked: boolean } }) => {
          write(ctx, n, "checked", e.currentTarget.checked);
          fire(ctx, n, "change");
        },
      }
    : { defaultChecked: flag(n, "checked"), onChange: () => fire(ctx, n, "change") };
  return h(
    "label",
    null,
    h("input", {
      ...base(n, false),
      type: "checkbox",
      role: n.kind === "switch" ? "switch" : undefined,
      "aria-label": name,
      disabled: flag(n, "disabled"),
      ...checked,
    }),
    h("span", { "aria-hidden": "true" }, name),
  );
}

function radio(n: Inst, ctx: Ctx): ReactNode {
  const group = ctx.group;
  const name = label(n) || contentText(n.children);
  const on = radioChecked(n, group);
  const controlled = group !== undefined && isBinding(group.props["value"]);
  return h(
    "label",
    null,
    h("input", {
      ...base(n, false),
      type: "radio",
      name: group && group.id !== "" ? `${ctx.prefix}${group.id}` : undefined,
      value: text(n, "value"),
      "aria-label": name,
      disabled: flag(n, "disabled"),
      ...(controlled ? { checked: on } : { defaultChecked: on }),
      onChange: () => {
        if (!group) return;
        write(ctx, group, "value", prop(n, "value").value);
        fire(ctx, group, "change");
      },
    }),
    h("span", { "aria-hidden": "true" }, name),
  );
}

function select(n: Inst, ctx: Ctx): ReactNode {
  // The HTML parser drops anything but options inside <select>, so only options are rendered.
  const opts = options(n);
  const value = isBinding(n.props["value"])
    ? { value: text(n, "value") }
    : { defaultValue: text(n, "value") };
  return h(
    "select",
    {
      ...base(n, false),
      "aria-label": label(n),
      disabled: flag(n, "disabled"),
      ...value,
      onChange: (e: { currentTarget: { value: string } }) => {
        const chosen = opts.find((o) => text(o, "value") === e.currentTarget.value);
        write(ctx, n, "value", chosen ? prop(chosen, "value").value : e.currentTarget.value);
        fire(ctx, n, "change");
      },
    },
    ...opts.map((o) =>
      h("option", { ...base(o), value: text(o, "value") }, contentText(o.children)),
    ),
  );
}

// SPEC §5.1: `column` children form the header row; the renderer emits it in <thead>.
// Content that is not a row is wrapped in a row and cell so the HTML parser keeps it in place.
function table(n: Inst, ctx: Ctx): ReactNode {
  const columns = nodes(n.children).filter((c) => c.kind === "column");
  const body = n.children.filter((c) => typeof c === "string" || c.kind !== "column");
  return h(
    "table",
    base(n),
    columns.length > 0
      ? h("thead", null, h("tr", null, ...columns.map((c) => column(c, ctx))))
      : null,
    body.length > 0
      ? h(
          "tbody",
          null,
          ...body.map((c) =>
            typeof c !== "string" && c.kind === "row"
              ? row(c, ctx)
              : h("tr", null, h("td", null, ...kids([c], ctx))),
          ),
        )
      : null,
  );
}

const SORT = new Set(["none", "ascending", "descending"]);

function column(n: Inst, ctx: Ctx): ReactNode {
  const sort = text(n, "sort");
  return h(
    "th",
    {
      ...base(n),
      scope: "col",
      "aria-sort": SORT.has(sort) ? sort : undefined,
      ...pressable(ctx, n),
    },
    contentText(n.children),
  );
}

function row(n: Inst, ctx: Ctx): ReactNode {
  const selected = n.props["selected"] === undefined ? undefined : String(flag(n, "selected"));
  return h(
    "tr",
    { ...base(n), "aria-selected": selected, ...pressable(ctx, n) },
    ...n.children.map((c) =>
      typeof c !== "string" && c.kind === "cell"
        ? h("td", base(c), ...kids(ordered(c), ctx))
        : h("td", null, ...kids([c], ctx)),
    ),
  );
}

// SPEC §5.1: a `tab` holds its panel, so one `tabs` element becomes a tablist of tab buttons
// followed by one tabpanel per tab, of which only the selected one is shown.
function tabs(n: Inst, ctx: Ctx): ReactNode {
  const list = nodes(n.children).filter((c) => c.kind === "tab");
  const other = n.children.filter((c) => typeof c === "string" || c.kind !== "tab");
  const sel = selectedTab(n, list);
  const tabId = (t: Inst) => `${ctx.prefix}${t.id}-tab`;
  const panelId = (t: Inst) => `${ctx.prefix}${t.id}-panel`;
  const choose = (i: number) => {
    const t = list[i];
    if (!t || i === sel) return;
    write(ctx, n, "selected", t.docId);
    fire(ctx, n, "change");
  };
  const onKeyDown = (e: KeyboardEvent<HTMLElement>, i: number) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (step === 0 || list.length === 0) return;
    e.preventDefault();
    const next = (i + step + list.length) % list.length;
    choose(next);
    const buttons = e.currentTarget.parentElement?.querySelectorAll<HTMLElement>('[role="tab"]');
    buttons?.[next]?.focus();
  };
  return h(
    "div",
    { "data-weft-tabs": "" },
    h(
      "div",
      { ...base(n), role: "tablist" },
      ...list.map((t, i) =>
        h(
          "button",
          {
            ...base(t, false),
            type: "button",
            role: "tab",
            id: tabId(t),
            "aria-selected": String(i === sel),
            "aria-controls": panelId(t),
            tabIndex: i === sel ? 0 : -1,
            onClick: () => choose(i),
            onKeyDown: (e: KeyboardEvent<HTMLElement>) => onKeyDown(e, i),
          },
          label(t),
        ),
      ),
      ...kids(other, ctx),
    ),
    ...list.map((t, i) =>
      h(
        "div",
        {
          role: "tabpanel",
          id: panelId(t),
          "aria-labelledby": tabId(t),
          hidden: i !== sel,
          tabIndex: 0,
        },
        ...kids(ordered(t), ctx),
      ),
    ),
  );
}

function dialog(n: Inst, ctx: Ctx): ReactNode {
  if (!dialogOpen(n)) return null;
  const modal = n.props["modal"] === undefined || flag(n, "modal");
  return h(
    "dialog",
    {
      ...base(n),
      open: true,
      "aria-modal": modal ? "true" : undefined,
      onKeyDown: (e: KeyboardEvent) => {
        if (e.key !== "Escape") return;
        e.preventDefault();
        write(ctx, n, "open", false);
        fire(ctx, n, "close");
      },
    },
    ...kids(ordered(n), ctx),
  );
}
