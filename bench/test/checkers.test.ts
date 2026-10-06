import assert from "node:assert/strict";
import { test } from "vitest";
import { SCREENS, loadTasks, readScreen, type EditTask, type QuestionTask } from "../src/corpus.ts";
import { FORMATS, type Format } from "../src/neutral.ts";
import { mockProvider } from "../src/provider.ts";
import {
  checkAnswer,
  checkEdit,
  editPrompt,
  planJobs,
  readPrompt,
  rescore,
  runJobs,
  runTask,
  summarize,
} from "../src/run-tasks.ts";

const task = (id: string): EditTask => {
  const t = loadTasks().find((x) => x.id === id);
  assert.ok(t && t.type === "edit", id);
  return t;
};
const question = (id: string): QuestionTask => loadTasks().find((x) => x.id === id) as QuestionTask;

const replace = (src: string, from: string, to: string): string => {
  assert.ok(src.includes(from), `anchor not found: ${from}`);
  return src.replace(from, to);
};

type Comp = Record<string, unknown> & { id: string };
function a2ui(src: string, change: (byId: Map<string, Comp>, list: Comp[]) => void): string {
  const messages = JSON.parse(src) as { updateComponents?: { components: Comp[] } }[];
  const list = messages.find((m) => m.updateComponents)?.updateComponents?.components as Comp[];
  change(new Map(list.map((c) => [c.id, c])), list);
  return JSON.stringify(messages, null, 2);
}
const need = (byId: Map<string, Comp>, id: string): Comp => {
  const c = byId.get(id);
  assert.ok(c, id);
  return c;
};
const insertAfter = (children: unknown[], after: string, id: string) =>
  children.splice(children.indexOf(after) + 1, 0, id);

// Hand-written correct solutions: [task id, format] -> edited source.
const solutions: Record<string, Partial<Record<Format, (src: string) => string>>> = {
  "login.e1": {
    weft: (s) =>
      replace(
        s,
        "    </stack>",
        '      <checkbox id="remember" label="Remember me" checked="{$.remember}"/>\n    </stack>',
      ),
    html: (s) =>
      replace(
        s,
        '<button type="submit"',
        '<label><input type="checkbox" data-bind="checked:$.remember"><span>Remember me</span></label>\n<button type="submit"',
      ),
    jsx: (s) =>
      replace(
        s,
        '<button type="submit"',
        '<label><input type="checkbox" checked={data.remember} onChange={(e) => actions.set("remember", e.target.checked)} /><span>Remember me</span></label>\n<button type="submit"',
      ),
    a2ui: (s) =>
      a2ui(s, (byId, list) => {
        list.push({
          id: "remember",
          component: "CheckBox",
          label: "Remember me",
          value: { path: "/remember" },
        });
        (need(byId, "fields").children as string[]).push("remember");
      }),
  },
  "login.e2": {
    weft: (s) => replace(s, "{!$.email}", "{$.busy}"),
    html: (s) => replace(s, "disabled:!$.email", "disabled:$.busy"),
    jsx: (s) => replace(s, "disabled={!data.email}", "disabled={data.busy}"),
    a2ui: (s) =>
      a2ui(s, (byId) => {
        (need(byId, "submit").checks as { condition: unknown }[])[0] = {
          condition: { call: "not", args: { value: { path: "/busy" } } },
        };
      }),
  },
  "login.e3": {
    weft: (s) =>
      replace(
        s,
        "Forgot password?</link>",
        'Forgot password?</link>\n      <link id="sso" on-press="nav.sso">Use single sign-on</link>',
      ),
    html: (s) =>
      replace(
        s,
        "Forgot password?</a>",
        'Forgot password?</a>\n<a href="#" data-action="press:nav.sso">Use single sign-on</a>',
      ),
    jsx: (s) =>
      replace(
        s,
        "Forgot password?</a>",
        'Forgot password?</a>\n<a href="#" onClick={() => actions.nav.sso()}>Use single sign-on</a>',
      ),
    a2ui: (s) =>
      a2ui(s, (byId, list) => {
        list.push(
          { id: "sso-label", component: "Text", text: "Use single sign-on" },
          {
            id: "sso",
            component: "Button",
            child: "sso-label",
            variant: "borderless",
            action: { event: { name: "nav.sso" } },
          },
        );
        insertAfter(need(byId, "form").children as string[], "reset", "sso");
      }),
  },
  "tabs.e3": {
    weft: (s) => replace(s, 'label="Team"', 'label="People"'),
    html: (s) => replace(s, ">Team</button>", ">People</button>"),
    jsx: (s) => replace(s, ">Team</button>", ">People</button>"),
    a2ui: (s) =>
      a2ui(s, (byId) => {
        (need(byId, "tabs").tabs as { title: string }[])[2]!.title = "People";
      }),
  },
  "data-table.e1": {
    weft: (s) =>
      replace(
        s,
        '          <button id="delete"',
        '          <button id="edit" on-press="users.edit">Edit</button>\n          <button id="delete"',
      ),
    html: (s) =>
      replace(
        s,
        '<button type="button" data-variant="danger" data-bind="label:$user.deleteLabel"',
        '<button type="button" data-action="press:users.edit">Edit</button>\n<button type="button" data-variant="danger" data-bind="label:$user.deleteLabel"',
      ),
    jsx: (s) =>
      replace(
        s,
        '<button type="button" aria-label={user.deleteLabel} data-variant="danger"',
        '<button type="button" onClick={() => actions.users.edit()}>Edit</button>\n<button type="button" aria-label={user.deleteLabel} data-variant="danger"',
      ),
    a2ui: (s) =>
      a2ui(s, (byId, list) => {
        list.push(
          { id: "edit-label", component: "Text", text: "Edit" },
          {
            id: "edit",
            component: "Button",
            child: "edit-label",
            action: { event: { name: "users.edit" } },
          },
        );
        (need(byId, "row").children as string[]).splice(3, 0, "edit");
      }),
  },
};

const wrap = (f: Format, doc: string): string => `Here you go:\n\`\`\`${f}\n${doc}\n\`\`\`\n`;

test("checkers accept a hand-written correct solution in every format", () => {
  let checked = 0;
  for (const [id, byFormat] of Object.entries(solutions)) {
    const t = task(id);
    for (const f of FORMATS) {
      const edit = byFormat[f];
      assert.ok(edit, `${id} ${f}`);
      const result = checkEdit(t, f, wrap(f, edit(readScreen(t.screen, f))));
      assert.deepEqual(result.failed, [], `${id} ${f}`);
      assert.deepEqual(result.errors, [], `${id} ${f}`);
      assert.equal(result.success, true, `${id} ${f}`);
      checked++;
    }
  }
  assert.equal(checked, 20);
});

test("checkers reject the unmodified screen for every edit task and format", () => {
  for (const t of loadTasks()) {
    if (t.type !== "edit") continue;
    for (const f of FORMATS)
      assert.equal(checkEdit(t, f, readScreen(t.screen, f)).success, false, `${t.id} ${f}`);
  }
});

test("order, content loss and invalid output are rejected", () => {
  const t = task("login.e1");
  // checkbox placed above the password field violates `after`
  const misplaced = replace(
    readScreen("login", "weft"),
    '      <field id="password"',
    '      <checkbox id="remember" label="Remember me" checked="{$.remember}"/>\n      <field id="password"',
  );
  assert.equal(checkEdit(t, "weft", misplaced).success, false);
  // edit done but the footer links were dropped
  const lossy = solutions["login.e1"]!.weft!(readScreen("login", "weft")).replace(
    /<slot name="footer">[\s\S]*<\/slot>\n/,
    "",
  );
  const r = checkEdit(t, "weft", lossy);
  assert.equal(r.success, false);
  assert.ok(r.failed.some((x) => x.startsWith("lost content")));
  // duplicate id and unknown kind make the document invalid even though the checkbox is there
  const dup = solutions["login.e1"]!.weft!(readScreen("login", "weft")).replace(
    'id="remember"',
    'id="email"',
  );
  assert.equal(checkEdit(t, "weft", dup).valid, false);
  const unknown = solutions["login.e1"]!.weft!(readScreen("login", "weft"))
    .replace("<checkbox", "<tickbox")
    .replace('Remember me"/>', 'Remember me"/>');
  assert.equal(checkEdit(t, "weft", unknown).valid, false);
  assert.equal(checkEdit(t, "a2ui", "not json").valid, false);
  assert.equal(checkEdit(t, "jsx", "export default function X( {").valid, false);
});

test("weft validity catches single quotes, bare attributes and unclosed elements", () => {
  const t = task("login.e2");
  const ok = solutions["login.e2"]!.weft!(readScreen("login", "weft"));
  assert.equal(checkEdit(t, "weft", ok).valid, true);
  assert.equal(checkEdit(t, "weft", ok.replace('level="1"', "level='1'")).valid, false);
  assert.equal(checkEdit(t, "weft", ok.replace('required="true"', "required")).valid, false);
  assert.equal(checkEdit(t, "weft", ok.replace("</heading>", "")).valid, false);
  assert.equal(
    checkEdit(t, "weft", ok.replace('variant="primary"', 'variant="huge"')).valid,
    false,
  );
});

test("question answers are matched on the last ANSWER line", () => {
  const q = question("login.q1");
  assert.equal(checkAnswer(q, "ANSWER: nav.reset"), true);
  assert.equal(checkAnswer(q, "Let me look.\nANSWER: `Nav.Reset`."), true);
  assert.equal(checkAnswer(q, "ANSWER: nav.signup"), false);
  assert.equal(checkAnswer(q, "nav.reset"), false);
});

test("an action named in the format's own spelling is the right answer", () => {
  const q = question("login.q1");
  for (const spelled of [
    "press:nav.reset",
    "actions.nav.reset()",
    "actions.nav.reset",
    "nav.reset()",
  ])
    assert.equal(checkAnswer(q, `ANSWER: ${spelled}`), true, spelled);
  assert.equal(checkAnswer(q, "ANSWER: press:nav.signup"), false);
  // Only action spellings are unwrapped; other answers still have to match exactly.
  assert.equal(checkAnswer(question("login.q2"), "ANSWER: press:2"), false);
});

test("prompts contain the primer, data model, screen and the request", () => {
  const e = editPrompt(task("login.e2"), "weft");
  assert.match(e, /Weft is a strict XML-subset/);
  assert.match(e, /"email"/);
  assert.match(e, /<screen id="login"/);
  assert.match(e, /Make the Sign in button disabled/);
  assert.match(readPrompt(question("login.q1"), "a2ui"), /A2UI v0\.9[\s\S]*Question: Which action/);
});

test("runTask scores a mocked model and summaries aggregate it", async () => {
  const t = task("login.e2");
  const good = mockProvider((p) =>
    wrap("weft", solutions["login.e2"]!.weft!(p.match(/```xml\n([\s\S]*?)```/)![1]!.trim())),
  );
  const bad = mockProvider(() => "```xml\n<screen/>\n```");
  const a = await runTask(t, "weft", good, "m");
  const b = await runTask(t, "weft", bad, "m");
  assert.equal(a.success, true);
  assert.equal(b.success, false);
  const [s] = summarize([a, b]);
  assert.equal(s?.success.mean, 0.5);
  const q = await runTask(
    question("login.q2"),
    "html",
    mockProvider(() => "ANSWER: 2"),
    "m",
  );
  assert.equal(q.success, true);
  assert.equal(q.repaired, false);
  assert.equal(q.reply, "ANSWER: 2");
});

test("an invalid edit reply gets one repair prompt with the validator's diagnostics", async () => {
  const t = task("login.e2");
  const solve = (p: string) =>
    wrap("weft", solutions["login.e2"]!.weft!(p.match(/```xml\n([\s\S]*?)```/)![1]!.trim()));
  const prompts: string[] = [];
  const r = await runTask(
    t,
    "weft",
    mockProvider((p) => {
      prompts.push(p);
      return p.includes("Your previous reply") ? solve(p) : '```xml\n<screen id="login"\n```';
    }),
    "m",
  );
  assert.equal(prompts.length, 2);
  assert.match(prompts[1]!, /It is not a valid document:\n- /);
  assert.deepEqual(
    [r.valid, r.success, r.repaired, r.validAfterRepair, r.successAfterRepair],
    [false, false, true, true, true],
  );
  const [s] = summarize([r]);
  assert.deepEqual([s?.valid.mean, s?.validAfterRepair.mean], [0, 1]);

  let calls = 0;
  const once = await runTask(
    t,
    "weft",
    mockProvider((p) => {
      calls++;
      return solve(p);
    }),
    "m",
  );
  assert.equal(calls, 1);
  assert.equal(once.repaired, false);
});

test("tasks cover every screen", () => {
  assert.equal(new Set(loadTasks().map((t) => t.screen)).size, SCREENS.length);
});

test("samples are summarized as a mean with the single-sample range", async () => {
  const t = task("login.e2");
  const solve = (p: string) =>
    wrap("weft", solutions["login.e2"]!.weft!(p.match(/```xml\n([\s\S]*?)```/)![1]!.trim()));
  const jobs = planJobs([t], ["weft"], ["m"], 3);
  assert.deepEqual(
    jobs.map((j) => j.sample),
    [0, 1, 2],
  );
  let n = 0;
  // The second sample fails, the others succeed.
  const { results, error } = await runJobs(
    jobs,
    () => mockProvider((p) => (n++ === 1 ? "```xml\n<screen/>\n```" : solve(p))),
    1,
  );
  assert.equal(error, undefined);
  const [s] = summarize(results);
  assert.deepEqual([s?.tasks, s?.samples], [1, 3]);
  assert.deepEqual(s?.success, { mean: 2 / 3, min: 0, max: 1 });
});

test("runJobs keeps finished results and stops at the first failure", async () => {
  const q = question("login.q2");
  const jobs = planJobs([q], ["weft", "html", "jsx", "a2ui"], ["m"], 1);
  let calls = 0;
  let active = 0;
  let peak = 0;
  const { results, error } = await runJobs(
    jobs,
    () => ({
      async complete() {
        active++;
        peak = Math.max(peak, active);
        await new Promise((r) => setTimeout(r, 5));
        active--;
        if (++calls === 2) throw new Error("quota");
        return { text: "ANSWER: 2" };
      },
    }),
    2,
  );
  assert.match(String(error), /quota/);
  assert.equal(peak, 2);
  assert.ok(results.length >= 1 && results.length < 4);
  assert.ok(results.every((r) => r.success));
});

test("rescore re-checks saved replies and keeps results without one", async () => {
  const q = question("login.q1");
  const r = await runTask(
    q,
    "html",
    mockProvider(() => "ANSWER: press:nav.reset"),
    "m",
  );
  // As scored by a checker that did not know format spellings.
  const old = { ...r, success: false, successAfterRepair: false, failed: ["wrong answer"] };
  const legacy = { ...old, id: "login.q1", reply: undefined as unknown as string };
  const [fixed, kept] = rescore([old, legacy], loadTasks());
  assert.equal(fixed?.success, true);
  assert.equal(kept, legacy);
});
