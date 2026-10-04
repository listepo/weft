// `values.ts` repeats the reference forms the WebAssembly core reads, for Figma's main thread.
// Each case goes through both, so the copy cannot drift from the core.
import assert from "node:assert/strict";
import { parse, serialize, type Document, type Value } from "@weft/core";
import { describe, test } from "vitest";
import { formatValue, readText } from "../src/index.ts";

const RAW = [
  "Sign in",
  "",
  "{$.user.name}",
  "{!$.email}",
  "{$item.title}",
  "{token.color.accent}",
  "{{literal brace",
  "{not a reference}",
  "{",
  "{$",
  "50% off",
];

const coreRead = (raw: string): Value | undefined => {
  const { document } = parse(
    `<screen id="s" weft="0.1"><x-a-b id="t" role="note" v="${raw}"/></screen>`,
  );
  const node = document?.root.children?.[0];
  return typeof node === "object" ? node.props?.["v"] : undefined;
};

describe("value forms", () => {
  for (const raw of RAW) {
    test(`read like the core: ${JSON.stringify(raw)}`, () => {
      assert.deepEqual(readText(raw), coreRead(raw));
    });
  }

  const values: Value[] = [
    "plain",
    "{brace",
    { bind: "$.a.b" },
    { bind: "$.a", not: true },
    { token: "space.md" },
    3,
    true,
  ];
  for (const value of values) {
    test(`written like the core: ${JSON.stringify(value)}`, () => {
      const document: Document = {
        weft: "0.1",
        root: {
          kind: "screen",
          id: "s",
          children: [{ kind: "x-a-b", id: "t", props: { role: "note", v: value } }],
        },
      };
      assert.match(
        serialize(document),
        new RegExp(` v="${formatValue(value).replace(/[{}$.]/g, "\\$&")}"`),
      );
    });
  }
});
