// The iteration loop: runs the checks the changed files affect, instead of the whole workspace.
// The merge gate stays the full check (`FULL_CHECK` below); this script falls back to it whenever
// it cannot trust its own selection. `--dry-run` prints the plan without running it.
import { execFileSync, spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { relative } from "node:path";
import {
  type Crate,
  type Facts,
  type Plan,
  type Project,
  type Task,
  projectOf,
  referencedProjects,
  select,
} from "./select.ts";

const FULL_CHECK = [
  "run",
  ":test",
  "root:typecheck",
  "root:lint",
  "root:rust-test",
  "root:rust-lint",
  "root:runtimes",
];

const read = (command: string, args: string[]) =>
  execFileSync(command, args, {
    encoding: "utf8",
    maxBuffer: 1 << 28,
    stdio: ["ignore", "pipe", "pipe"],
  });

const lines = (text: string) => text.split("\0").filter(Boolean);

function mergeBase(requested: string | undefined): string {
  if (requested) {
    return read("git", ["rev-parse", "--verify", `${requested}^{commit}`]).trim();
  }
  for (const branch of ["main", "origin/main"]) {
    try {
      return read("git", ["merge-base", "HEAD", branch]).trim();
    } catch {
      // Try the next name: a worktree of a clone may only have the remote branch.
    }
  }
  throw new Error("no merge base with main");
}

// Committed, staged, unstaged and untracked work: the same set Vitest's `--changed` sees.
function changedSince(base: string): string[] {
  const tracked = lines(read("git", ["diff", "--name-only", "--no-renames", "-z", base]));
  const untracked = lines(read("git", ["ls-files", "-z", "--others", "--exclude-standard"]));
  return [...new Set([...tracked, ...untracked])].sort();
}

interface MoonTask {
  target: string;
  command?: string;
  script?: string;
  inputFiles?: Record<string, unknown>;
  inputGlobs?: Record<string, unknown>;
  deps?: { target: string }[];
}

function moonFacts(): { projects: Project[]; tasks: Task[] } {
  const projects = (
    JSON.parse(read("moon", ["query", "projects"])) as {
      projects: { id: string; source: string; dependencies?: { id: string }[] }[];
    }
  ).projects.map((project) => ({
    id: project.id,
    source: project.source,
    dependencies: (project.dependencies ?? []).map((dependency) => dependency.id),
  }));
  const grouped = (
    JSON.parse(read("moon", ["query", "tasks"])) as {
      tasks: Record<string, Record<string, MoonTask>>;
    }
  ).tasks;
  const tasks = Object.entries(grouped).flatMap(([project, byName]) =>
    Object.entries(byName).map(([name, task]) => ({
      target: task.target,
      project,
      name,
      globs: Object.keys(task.inputGlobs ?? {}),
      files: Object.keys(task.inputFiles ?? {}),
      deps: (task.deps ?? []).map((dep) => dep.target),
      vitest: task.command === "vitest" && task.script === undefined,
    })),
  );
  return { projects, tasks };
}

function cargoCrates(): Crate[] {
  const metadata = JSON.parse(
    read("cargo", ["metadata", "--no-deps", "--format-version", "1"]),
  ) as {
    workspace_root: string;
    packages: { name: string; manifest_path: string; dependencies: { name: string }[] }[];
  };
  const names = new Set(metadata.packages.map((pkg) => pkg.name));
  return metadata.packages.map((pkg) => ({
    name: pkg.name,
    dir: relative(metadata.workspace_root, pkg.manifest_path).replace(/\/Cargo\.toml$/, ""),
    dependsOn: pkg.dependencies.map((dep) => dep.name).filter((name) => names.has(name)),
  }));
}

// Tests read sibling packages by path as well as by import; the package.json graph shows only the
// imports, so the sources are scanned for the paths.
function sourceReads(projects: Project[]): Facts["reads"] {
  const found = { source: {}, own: {} } as Record<
    keyof Facts["reads"],
    Record<string, Set<string>>
  >;
  const files = lines(
    read("git", [
      "ls-files",
      "-z",
      "--cached",
      "--others",
      "--exclude-standard",
      "--",
      "*.ts",
      "*.tsx",
      "*.mts",
    ]),
  );
  for (const file of files) {
    const id = projectOf(file, projects);
    if (id === undefined) {
      continue;
    }
    let text = "";
    try {
      text = readFileSync(file, "utf8");
    } catch {
      continue; // Deleted in the working tree.
    }
    const home = projects.find((project) => project.id === id);
    const kind = home && file.startsWith(`${home.source}/src/`) ? "source" : "own";
    for (const other of referencedProjects(text, projects)) {
      if (other !== id) {
        (found[kind][id] ??= new Set()).add(other);
      }
    }
  }
  const lists = (byId: Record<string, Set<string>>) =>
    Object.fromEntries(Object.entries(byId).map(([id, set]) => [id, [...set]]));
  return { source: lists(found.source), own: lists(found.own) };
}

function gather(requestedBase: string | undefined): { base: string; facts: Facts } {
  const base = mergeBase(requestedBase);
  const { projects, tasks } = moonFacts();
  return {
    base,
    facts: {
      changed: changedSince(base),
      projects,
      tasks,
      crates: cargoCrates(),
      reads: sourceReads(projects),
    },
  };
}

const quote = (arg: string) => (/^[\w@%+=:,./-]+$/.test(arg) ? arg : `'${arg}'`);

interface Step {
  command: string;
  args: string[];
}

function steps(plan: Plan & { kind: "selected" }, base: string): { rust: Step[]; ts: Step[] } {
  const rust: Step[] = [];
  const { tests, clippy, fmt } = plan.rust;
  if (tests !== null) {
    rust.push({
      command: "moon",
      args: ["run", "root:rust-test", ...(tests === "all" ? [] : ["--", "-E", tests.filterset])],
    });
  }
  // The same commands as `root:rust-lint`, limited to the affected crates; that task is a script,
  // which takes no arguments.
  if (clippy.length > 0) {
    rust.push({
      command: "cargo",
      args: [
        "clippy",
        ...clippy.flatMap((name) => ["-p", name]),
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
      ],
    });
  }
  if (fmt) {
    rust.push({ command: "cargo", args: ["fmt", "--all", "--check"] });
  }
  const ts: Step[] = [];
  if (plan.narrowed.length > 0) {
    ts.push({ command: "moon", args: ["run", ...plan.narrowed, "--", "--changed", base] });
  }
  if (plan.whole.length > 0) {
    ts.push({ command: "moon", args: ["run", ...plan.whole] });
  }
  return { rust, ts };
}

const show = (step: Step) => `${step.command} ${step.args.map(quote).join(" ")}`;

function describe(plan: Plan, base: string, changed: number): string[] {
  if (plan.kind === "full") {
    return [`full check: ${plan.reason}`];
  }
  const { rust, ts } = steps(plan, base);
  const out = [`${changed} changed file(s) since ${base.slice(0, 9)}`];
  for (const [target, why] of Object.entries(plan.why)) {
    out.push(`  ${target}: ${why.join(", ")}`);
  }
  const all = [...rust, ...ts];
  out.push(all.length === 0 ? "nothing to run" : "steps:", ...all.map((step) => `  ${show(step)}`));
  return out;
}

const execute = (step: Step) =>
  new Promise<number>((resolve) => {
    console.log(`$ ${show(step)}`);
    const child = spawn(step.command, step.args, { stdio: "inherit" });
    child.on("error", () => resolve(1));
    child.on("exit", (code) => resolve(code ?? 1));
  });

// Stops at the first failure of a track, but lets the other track finish: the failures of both are
// what the author needs.
async function sequence(list: Step[]): Promise<number> {
  for (const step of list) {
    const code = await execute(step);
    if (code !== 0) {
      return code;
    }
  }
  return 0;
}

async function main(argv: string[]): Promise<number> {
  const dryRun = argv.includes("--dry-run");
  const baseAt = argv.indexOf("--base");
  const requested = baseAt >= 0 ? argv[baseAt + 1] : undefined;

  let plan: Plan;
  let base = "";
  let changed = 0;
  try {
    const gathered = gather(requested);
    base = gathered.base;
    changed = gathered.facts.changed.length;
    plan = select(gathered.facts);
  } catch (error) {
    const message = error instanceof Error ? error.message.split("\n")[0] : String(error);
    plan = { kind: "full", reason: `the change set cannot be computed (${message})` };
  }

  console.log(describe(plan, base, changed).join("\n"));
  if (plan.kind === "full") {
    const full = { command: "moon", args: FULL_CHECK };
    console.log(`steps:\n  ${show(full)}`);
    return dryRun ? 0 : execute(full);
  }
  if (dryRun) {
    return 0;
  }
  const { rust, ts } = steps(plan, base);
  const codes = await Promise.all([sequence(rust), sequence(ts)]);
  return codes.find((code) => code !== 0) ?? 0;
}

process.exitCode = await main(process.argv.slice(2));
