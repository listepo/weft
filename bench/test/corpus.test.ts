import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { CORPUS_DIR, SCREENS, loadTasks, readScreen } from "../src/corpus.ts";
import { FILE_NAME, parsers } from "../src/formats.ts";
import { FORMATS, evaluate, walk, type NNode } from "../src/neutral.ts";

const lift = (n: NNode, kinds: string[]): NNode[] => {
  const kids = n.children.flatMap((c) => lift(c, kinds));
  return kinds.includes(n.kind) ? kids : [{ ...n, children: kids }];
};

test("every screen has all five files", () => {
  for (const screen of SCREENS) {
    for (const f of [...FORMATS.map((x) => FILE_NAME[x]), "data.json"])
      assert.ok(existsSync(`${CORPUS_DIR}${screen}/${f}`), `${screen}/${f}`);
    JSON.parse(readFileSync(`${CORPUS_DIR}${screen}/data.json`, "utf8"));
  }
});

test("every format parses without errors", () => {
  for (const screen of SCREENS) {
    for (const f of FORMATS) {
      const p = parsers[f](readScreen(screen, f));
      assert.deepEqual(p.errors, [], `${screen} ${f}`);
      assert.ok(p.tree && p.tree.children.length > 0, `${screen} ${f}`);
    }
  }
});

test("html and jsx describe exactly the same tree as weft", () => {
  for (const screen of SCREENS) {
    const weft = lift(parsers.weft(readScreen(screen, "weft")).tree as NNode, ["stack"]);
    for (const f of ["html", "jsx"] as const) {
      const other = lift(parsers[f](readScreen(screen, f)).tree as NNode, ["stack"]);
      assert.deepEqual(other, weft, `${screen} ${f}`);
    }
  }
});

// A2UI has no form, item, row or cell and drops several props, so only the stream of named,
// bound or acting nodes is compared: same content, same order, same bindings, same actions.
const stream = (tree: NNode): string[] =>
  walk(tree)
    .map((e) => e.node)
    .filter((n) => n.kind !== "screen")
    .map((n) => {
      // A2UI has no form submit and no dialog close event; both are listed as gaps in corpus/README.md.
      const { submit: _submit, ...rest } = n.on;
      const on = n.kind === "dialog" ? {} : rest;
      const bind = n.kind === "dialog" || n.kind === "tabs" ? "" : (n.bind ?? "");
      const each = n.kind === "each" ? `each ${n.each}` : "";
      return [each, n.name ?? "", n.nameBind ?? "", bind, Object.entries(on).join(",")].join("|");
    })
    .filter((s) => s !== "||||");

test("a2ui carries the same content stream as weft", () => {
  for (const screen of SCREENS) {
    const weft = parsers.weft(readScreen(screen, "weft")).tree as NNode;
    const a2ui = parsers.a2ui(readScreen(screen, "a2ui")).tree as NNode;
    assert.deepEqual(stream(a2ui), stream(weft), screen);
  }
});

test("tasks: 3 edits and 2 questions per screen, unique ids", () => {
  const tasks = loadTasks();
  assert.equal(new Set(tasks.map((t) => t.id)).size, tasks.length);
  for (const screen of SCREENS) {
    assert.equal(tasks.filter((t) => t.screen === screen && t.type === "edit").length, 3, screen);
    assert.equal(
      tasks.filter((t) => t.screen === screen && t.type === "question").length,
      2,
      screen,
    );
  }
});

test("no edit expectation already holds on the unmodified screen", () => {
  for (const task of loadTasks()) {
    if (task.type !== "edit") continue;
    for (const f of FORMATS) {
      const tree = parsers[f](readScreen(task.screen, f)).tree as NNode;
      const results = evaluate(tree, task.expect, f);
      for (const r of results)
        assert.equal(r.ok, false, `${task.id} ${f}: ${JSON.stringify(r.assertion)} already holds`);
    }
  }
});
