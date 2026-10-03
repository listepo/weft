import type { Format } from "./neutral.ts";

// Each primer teaches just enough of its format to edit the corpus screens. Weft and A2UI primers
// are written to similar length (reported by `run.ts tokens`) so neither gains from a longer manual.

const WEFT = `Weft is a strict XML-subset format for describing a UI screen.
Syntax: one root <screen id="..." weft="0.1" label="...">. Every element has a unique id. Attribute values are always double-quoted. Booleans are written true or false. Elements without content are self-closing.
Values: a literal ("Email"), a binding value="{$.user.email}" (inputs write back to the data model), a negated binding disabled="{!$.email}", a token gap="{token.space.md}". Never mix text and a binding in one value. Inside <each in="{$.items}" as="item"> children use {$item.field}.
Events: on-press, on-submit, on-change, on-close hold an action name such as auth.submit (no arguments).
Structure: children nest directly. <slot name="footer"> is allowed in form, <slot name="actions"> in dialog. <each> repeats its single child for every item of an array.
Universal attributes: id, label (accessible name), hidden, state.
Components (props; * = required):
stack(direction column|row, gap, align, wrap); grid(columns*, gap); section(label*); heading(level* 1-6, value or text); text(value or text, tone); image(src*, label*); link(href; press); button(variant primary|secondary|danger, disabled; press); form(submit; slot footer); field(label*, type text|email|password|number|search|multiline, value, placeholder, required, disabled; change); checkbox(label*, checked, disabled; change); switch(label*, checked, disabled; change); radio-group(label*, value; change) containing radio(value*, text); select(label*, value; change) containing option(value*, text); list(ordered) containing item; table(label*) containing column(sort none|ascending|descending; press) and row containing cell; tabs containing tab(label*) which holds its panel content; dialog(label*, open, modal; close; slot actions); alert(tone info|success|warning|danger); menu(label*) containing menu-item(disabled; press).
Only these components and props exist.`;

const A2UI = `A2UI v0.9 describes a UI as a JSON array of messages. Use {"version":"v0.9","createSurface":{"surfaceId":...,"catalogId":"https://a2ui.org/specification/v0_9/catalogs/basic/catalog.json"}} then {"version":"v0.9","updateComponents":{"surfaceId":...,"components":[...]}}.
components is a flat list. Each entry has a unique "id" and a "component" type; parents list children by id ("children":[ids] or "child":id). One component has id "root". There is no nesting.
Values: a literal ("Email"), or a data binding {"path":"/user/email"} (a JSON Pointer; inside a template the relative path "title" refers to the current item). Inputs write back to the bound path.
Actions: Button "action" is {"event":{"name":"auth.submit"}} or {"functionCall":{"call":"openUrl","args":{"url":...}}}. Buttons disable themselves when a check fails: "checks":[{"condition":{"call":"required","args":{"value":{"path":"/email"}}},"message":"..."}]; use {"call":"not","args":{"value":{"path":"/busy"}}} to disable while a value is true. Functions: required, not, and, or, regex, length, numeric, email.
Repetition: "children":{"componentId":"item-template","path":"/todos"} renders the template component once per array item.
Components (props; * = required):
Text(text*, variant h1-h5|body); Image(url*, description, variant); Row/Column(children*, justify, align); List(children*, direction); Card(child*); Tabs(tabs*: [{title, child}]); Modal(trigger*, content*); Divider; Button(child*, action*, variant default|primary|borderless, checks); TextField(label*, value, variant shortText|longText|number|obscured, checks); CheckBox(label*, value*, checks); ChoicePicker(options*: [{label,value}], value*, label, variant mutuallyExclusive|multipleSelection, filterable); Slider; DateTimeInput.
Every component also accepts "accessibility":{"label":...,"description":...}. Only these components and props exist; there is no table, switch, alert, menu, link or heading level 6.`;

const HTML = `The screen is semantic HTML5 with ARIA. Conventions: data-bind="prop:$.path; prop2:!$.path" binds an element property (value, checked, disabled, hidden, open, src, href, text) to the data model, "!" negates; data-action="event:action.name" names the action fired by an event (press, submit, change, close). Repetition uses <template data-each="$.items" data-as="item"> and inside it paths are $item.field. data-variant sets button variants (primary, danger). Layout classes stack, row, gap-sm and gap-md carry spacing intent. Labels use <label for>, tabs use role=tablist/tab/tabpanel, menus role=menu/menuitem.`;

const JSX = `The screen is an idiomatic React function component receiving data (the data model) and actions (named host actions). Conventions: values read from data.path; inputs are controlled and write with actions.set("path", value); named actions are called as actions.group.name(); repetition uses data.items.map((item, index) => ...); data-variant sets button variants (primary, danger); className stack, row, gap-sm and gap-md carry layout intent. Use semantic HTML elements and ARIA attributes only; no styling beyond classes, no hooks.`;

export const PRIMERS: Record<Format, string> = { weft: WEFT, a2ui: A2UI, html: HTML, jsx: JSX };
