// The shared plugin UI over a fake DOM: what it sends for each button, and what it shows for each
// reply. The plugin bundle tests run the same UI built, inside each tool's split.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { beforeAll, describe, test } from "vitest";
import type { PluginRequest } from "@weft/design-tool";
import { FakeElement } from "./ui-harness.ts";

const tokens = readFileSync(
  new URL("../../catalog/tokens/default.tokens.json", import.meta.url),
  "utf8",
);
const login = readFileSync(new URL("../../../corpus/login/screen.weft", import.meta.url), "utf8");

const elements = new Map<string, FakeElement>();
const byId = (id: string): FakeElement => {
  let found = elements.get(id);
  if (found === undefined) elements.set(id, (found = new FakeElement()));
  return found;
};
const sent: PluginRequest[] = [];
let reply: (message: unknown) => void = () => {};

beforeAll(async () => {
  Object.assign(globalThis, {
    WEFT_DEFAULT_TOKENS: JSON.parse(tokens),
    document: { getElementById: byId, createElement: () => new FakeElement() },
  });
  const { startUi } = await import("../src/index.ts");
  startUi({ send: (request) => sent.push(request), listen: (onReply) => (reply = onReply) });
});

describe("the plugin UI", () => {
  test("sends a parsed screen to build and an export request", () => {
    byId("source").value = login;
    byId("build").click();
    assert.equal(sent.at(-1)?.type, "build");
    byId("export").click();
    assert.equal(sent.at(-1)?.type, "export");
  });

  test("sends nothing for broken markup", () => {
    const before = sent.length;
    byId("source").value = "<screen";
    byId("build").click();
    assert.equal(sent.length, before);
    assert.ok(byId("notes").items.length > 0);
  });

  test("shows replies as text and ignores anything that is not a reply", () => {
    reply({ type: "error", message: "<b>no</b>" });
    assert.equal(byId("status").textContent, "<b>no</b>");
    for (const junk of [null, "built", { type: 1 }]) reply(junk);
    assert.equal(byId("status").textContent, "<b>no</b>");
    reply({ type: "built", id: "1" });
    assert.equal(byId("status").textContent, "Built.");
  });
});
