// The bench checkers are imported by relative path: bench is a private harness, not a package
// this one should depend on, and its checkers are the one definition of "the task is done".
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";
import { coreCatalog, loadTokens, tokenTypes } from "@weft/catalog";
import { hasErrors, parse } from "@weft/core";
import { loadTasks, readScreen, type EditTask } from "../../../bench/src/corpus.ts";
import { checkEdit } from "../../../bench/src/run-tasks.ts";
import { call, connect } from "./connect.ts";

const tokens = tokenTypes(
  loadTokens(
    JSON.parse(
      readFileSync(new URL("../../catalog/tokens/default.tokens.json", import.meta.url), "utf8"),
    ),
  ).tokens,
);

type Solution = { task: string; patches: unknown[] };

// Hand-written, as a model would write them from the markup and the instruction.
const solutions: Solution[] = [
  {
    task: "login.e1",
    patches: [
      {
        op: "insert",
        parent: "fields",
        index: 2,
        markup: '<checkbox id="remember" label="Remember me" checked="{$.remember}"/>',
      },
    ],
  },
  {
    task: "login.e2",
    patches: [{ op: "set", id: "submit", prop: "disabled", value: { bind: "$.busy" } }],
  },
  {
    task: "login.e3",
    patches: [
      {
        op: "insert",
        parent: "form",
        slot: "footer",
        index: 1,
        markup: '<link id="sso" on-press="nav.sso">Use single sign-on</link>',
      },
    ],
  },
  {
    task: "signup.e1",
    patches: [
      {
        op: "insert",
        parent: "fields",
        index: 2,
        markup: '<field id="phone" label="Phone number" value="{$.phone}"/>',
      },
    ],
  },
  { task: "signup.e2", patches: [{ op: "set", id: "name", prop: "required", value: false }] },
  {
    task: "settings.e1",
    patches: [
      {
        op: "insert",
        parent: "appearance",
        markup: '<switch id="auto-save" label="Auto-save" checked="{$.autoSave}"/>',
      },
    ],
  },
  {
    task: "settings.e2",
    patches: [{ op: "move", id: "dark-mode", parent: "notifications" }],
  },
  {
    task: "data-table.e3",
    patches: [{ op: "set", id: "delete", prop: "on-press", value: "users.remove" }],
  },
  {
    task: "tabs.e1",
    patches: [
      {
        op: "insert",
        parent: "tabs",
        markup:
          '<tab id="tab-security" label="Security"><button id="enable-2fa" on-press="security.enable">Enable 2FA</button></tab>',
      },
    ],
  },
  { task: "tabs.e3", patches: [{ op: "set", id: "tab-team", prop: "label", value: "People" }] },
  {
    task: "confirm-dialog.e1",
    patches: [{ op: "set", id: "cancel", prop: "variant", value: "primary" }],
  },
  {
    task: "confirm-dialog.e3",
    patches: [{ op: "set", id: "delete-file", prop: "disabled", value: { bind: "$.busy" } }],
  },
  {
    task: "wizard-step.e1",
    patches: [
      {
        op: "insert",
        parent: "plan",
        markup: '<radio id="plan-enterprise" value="enterprise">Enterprise</radio>',
      },
    ],
  },
  {
    task: "search-results.e1",
    patches: [
      {
        op: "insert",
        parent: "bar",
        index: 2,
        markup: '<button id="clear" on-press="search.clear">Clear</button>',
      },
    ],
  },
  {
    task: "menu.e1",
    patches: [
      {
        op: "insert",
        parent: "menu",
        index: 3,
        markup: '<menu-item id="item-help" on-press="nav.help">Help</menu-item>',
      },
    ],
  },
  { task: "menu.e3", patches: [{ op: "move", id: "item-settings", parent: "menu", index: 3 }] },
  {
    task: "error-state.e2",
    patches: [{ op: "set", id: "retry", prop: "disabled", value: { bind: "$.offline" } }],
  },
];

const task = (id: string): EditTask => {
  const found = loadTasks().find((t) => t.id === id);
  assert.ok(found?.type === "edit", id);
  return found;
};

test("the solutions cover at least six different screens", () => {
  const screens = new Set(solutions.map((s) => task(s.task).screen));
  assert.ok(screens.size >= 6, [...screens].join(", "));
});

for (const { task: id, patches } of solutions) {
  test(`${id} is completed with weft_patch alone`, async () => {
    const t = task(id);
    const { client, close } = await connect({ tokens });
    try {
      const markup = readScreen(t.screen, "weft");
      const result = await call(client, "weft_patch", { markup, patches });
      assert.equal(result.isError, false, result.blocks.join("\n"));
      const reply = result.blocks[0] ?? "";

      const checked = checkEdit(t, "weft", reply);
      assert.deepEqual(checked.failed, []);
      assert.equal(checked.success, true);

      const { diagnostics } = parse(reply, { catalog: coreCatalog, mode: "strict", tokens });
      assert.equal(hasErrors(diagnostics), false, JSON.stringify(diagnostics));
      assert.deepEqual(diagnostics, []);
      // Canonical text is a fixed point of weft_format.
      assert.equal((await call(client, "weft_format", { markup: reply })).blocks[0], reply);
    } finally {
      await close();
    }
  });
}

test("a model repairs a rejected patch from the diagnostic alone", async () => {
  const { client, close } = await connect({ tokens });
  const markup = readScreen("login", "weft");
  const first = await call(client, "weft_patch", {
    markup,
    patches: [
      { op: "insert", parent: "fields", markup: '<checkbox id="submit" label="Remember me"/>' },
    ],
  });
  assert.equal(first.isError, true);
  const [d] = (
    JSON.parse(first.blocks[0] ?? "") as { diagnostics: { code: string; hint: string }[] }
  ).diagnostics;
  assert.equal(d?.code, "W509");
  const id = /"(.+)"/.exec(d?.hint ?? "")?.[1];
  assert.equal(id, "submit-2");

  const second = await call(client, "weft_patch", {
    markup,
    patches: [
      { op: "insert", parent: "fields", markup: `<checkbox id="${id}" label="Remember me"/>` },
    ],
  });
  assert.equal(second.isError, false, second.blocks.join("\n"));
  await close();
});
