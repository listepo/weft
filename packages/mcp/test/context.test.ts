// weft_context and the context rules of weft_patch (SPEC §2.3, §7; AGENT-SPEC §2.9).
import assert from "node:assert/strict";
import { test } from "vitest";
import { loadProject } from "@weft/catalog";
import { hostOptions } from "../src/project.ts";
import { CONTEXT_NOTICE, withinLimits } from "../src/tools/context.ts";
import { call, connect } from "./connect.ts";

const SCREEN = `<screen id="s" label="Sign in" weft="0.2">
  <context>
    <entry id="why" by="human" kind="intent" name="Ivan">Returning users sign in.</entry>
    <entry id="go-why" by="agent" for="go" kind="decision" name="m">Primary, because it is the one action.</entry>
    <entry id="where" by="agent" for="go" kind="question" name="m" status="open">Dialog or screen?</entry>
  </context>
  <button id="go" on-press="auth.submit" variant="primary">Sign in</button>
</screen>
`;

type Listed = { notice: string; entries: { id: string }[]; omitted?: number };
const listed = (text: string | undefined) => JSON.parse(text ?? "null") as Listed;
const ids = (text: string | undefined) => listed(text).entries.map((e) => e.id);

test("weft_context returns the entries under the fixed notice, filtered", async () => {
  const { client, close } = await connect();
  const all = await call(client, "weft_context", { markup: SCREEN });
  assert.equal(all.isError, false);
  assert.equal(listed(all.blocks[0]).notice, CONTEXT_NOTICE);
  assert.deepEqual(ids(all.blocks[0]), ["why", "go-why", "where"]);
  const screen = await call(client, "weft_context", { markup: SCREEN, for: "" });
  assert.deepEqual(ids(screen.blocks[0]), ["why"]);
  const open = await call(client, "weft_context", { markup: SCREEN, for: "go", kind: "question" });
  assert.deepEqual(ids(open.blocks[0]), ["where"]);
  const none = await call(client, "weft_context", { markup: SCREEN, status: "resolved" });
  assert.deepEqual(ids(none.blocks[0]), []);
  const broken = await call(client, "weft_context", { markup: "<screen" });
  assert.equal(broken.isError, true);
  await close();
});

test("an oversized context is cut at the format's limits before a model sees it", () => {
  const entry = (i: number) => ({
    id: `e${i}`,
    kind: "intent",
    by: "agent",
    name: "m",
    text: "x".repeat(600),
  });
  const { entries, omitted } = withinLimits(Array.from({ length: 40 }, (_, i) => entry(i)));
  assert.equal(entries[0]?.text.length, 500);
  assert.equal(entries.length, 32);
  assert.equal(omitted, 8);
});

test("weft_patch writes context as the agent, and a read-only host refuses it", async () => {
  const add = (by: string) => [
    {
      op: "add-context",
      entry: { id: "n", kind: "todo", by, name: "m", status: "open", text: "Check the copy." },
    },
  ];
  const { client, close } = await connect();
  const ok = await call(client, "weft_patch", { markup: SCREEN, patches: add("agent") });
  assert.equal(ok.isError, false);
  assert.match(
    ok.blocks[0] ?? "",
    /<entry id="n" by="agent" kind="todo" name="m" status="open">Check the copy\.<\/entry>/,
  );
  const human = await call(client, "weft_patch", { markup: SCREEN, patches: add("human") });
  assert.equal(human.isError, true);
  assert.match(human.blocks[0] ?? "", /W512/);
  await close();

  const readOnly = await connect({}, { context: "read-only" });
  const { tools } = await readOnly.client.listTools();
  assert.match(tools.find((t) => t.name === "weft_patch")?.description ?? "", /read-only/);
  const refused = await call(readOnly.client, "weft_patch", {
    markup: SCREEN,
    patches: add("agent"),
  });
  assert.equal(refused.isError, true);
  assert.match(refused.blocks[0] ?? "", /W512/);
  const read = await call(readOnly.client, "weft_context", { markup: SCREEN });
  assert.deepEqual(ids(read.blocks[0]), ["why", "go-why", "where"]);
  await readOnly.close();
});

test("mcp.context in the project file reaches the server settings", () => {
  const { project } = loadProject({ mcp: { context: "read-only" } });
  assert.equal(hostOptions(project).settings.context, "read-only");
});
