// Data schema cases (SPEC §10.5), shared by data.test.ts and the TypeScript–Rust differential
// check. Markup is written against the test catalog of ./catalog.ts.
import { screen } from "./cases.ts";

export type DataCase = { schema: unknown; markup: string; codes: string[] };

const todos = {
  type: "object",
  properties: {
    email: { type: "string" },
    busy: { type: "boolean" },
    level: { type: "integer" },
    note: { type: ["string", "null"] },
    nothing: { type: "null" },
    profile: { type: "object" },
    gone: false,
    prices: { type: "object", additionalProperties: { type: "number" } },
    todos: {
      type: "array",
      items: {
        type: "object",
        properties: { title: { type: "string" }, done: { type: "boolean" } },
      },
    },
  },
};

const list = (inner: string, as = "todo", source = "$.todos") =>
  screen(
    `<list id="l"><each id="e" as="${as}" in="{${source}}"><item id="i">${inner}</item></each></list>`,
  );

const nest = (depth: number): unknown =>
  depth === 0 ? { type: "string" } : { type: "object", properties: { a: nest(depth - 1) } };

export const dataCases: Record<string, DataCase> = {
  "declared paths of the right type": {
    schema: todos,
    markup: screen(
      '<field id="f" label="Email" value="{$.email}"/><button id="b" disabled="{$.busy}">Go</button><heading id="h" level="{$.level}">Hi</heading>',
    ),
    codes: [],
  },
  "a loop variable starts at the items schema": {
    schema: todos,
    markup: list('<text id="t" text="{$todo.title}"/>'),
    codes: [],
  },
  "a misspelled name": {
    schema: todos,
    markup: screen('<field id="f" label="Email" value="{$.emial}"/>'),
    codes: ["W315"],
  },
  "a misspelled name below a loop variable": {
    schema: todos,
    markup: list('<text id="t" text="{$todo.titel}"/>'),
    codes: ["W315"],
  },
  "a string where a boolean goes": {
    schema: todos,
    markup: screen('<button id="b" disabled="{$.email}">Go</button>'),
    codes: ["W316"],
  },
  "a negated binding takes any type": {
    schema: todos,
    markup: screen('<button id="b" disabled="{!$.email}">Go</button>'),
    codes: [],
  },
  "text shows numbers": {
    schema: todos,
    markup: screen('<text id="t" text="{$.level}"/>'),
    codes: [],
  },
  "a string where a number goes": {
    schema: todos,
    markup: screen('<heading id="h" level="{$.email}">Hi</heading>'),
    codes: ["W316"],
  },
  "an object as text": {
    schema: todos,
    markup: screen('<text id="t" text="{$.profile}"/>'),
    codes: ["W316"],
  },
  "a nullable string as text": {
    schema: todos,
    markup: screen('<text id="t" text="{$.note}"/>'),
    codes: [],
  },
  "only null": {
    schema: todos,
    markup: screen('<text id="t" text="{$.nothing}"/>'),
    codes: ["W316"],
  },
  "label is a string attribute": {
    schema: todos,
    markup: screen('<field id="f" label="{$.busy}"/>'),
    codes: ["W316"],
  },
  "each needs an array": {
    schema: todos,
    markup: list('<text id="t" text="{$todo.title}"/>', "todo", "$.email"),
    codes: ["W316"],
  },
  "an index steps into items": {
    schema: todos,
    markup: screen('<text id="t" text="{$.todos.0.title}"/>'),
    codes: [],
  },
  "a name on an array": {
    schema: todos,
    markup: screen('<text id="t" text="{$.todos.title}"/>'),
    codes: ["W315"],
  },
  "a step into a string": {
    schema: todos,
    markup: screen('<text id="t" text="{$.email.length}"/>'),
    codes: ["W315"],
  },
  "a property declared false": {
    schema: todos,
    markup: screen('<text id="t" text="{$.gone}"/>'),
    codes: ["W315"],
  },
  "a map declares every name": {
    schema: todos,
    markup: screen('<text id="t" text="{$.prices.anything}"/>'),
    codes: [],
  },
  "an object without properties is open": {
    schema: todos,
    markup: screen('<text id="t" text="{$.profile.name}"/>'),
    codes: [],
  },
  "a schema without type accepts everything": {
    schema: {},
    markup: screen('<button id="b" disabled="{$.a.b.0}">Go</button>'),
    codes: [],
  },
  "true accepts everything": {
    schema: true,
    markup: screen('<text id="t" text="{$.x}"/>'),
    codes: [],
  },
  "false declares nothing": {
    schema: false,
    markup: screen('<text id="t" text="{$.x}"/>'),
    codes: ["W315"],
  },
  "an unsupported keyword accepts anything there": {
    schema: { type: "object", properties: { user: { $ref: "#/$defs/user" } } },
    markup: screen('<text id="t" text="{$.user.name}"/>'),
    codes: ["W710"],
  },
  "a type that names nothing": {
    schema: { type: "object", properties: { a: { type: "str" }, b: { type: ["string", 1] } } },
    markup: screen('<text id="t" text="{$.a}"/>'),
    codes: ["W709", "W709"],
  },
  "properties that is not an object": {
    schema: { type: "object", properties: ["a"] },
    markup: screen('<text id="t" text="{$.a}"/>'),
    codes: ["W709"],
  },
  "a schema that is a number": {
    schema: 5,
    markup: screen('<text id="t" text="{$.a}"/>'),
    codes: ["W709"],
  },
  "nesting deeper than the limit": {
    schema: nest(300),
    markup: screen('<text id="t" text="{$.a.a}"/>'),
    // `$.a.a` is still an object, which text does not show.
    codes: ["W709", "W316"],
  },
  "an unknown loop variable is left to the validator": {
    schema: todos,
    markup: screen('<text id="t" text="{$todo.title}"/>'),
    codes: [],
  },
  "extension attributes are checked for paths only": {
    schema: todos,
    markup: screen(
      '<x-acme-map id="m" role="img" x-acme-center="{$.profile}" x-acme-zoom="{$.zoom}"/>',
    ),
    codes: ["W315"],
  },
  "named slots and nested loops": {
    schema: {
      type: "object",
      properties: {
        groups: {
          type: "array",
          items: {
            type: "object",
            properties: { rows: { type: "array", items: { type: "string" } } },
          },
        },
        open: { type: "boolean" },
      },
    },
    markup: screen(
      '<dialog id="d" label="D" open="{$.open}"><list id="l"><each id="g" as="group" in="{$.groups}"><each id="r" as="row" in="{$group.rows}"><item id="i"><text id="t" text="{$row}"/><text id="u" text="{$row.x}"/></item></each></each></list><slot name="actions"><button id="b" disabled="{$.open.x}">Go</button></slot></dialog>',
    ),
    codes: ["W315", "W315"],
  },
};
