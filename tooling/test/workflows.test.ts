// The workflows name moon targets as plain text, and only the hosted runner used to check them:
// a since-removed deploy job ran `moon run penpot-plugin:build`, which moon drops in CI because the
// task has `runInCI: false`, so the deploy failed on main while the gate stayed green. This suite
// checks every target in .github/workflows against the workspace's tasks, offline.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, test } from "vitest";

type Tasks = Record<string, Record<string, { options: { runInCI: boolean | string } }>>;

const tasks = (
  JSON.parse(
    execFileSync("moon", ["query", "tasks"], {
      encoding: "utf8",
      maxBuffer: 1 << 28,
      stdio: ["ignore", "pipe", "pipe"],
    }),
  ) as { tasks: Tasks }
).tasks;

// moon removes these from the pipeline whenever it detects CI, unless told to ignore that.
const droppedInCI = new Set<boolean | string>([false, "skip"]);

/** Every `moon run` or `moon exec` command line in a workflow, comments left out. */
function moonCommands(workflow: string): string[] {
  return workflow
    .split("\n")
    .filter((line) => !line.trimStart().startsWith("#"))
    .flatMap((line) => [...line.matchAll(/\bmoon\s+(?:run|exec)\s[^\n#&|;]*/g)].map((m) => m[0]));
}

/** What would make the command fail in CI: an unknown target, or one moon drops there. */
function problems(command: string): string[] {
  const words = command.trim().split(/\s+/).slice(2);
  const end = words.indexOf("--");
  const args = end === -1 ? words : words.slice(0, end);
  const ignoresCI =
    args.includes("--ignore-ci-checks") ||
    args.some((arg, i) => arg === "--ci=false" || (arg === "--ci" && args[i + 1] === "false"));
  const found: string[] = [];
  for (const target of args.filter((arg) => !arg.startsWith("-") && arg.includes(":"))) {
    const match = /^([\w.-]*):([\w.-]+)$/.exec(target);
    if (!match) {
      found.push(`${target}: not a project:task or :task target`);
      continue;
    }
    const [, project = "", task = ""] = match;
    const projects = project === "" ? Object.keys(tasks) : [project];
    const resolved = projects.filter((id) => tasks[id]?.[task]);
    if (resolved.length === 0) {
      found.push(`${target}: no such moon task`);
      continue;
    }
    if (ignoresCI) continue;
    for (const id of resolved) {
      if (droppedInCI.has(tasks[id]?.[task]?.options.runInCI ?? true))
        found.push(
          `${id}:${task}: runInCI is off, so moon skips it in CI without --ignore-ci-checks`,
        );
    }
  }
  return found;
}

const folder = fileURLToPath(new URL("../../.github/workflows/", import.meta.url));
const workflows = readdirSync(folder).filter((name) => /\.ya?ml$/.test(name));

describe("the moon targets the workflows run", () => {
  test("the check itself flags an unknown task and a task moon drops in CI", () => {
    assert.deepEqual(problems("moon run penpot-plugin:nope"), [
      "penpot-plugin:nope: no such moon task",
    ]);
    assert.equal(problems("moon run penpot-plugin:build").length, 1);
    assert.deepEqual(problems("moon exec penpot-plugin:build --ignore-ci-checks"), []);
    assert.deepEqual(problems("moon exec penpot-plugin:build --ci false"), []);
    assert.deepEqual(problems("moon run :test root:lint -- --reporter dot"), []);
  });

  test("the workflows run moon at all", () => {
    const all = workflows.flatMap((name) => moonCommands(readFileSync(folder + name, "utf8")));
    assert.ok(all.length > 0, "no moon command found in .github/workflows; is the parser stale?");
  });

  for (const name of workflows) {
    test(`${name} runs only tasks that exist and that moon runs in CI`, () => {
      const commands = moonCommands(readFileSync(folder + name, "utf8"));
      assert.deepEqual(commands.flatMap(problems), []);
    });
  }
});

// The creator decided against GitHub Pages, and Pages is off for the repository: a Pages deploy
// fails on every push to main, which turned main red while every pull request stayed green.
describe("no workflow deploys to GitHub Pages", () => {
  for (const name of workflows) {
    test(`${name} asks for no Pages permission and runs no Pages deploy`, () => {
      const lines = readFileSync(folder + name, "utf8")
        .split("\n")
        .filter((line) => !line.trimStart().startsWith("#"));
      assert.deepEqual(
        lines.filter((line) => /^\s*pages:\s*write\b|\bactions\/deploy-pages@/.test(line)),
        [],
      );
    });
  }
});
