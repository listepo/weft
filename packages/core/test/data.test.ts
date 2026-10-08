import assert from "node:assert/strict";
import { describe, test } from "vitest";
import { checkData, compileDataSchema, parse, type Diagnostic } from "../src/index.ts";
import { catalog } from "./catalog.ts";
import { screen } from "./cases.ts";
import { dataCases } from "./data-cases.ts";

function run(schema: unknown, markup: string): Diagnostic[] {
  const { schema: data } = compileDataSchema(schema);
  const { document, source } = parse(markup, { catalog });
  assert.ok(document, "the case markup parses");
  return checkData(document, { catalog, data, source });
}

for (const [name, c] of Object.entries(dataCases)) {
  describe(name, () => {
    test(`reports ${c.codes.join(", ") || "nothing"}`, () => {
      assert.deepEqual(
        run(c.schema, c.markup).map((d) => d.code),
        c.codes,
      );
    });
  });
}

describe("a data diagnostic", () => {
  test("points at the attribute and names the nearest declared property", () => {
    const schema = { type: "object", properties: { email: { type: "string" } } };
    const [d] = run(schema, screen('\n<field id="f" label="E" value="{$.emial}"/>'));
    assert.deepEqual(d, {
      code: "W315",
      severity: "error",
      message: 'The data schema does not declare "emial" in $.',
      path: "/screen#s/field#f/@value",
      line: 2,
      column: 25,
      expected: 'one of: "email"',
      got: "$.emial",
      hint: 'did you mean "email"?',
    });
  });

  test("names both types when the type does not fit", () => {
    const schema = { type: "object", properties: { busy: { type: "string" } } };
    const [d] = run(schema, screen('<button id="b" disabled="{$.busy}">Go</button>'));
    assert.equal(d?.message, "The data at $.busy is string; this attribute takes boolean.");
    assert.equal(d?.expected, "boolean");
    assert.equal(d?.got, "string");
  });
});

describe("compileDataSchema", () => {
  // Not a shared case: the fixture stays shallow enough for serde_json's default recursion limit.
  test("stops at 256 nested schemas", () => {
    const nest = (depth: number): unknown =>
      depth === 0 ? { type: "string" } : { type: "object", properties: { a: nest(depth - 1) } };
    const { problems } = compileDataSchema(nest(300));
    const pointer = "/properties/a".repeat(257);
    assert.deepEqual(problems, [
      {
        code: "W709",
        pointer,
        message: `Schemas nest deeper than 256 levels at ${pointer}.`,
      },
    ]);
    // `$.a.a` is still an object, which text does not show.
    assert.deepEqual(
      run(nest(300), screen('<text id="t" text="{$.a.a}"/>')).map((d) => d.code),
      ["W709", "W316"],
    );
  });

  test("never throws on hostile input", () => {
    for (const bad of [null, 1, "x", [], { type: {} }, { properties: null }, { items: 7 }]) {
      assert.doesNotThrow(() => compileDataSchema(bad));
    }
  });

  test("checkData includes schema compile problems", () => {
    const { schema: data } = compileDataSchema(5);
    const { document } = parse(screen('<text id="t" text="{$.a}"/>'), { catalog });
    assert.ok(document);
    assert.ok(checkData(document, { catalog, data }).some((d) => d.code === "W709"));
  });

  test("points problems into the schema", () => {
    const { problems } = compileDataSchema({ properties: { "a/b": { anyOf: [] } } });
    assert.deepEqual(problems, [
      {
        code: "W710",
        pointer: "/properties/a~1b/anyOf",
        message:
          '"anyOf" at /properties/a~1b/anyOf is not supported; that schema accepts any data.',
      },
    ]);
  });
});
