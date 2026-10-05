// Chooses the checks that a set of changed files affects. Pure: `changed.ts` gathers the facts
// (git, moon, cargo) and runs the plan, so every rule here is testable without a repository.
import { matchesGlob } from "node:path";

export interface Project {
  id: string;
  source: string;
  /** Workspace packages it lists in package.json; the module graph of its tests follows these. */
  dependencies: readonly string[];
}

export interface Task {
  target: string;
  project: string;
  name: string;
  /** Workspace-relative globs and files from the declared inputs; a leading `!` excludes. */
  globs: readonly string[];
  files: readonly string[];
  deps: readonly string[];
  /** A plain `vitest` command, to which moon can pass `--changed`. A script cannot take arguments. */
  vitest: boolean;
}

export interface Crate {
  name: string;
  dir: string;
  dependsOn: readonly string[];
}

export interface Facts {
  changed: readonly string[];
  projects: readonly Project[];
  tasks: readonly Task[];
  crates: readonly Crate[];
  /**
   * Project id to the ids of the projects whose folders its files name by path: `source` for files
   * under `src/`, which every importer runs too, `own` for tests and build scripts, which only its
   * own suite runs.
   */
  reads: {
    source: Readonly<Record<string, readonly string[]>>;
    own: Readonly<Record<string, readonly string[]>>;
  };
}

export interface RustPlan {
  /** `all` for a change in a file the tests read, a nextest filterset for changed crates, or none. */
  tests: "all" | { filterset: string } | null;
  clippy: string[];
  fmt: boolean;
}

export type Plan =
  | { kind: "full"; reason: string }
  | {
      kind: "selected";
      /** Moon targets run with `--changed`: only the tests that import a changed module. */
      narrowed: string[];
      /** Moon targets run whole. */
      whole: string[];
      rust: RustPlan;
      /** Why each target runs, for the printed plan. */
      why: Record<string, string[]>;
    };

const ROOT = "root";

// A change here alters the toolchain, the dependency set or the task graph itself, so no subset
// computed from the old graph can be trusted.
const FULL_RUN_FILES: readonly [RegExp, string][] = [
  [/^pnpm-lock\.yaml$/, "a lockfile"],
  [/^Cargo\.lock$/, "a lockfile"],
  [/^mise\.toml$/, "the toolchain pins"],
  [/^rust-toolchain(\.toml)?$/, "the toolchain pins"],
  [/^\.moon\//, "the moon configuration"],
  [/^moon\.yml$/, "the root tasks"],
  [/^pnpm-workspace\.yaml$/, "the workspace configuration"],
  [/^package\.json$/, "the workspace configuration"],
  [/^Cargo\.toml$/, "the workspace configuration"],
  [/^tsconfig\.json$/, "the TypeScript configuration"],
  [/^vitest\.config\.ts$/, "the Vitest configuration"],
  [/^clippy\.toml$/, "the clippy configuration"],
  [/^\.oxfmtrc\.json$/, "the formatter configuration"],
];

// The inherited `test` task and the root checks declare these as inputs because they cannot tell
// which package or crate a task reads. Taken literally they select every task for any change, so
// they are replaced by the package graph and the crate graph below.
const COARSE_GLOBS = new Set(["packages/**/*", "crates/**/*"]);

// The crate whose build output each task consumes. A task that lists `crates/**/*` without an entry
// is treated as reading every crate. The WebAssembly and native builds list the crates they are made
// of instead, so their own globs select them.
const TASK_CRATE: Readonly<Record<string, string>> = {
  "root:cli": "weft-cli",
  "xcode-plugin:bundle": "weft-cli",
  "visual:test": "weft-cli",
};

// The runtime legs run only these suites, though their declared inputs name every package.
const RUNTIME_SUITES = ["core", "catalog"];

// The linters and the type checker read sources and JSON only; Markdown never reaches them.
const SOURCE_FILES = /\.(m?[jt]sx?|json)$/;
const SOURCE_ONLY = new Set(["root:lint", "root:typecheck"]);
const MODULE_FILES = /\.(m?ts|tsx)$/;

const RUST_TEST = "root:rust-test";
const RUST_LINT = "root:rust-lint";

interface Reason {
  text: string;
  /** Reached only through an import, which Vitest's `--changed` can follow. */
  module: boolean;
}

const isCheck = (task: Task): boolean =>
  task.name === "test" || ["root:typecheck", "root:lint", "root:runtimes"].includes(task.target);

const globMatch = (file: string, task: Task, skipCoarse: boolean): boolean => {
  const globs = task.globs.filter((glob) => !(skipCoarse && COARSE_GLOBS.has(glob)));
  const hit = (list: readonly string[]) => list.some((glob) => matchesGlob(file, glob));
  return (
    task.files.includes(file) ||
    (hit(globs.filter((glob) => !glob.startsWith("!"))) &&
      !hit(globs.filter((glob) => glob.startsWith("!")).map((glob) => glob.slice(1))))
  );
};

const closure = (start: Iterable<string>, edges: ReadonlyMap<string, ReadonlySet<string>>) => {
  const seen = new Set<string>(start);
  const queue = [...seen];
  for (let id = queue.pop(); id !== undefined; id = queue.pop()) {
    for (const next of edges.get(id) ?? []) {
      if (!seen.has(next)) {
        seen.add(next);
        queue.push(next);
      }
    }
  }
  return seen;
};

/** Project ids named by a source text through a path such as `packages/core`, `plugins/shared` or `../../catalog/`. */
export function referencedProjects(text: string, projects: readonly Project[]): string[] {
  const ids = new Set<string>();
  for (const [, prefix = "", name = ""] of text.matchAll(
    /(packages\/|plugins\/|\.\.\/)([a-z][a-z0-9-]*)(?=[/"'`\s)])/g,
  )) {
    for (const project of projects) {
      const folder = project.source.split("/");
      const same =
        prefix === "../" ? folder.at(-1) === name : project.source === `${prefix}${name}`;
      if (same && project.id !== ROOT) {
        ids.add(project.id);
      }
    }
  }
  return [...ids];
}

/** The project whose folder holds the file; the root project owns everything else and is not a candidate. */
export function projectOf(file: string, projects: readonly Project[]): string | undefined {
  return projects
    .filter((project) => project.id !== ROOT && file.startsWith(`${project.source}/`))
    .sort((a, b) => b.source.length - a.source.length)[0]?.id;
}

export function select(facts: Facts): Plan {
  for (const file of facts.changed) {
    const rule = FULL_RUN_FILES.find(([pattern]) => pattern.test(file));
    if (rule) {
      return { kind: "full", reason: `${file} changed (${rule[1]})` };
    }
  }

  // The nearest package wins, as in the determinator: a file under `crates/` that belongs to no
  // package means the crate layout changed.
  const crateOf = (file: string) =>
    facts.crates
      .filter((crate) => file.startsWith(`${crate.dir}/`))
      .sort((a, b) => b.dir.length - a.dir.length)[0];
  const seeds = new Set<string>();
  for (const file of facts.changed) {
    if (!file.startsWith("crates/")) {
      continue;
    }
    const crate = crateOf(file);
    if (!crate) {
      return { kind: "full", reason: `${file} changed and belongs to no crate` };
    }
    seeds.add(crate.name);
  }
  const dependents = new Map<string, Set<string>>();
  for (const crate of facts.crates) {
    for (const dep of crate.dependsOn) {
      dependents.set(dep, (dependents.get(dep) ?? new Set()).add(crate.name));
    }
  }
  const affectedCrates = closure(seeds, dependents);

  const projects = facts.projects.filter((project) => project.id !== ROOT);
  const owner = (file: string) => projectOf(file, projects);
  const declared = new Map(projects.map((p) => [p.id, new Set(p.dependencies)]));
  // Imports are what Vitest follows; a path in a source text is only a guess that the file is read,
  // and only one step of it counts: what a project reads by path is not what that project imports.
  const imported = (id: string) => closure([id], declared);
  const read = (id: string) => {
    const imports = imported(id);
    const ids = new Set(imports);
    for (const dependency of imports) {
      (facts.reads.source[dependency] ?? []).forEach((other) => ids.add(other));
    }
    (facts.reads.own[id] ?? []).forEach((other) => ids.add(other));
    return ids;
  };

  const reasons = new Map<string, Reason[]>();
  const add = (target: string, reason: Reason) =>
    reasons.set(target, [...(reasons.get(target) ?? []), reason]);

  for (const task of facts.tasks) {
    if (task.target === RUST_TEST || task.target === RUST_LINT) {
      continue;
    }
    const coarsePackages = task.globs.includes("packages/**/*");
    const own = task.project === ROOT ? undefined : task.project;
    for (const file of facts.changed) {
      if (SOURCE_ONLY.has(task.target) && !SOURCE_FILES.test(file)) {
        continue;
      }
      const home = owner(file);
      if (globMatch(file, task, true)) {
        const module =
          own !== undefined &&
          home !== undefined &&
          MODULE_FILES.test(file) &&
          imported(own).has(home);
        add(task.target, { text: file, module });
      } else if (coarsePackages && own !== undefined && home !== undefined && read(own).has(home)) {
        add(task.target, {
          text: file,
          module: MODULE_FILES.test(file) && imported(own).has(home),
        });
      } else if (coarsePackages && own === undefined && file.startsWith("packages/")) {
        const suites = task.target.startsWith("root:runtimes-");
        if (
          !suites ||
          (home !== undefined && RUNTIME_SUITES.some((suite) => read(suite).has(home)))
        ) {
          add(task.target, { text: file, module: false });
        }
      }
    }
    if (seeds.size > 0 && task.globs.includes("crates/**/*")) {
      const crate = TASK_CRATE[task.target];
      if (crate === undefined || affectedCrates.has(crate)) {
        add(task.target, { text: `crates ${[...seeds].join(", ")}`, module: false });
      }
    }
  }

  // A task that depends on an affected task runs again whatever its own inputs say: the generated
  // module and the CLI feed every test that loads them.
  for (let changed = true; changed;) {
    changed = false;
    for (const task of facts.tasks) {
      const dep = task.deps.find((target) => reasons.has(target));
      if (dep && !(reasons.get(task.target) ?? []).some((r) => r.text === `needs ${dep}`)) {
        add(task.target, { text: `needs ${dep}`, module: false });
        changed = true;
      }
    }
  }

  const narrowed: string[] = [];
  const whole: string[] = [];
  const why: Record<string, string[]> = {};
  for (const task of facts.tasks.filter(isCheck)) {
    const found = reasons.get(task.target);
    if (!found) {
      continue;
    }
    (task.vitest && found.every((reason) => reason.module) ? narrowed : whole).push(task.target);
    why[task.target] = [...new Set(found.map((reason) => reason.text))].slice(0, 3);
  }

  const rustTest = facts.tasks.find((task) => task.target === RUST_TEST);
  const data =
    rustTest !== undefined &&
    facts.changed.some((file) => !file.startsWith("crates/") && globMatch(file, rustTest, true));
  const names = [...seeds].sort();
  return {
    kind: "selected",
    narrowed: narrowed.sort(),
    whole: whole.sort(),
    rust: {
      tests: data
        ? "all"
        : names.length > 0
          ? { filterset: names.map((name) => `rdeps(=${name})`).join(" | ") }
          : null,
      clippy: [...affectedCrates].sort(),
      fmt: seeds.size > 0,
    },
    why,
  };
}
